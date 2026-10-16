use anyhow::Result;
use futures::future::LocalBoxFuture;
use gpui::{AnyWindowHandle, App, Global, Subscription, Window};
use guic_core::CancellationToken;
use std::{cell::RefCell, rc::Rc};

/// Requested lifecycle transition, available to save and confirmation handlers.
#[derive(Clone, Copy, Debug)]
pub enum ShutdownReason {
    /// Close one window while preserving other windows and tray residency.
    CloseWindow(AnyWindowHandle),
    /// Exit the application.
    Quit,
    /// Restart after saving state, including following an installed update.
    Restart,
}

/// Result of an asynchronous unsaved-work or user confirmation guard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownDecision {
    /// Continue to cancellation, persistence, and cleanup.
    Proceed,
    /// Keep running without cancelling work or closing windows.
    Cancel,
}

/// Observable lifecycle state for application UI.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ShutdownState {
    /// No transition is in progress.
    #[default]
    Idle,
    /// Waiting for guards, save operations, or background cleanup.
    Preparing,
    /// Preparation succeeded and the final platform action is being performed.
    Ready,
    /// A guard or cleanup operation failed; the application remains open.
    Failed(String),
}

type Guard =
    Rc<dyn Fn(ShutdownReason, &mut App) -> LocalBoxFuture<'static, Result<ShutdownDecision>>>;
type Cleanup = Rc<dyn Fn(ShutdownReason, &mut App) -> LocalBoxFuture<'static, Result<()>>>;
type BeforeExit = Box<dyn FnOnce(&mut App) -> LocalBoxFuture<'static, Result<()>>>;

#[derive(Default)]
struct Inner {
    state: ShutdownState,
    guards: Vec<Guard>,
    cleanup: Vec<Cleanup>,
    work: Vec<(Option<gpui::WindowId>, CancellationToken)>,
}

/// Coordinates confirmation, cooperative cancellation, persistence, and exit.
///
/// Route menu and update actions through `request`. Window close requests can
/// be connected with `protect_window`. Guards and cleanup futures run on the
/// GPUI thread; move blocking I/O to `background_executor` inside each handler.
#[derive(Clone, Default)]
pub struct Lifecycle(Rc<RefCell<Inner>>);
impl Global for Lifecycle {}

impl Lifecycle {
    /// Creates a lifecycle coordinator with no callbacks.
    pub fn new() -> Self {
        Self::default()
    }
    /// Registers an asynchronous guard. All guards must approve before cleanup.
    pub fn guard(
        &self,
        guard: impl Fn(ShutdownReason, &mut App) -> LocalBoxFuture<'static, Result<ShutdownDecision>>
        + 'static,
    ) {
        self.0.borrow_mut().guards.push(Rc::new(guard));
    }
    /// Registers a save or cleanup operation, executed in registration order.
    /// Callbacks must tolerate retries after another callback fails.
    pub fn cleanup(
        &self,
        cleanup: impl Fn(ShutdownReason, &mut App) -> LocalBoxFuture<'static, Result<()>> + 'static,
    ) {
        self.0.borrow_mut().cleanup.push(Rc::new(cleanup));
    }
    /// Registers work to cancel on an approved transition. `None` is app-wide
    /// work and is not cancelled when only one window closes.
    pub fn register_work(&self, window: Option<gpui::WindowId>, token: CancellationToken) {
        self.0.borrow_mut().work.push((window, token));
    }
    /// Returns a snapshot of the current state.
    pub fn state(&self) -> ShutdownState {
        self.0.borrow().state.clone()
    }

    /// Installs the coordinator and a final cleanup fallback for platform exits.
    /// Keep the subscription alive. OS shutdown cannot run interactive guards;
    /// normal application actions should use `request` before GPUI starts exiting.
    pub fn install(&self, cx: &mut App) -> Subscription {
        cx.set_global(self.clone());
        let lifecycle = self.clone();
        cx.on_app_quit(move |cx| {
            let ready = lifecycle.state() == ShutdownState::Ready;
            lifecycle.cancel_work(ShutdownReason::Quit);
            let futures = if ready {
                Vec::new()
            } else {
                lifecycle
                    .0
                    .borrow()
                    .cleanup
                    .clone()
                    .into_iter()
                    .map(|cleanup| cleanup(ShutdownReason::Quit, cx))
                    .collect()
            };
            async move {
                for future in futures {
                    if let Err(error) = future.await {
                        tracing::error!("final application cleanup failed: {error}");
                    }
                }
            }
        })
    }

