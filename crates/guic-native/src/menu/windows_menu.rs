use anyhow::{Result, ensure};
use std::cell::Cell;
use tray_icon::menu::{ContextMenu, Menu};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::WM_DESTROY,
    },
};

/// Detach before DestroyWindow destroys the shared HMENU and its submenus.
/// The allocation remains stable until the subclass is removed on the UI thread.
pub(super) struct Attachment(Box<Data>);
struct Data {
    menu: Menu,
    hwnd: Cell<HWND>,
}
impl Attachment {
    /// The HWND must be live and owned by the calling thread.
    pub(super) unsafe fn new(menu: Menu, hwnd: HWND) -> Result<Self> {
        let attachment = Self(Box::new(Data {
            menu,
            hwnd: Cell::new(std::ptr::null_mut()),
        }));
        // SAFETY: the caller provides a live HWND on its owning thread.
        unsafe { attachment.0.menu.init_for_hwnd(hwnd as isize)? };
        attachment.0.hwnd.set(hwnd);
        let data = &*attachment.0 as *const Data as usize;
        // SAFETY: the boxed data outlives the subclass, including failed installation.
        ensure!(
            unsafe { SetWindowSubclass(hwnd, Some(subclass), data, data) } != 0,
            "could not install menu lifetime subclass"
        );
        Ok(attachment)
    }
}
impl Data {
    unsafe fn detach(&self) {
        let hwnd = self.hwnd.replace(std::ptr::null_mut());
        if !hwnd.is_null() {
            // SAFETY: called on the owning thread before HWND destruction, or by Drop
            // while the window is still live. Clear the handle first for reentrancy.
            unsafe {
                RemoveWindowSubclass(hwnd, Some(subclass), self as *const Self as usize);
                self.menu.detach_menu_subclass_from_hwnd(hwnd as isize);
                let _ = self.menu.remove_for_hwnd(hwnd as isize);
            }
        }
    }
}
impl Drop for Attachment {
    fn drop(&mut self) {
        // SAFETY: attachments are created and dropped only on the owning UI thread.
        unsafe { self.0.detach() };
    }
}
unsafe extern "system" fn subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    data: usize,
) -> LRESULT {
    if message == WM_DESTROY {
        // SAFETY: the subclass holds a stable pointer until detach removes it.
        unsafe { (&*(data as *const Data)).detach() };
    }
    // SAFETY: forward the unchanged message to the remaining window procedures.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tray_icon::menu::{MenuItem, Submenu};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, GetMenu, IsMenu, WS_OVERLAPPEDWINDOW,
    };

    #[test]
    fn closing_and_reopening_windows_preserves_shared_menu() {
        // Hidden native windows exercise real destruction without a GPUI test backend.
        unsafe {
            let create = || {
                CreateWindowExW(
                    0,
                    windows_sys::w!("STATIC"),
                    windows_sys::w!("GUIC menu lifetime test"),
                    WS_OVERLAPPEDWINDOW,
                    0,
                    0,
                    100,
                    100,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                )
            };
            let menu = Menu::new();
            let submenu = Submenu::new("Application", true);
            submenu
                .append(&MenuItem::new("Show", true, None))
                .expect("append command");
            menu.append(&submenu).expect("append submenu");
            let first = create();
            assert!(!first.is_null());
            let first_attachment = Attachment::new(menu.clone(), first).expect("attach first");
            let native_menu = GetMenu(first);
            assert!(!native_menu.is_null());
            assert!(Attachment::new(menu.clone(), first).is_err());
            assert_eq!(
                GetMenu(first),
                native_menu,
                "failed duplicate attachment is inert"
            );
            let other = create();
            assert!(!other.is_null());
            let other_attachment = Attachment::new(menu.clone(), other).expect("attach other");
            assert_ne!(DestroyWindow(first), 0);
            assert_ne!(
                IsMenu(native_menu),
                0,
                "DestroyWindow must not destroy shared menus"
            );
            assert_eq!(GetMenu(other), native_menu);
            drop(first_attachment);
            let second = create();
            assert!(!second.is_null());
            let second_attachment = Attachment::new(menu, second).expect("attach second");
            assert_eq!(GetMenu(second), native_menu);
            drop(second_attachment);
            assert!(
                GetMenu(second).is_null(),
                "dropping the owner detaches a live window"
            );
            assert_ne!(DestroyWindow(second), 0);
            assert_ne!(DestroyWindow(other), 0);
            drop(other_attachment);
        }
    }
}