    /// Routes native window close requests through the lifecycle coordinator.
    pub fn protect_window(&self, window: &Window, cx: &App) {
        let lifecycle = self.clone();
        window.on_window_should_close(cx, move |window, cx| {
            lifecycle.request(ShutdownReason::CloseWindow(window.window_handle()), cx);
            false
        });
    }

    /// Starts a transition. Duplicate requests while preparing are ignored.
    /// On cancellation or failure, the platform action is not performed.
    pub fn request(&self, reason: ShutdownReason, cx: &mut App) {
        self.request_action(reason, cx, None);
    }
    /// Confirms and saves before installing an update, then restarts on success.
    /// A cancelled guard never invokes `install`; installation failure leaves the
    /// application open with a failed state. The installer may hand off to an OS
    /// installer only after this preparation has completed.
    pub fn restart_after(
        &self,
        cx: &mut App,
        install: impl FnOnce(&mut App) -> LocalBoxFuture<'static, Result<()>> + 'static,
    ) {
        self.request_action(ShutdownReason::Restart, cx, Some(Box::new(install)));
    }
    fn request_action(&self, reason: ShutdownReason, cx: &mut App, action: Option<BeforeExit>) {
        if matches!(
            self.state(),
            ShutdownState::Preparing | ShutdownState::Ready
        ) {
            return;
        }
        self.0.borrow_mut().state = ShutdownState::Preparing;
        cx.refresh_windows();
        let lifecycle = self.clone();
        cx.spawn(async move |cx| {
            let guards = lifecycle.0.borrow().guards.clone();
            for guard in guards {
                let future = cx.update(|cx| guard(reason, cx));
                match future.await {
                    Ok(ShutdownDecision::Proceed) => {}
                    Ok(ShutdownDecision::Cancel) => {
                        lifecycle.set_state(ShutdownState::Idle, cx);
                        return;
                    }
                    Err(error) => {
                        lifecycle.set_state(ShutdownState::Failed(error.to_string()), cx);
                        return;
                    }
                }
            }
            lifecycle.cancel_work(reason);
            let cleanup = lifecycle.0.borrow().cleanup.clone();
            for callback in cleanup {
                let future = cx.update(|cx| callback(reason, cx));
                if let Err(error) = future.await {
                    lifecycle.set_state(ShutdownState::Failed(error.to_string()), cx);
                    return;
                }
            }
            if let Some(action) = action {
                let future = cx.update(action);
                if let Err(error) = future.await {
                    lifecycle.set_state(ShutdownState::Failed(error.to_string()), cx);
                    return;
                }
            }
            lifecycle.set_state(ShutdownState::Ready, cx);
            cx.update(|cx| match reason {
                ShutdownReason::Quit => cx.quit(),
                ShutdownReason::Restart => cx.restart(),
                ShutdownReason::CloseWindow(handle) => {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                    lifecycle.0.borrow_mut().state = ShutdownState::Idle;
                }
            });
        })
        .detach();
    }

    fn cancel_work(&self, reason: ShutdownReason) {
        let mut inner = self.0.borrow_mut();
        inner.work.retain(|(window, token)| {
            let cancel = match reason {
                ShutdownReason::CloseWindow(handle) => *window == Some(handle.window_id()),
                _ => true,
            };
            if cancel {
                token.cancel();
            }
            !cancel
        });
    }
    fn set_state(&self, state: ShutdownState, cx: &mut gpui::AsyncApp) {
        self.0.borrow_mut().state = state;
        cx.update(|cx| cx.refresh_windows());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[gpui::test]
    fn successful_restart_runs_guards_then_cleanup_then_installer(cx: &mut gpui::TestAppContext) {
        let lifecycle = Lifecycle::new();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let guard_calls = calls.clone();
        lifecycle.guard(move |_, _| {
            guard_calls.borrow_mut().push("guard");
            Box::pin(async { Ok(ShutdownDecision::Proceed) })
        });
        let cleanup_calls = calls.clone();
        lifecycle.cleanup(move |_, _| {
            cleanup_calls.borrow_mut().push("save");
            Box::pin(async { Ok(()) })
        });
        let install_calls = calls.clone();
        let token = CancellationToken::new();
        lifecycle.register_work(None, token.clone());
        let mut restart = cx.expect_restart();
        cx.update(|cx| {
            lifecycle.restart_after(cx, move |_| {
                install_calls.borrow_mut().push("install");
                Box::pin(async { Ok(()) })
            })
        });
        cx.run_until_parked();
        assert!(restart.try_recv().expect("restart signal").is_some());
        assert_eq!(&*calls.borrow(), &["guard", "save", "install"]);
        assert!(token.is_cancelled());
    }
    #[gpui::test]
    fn closing_one_window_preserves_other_work(cx: &mut gpui::TestAppContext) {
        struct Empty;
        impl gpui::Render for Empty {
            fn render(
                &mut self,
                _: &mut Window,
                _: &mut gpui::Context<Self>,
            ) -> impl gpui::IntoElement {
                gpui::div()
            }
        }
        let handle = cx.add_window(|_, _| Empty);
        let lifecycle = Lifecycle::new();
        let app_work = CancellationToken::new();
        let window_work = CancellationToken::new();
        lifecycle.register_work(None, app_work.clone());
        lifecycle.register_work(Some(handle.window_id()), window_work.clone());
        cx.update(|cx| lifecycle.request(ShutdownReason::CloseWindow(handle.into()), cx));
        cx.run_until_parked();
        assert!(window_work.is_cancelled());
        assert!(!app_work.is_cancelled());
        assert!(cx.read(|cx| cx.windows().is_empty()));
        assert_eq!(lifecycle.state(), ShutdownState::Idle);
    }
    #[gpui::test]
    fn cancelled_update_never_runs_installer(cx: &mut gpui::TestAppContext) {
        let lifecycle = Lifecycle::new();
        lifecycle.guard(|_, _| Box::pin(async { Ok(ShutdownDecision::Cancel) }));
        cx.update(|cx| {
            lifecycle.restart_after(cx, |_| panic!("cancelled installation must not start"))
        });
        cx.run_until_parked();
        assert_eq!(lifecycle.state(), ShutdownState::Idle);
    }
    #[gpui::test]
    fn failed_update_is_reported_without_restart(cx: &mut gpui::TestAppContext) {
        let lifecycle = Lifecycle::new();
        cx.update(|cx| {
            lifecycle.restart_after(cx, |_| {
                Box::pin(async { anyhow::bail!("installation failed") })
            })
        });
        cx.run_until_parked();
        assert_eq!(
            lifecycle.state(),
            ShutdownState::Failed("installation failed".into())
        );
    }
    #[gpui::test]
    fn duplicate_requests_do_not_repeat_guards_and_cleanup(cx: &mut gpui::TestAppContext) {
        let lifecycle = Lifecycle::new();
        let calls = Rc::new(std::cell::Cell::new(0));
        let observed = calls.clone();
        lifecycle.guard(move |_, _| {
            observed.set(observed.get() + 1);
            Box::pin(async { Ok(ShutdownDecision::Cancel) })
        });
        cx.update(|cx| {
            lifecycle.request(ShutdownReason::Quit, cx);
            lifecycle.request(ShutdownReason::Restart, cx);
        });
        cx.run_until_parked();
        assert_eq!(calls.get(), 1);
        assert_eq!(lifecycle.state(), ShutdownState::Idle);
        cx.update(|cx| lifecycle.request(ShutdownReason::Quit, cx));
        cx.run_until_parked();
        assert_eq!(calls.get(), 2);
    }
    #[gpui::test]
    fn cancelled_guard_preserves_running_work(cx: &mut gpui::TestAppContext) {
        let lifecycle = Lifecycle::new();
        let token = CancellationToken::new();
        lifecycle.register_work(None, token.clone());
        lifecycle.guard(|_, _| Box::pin(async { Ok(ShutdownDecision::Cancel) }));
        cx.update(|cx| lifecycle.request(ShutdownReason::Quit, cx));
        cx.run_until_parked();
        assert_eq!(lifecycle.state(), ShutdownState::Idle);
        assert!(!token.is_cancelled());
    }
    #[gpui::test]
    fn failed_cleanup_keeps_app_alive(cx: &mut gpui::TestAppContext) {
        let lifecycle = Lifecycle::new();
        lifecycle.cleanup(|_, _| Box::pin(async { anyhow::bail!("save failed") }));
        cx.update(|cx| lifecycle.request(ShutdownReason::Quit, cx));
        cx.run_until_parked();
        assert_eq!(
            lifecycle.state(),
            ShutdownState::Failed("save failed".into())
        );
    }
}
