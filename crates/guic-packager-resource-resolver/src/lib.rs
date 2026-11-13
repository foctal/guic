// Copyright 2023-2023 CrabNebula Ltd.
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! # guic-packager-resource-resolver
//!
//! Resource resolver for apps that were packaged by [`guic-packager`](https://docs.rs/guic-packager).
//!
//! It resolves the root path which contains resources, which was set using the `resources`
//! field of [cargo packager configuration](https://docs.rs/guic-packager/latest/guic_packager/config/struct.Config.html).
//!
//! ## Get the resource path
//!
//! ```
//! use guic_packager_resource_resolver::{resources_dir, PackageFormat};
//!
//! let resource_path = resources_dir(PackageFormat::Nsis).unwrap();
//! ```
//! ## Automatically detect formats
//!
//! Enable `auto-detect-format` and build the application through
//! `before_each_package_command`, which sets `CARGO_PACKAGER_FORMAT` for each
//! package. `before_packaging_command` does not set that variable.
//!
//! ```no_run
//! # #[cfg(feature = "auto-detect-format")]
//! # fn example() -> Result<(), guic_packager_resource_resolver::Error> {
//! use guic_packager_resource_resolver::{resources_dir, current_format};
//!
//! let resource_path = resources_dir(current_format()?)?;
//! # Ok(())
//! # }
//! ```
//!
use std::path::PathBuf;

use error::Result;
use guic_packager_utils::current_exe::current_exe;
pub use guic_packager_utils::PackageFormat;

mod error;

pub use error::Error;

/// Get the current package format.
/// Requires the `auto-detect-format` feature and an application compiled through
/// the packager's `before_each_package_command` hook.
#[cfg(feature = "auto-detect-format")]
pub fn current_format() -> crate::Result<PackageFormat> {
    match std::option_env!("CARGO_PACKAGER_FORMAT") {
        Some("app") => Ok(PackageFormat::App),
        Some("dmg") => Ok(PackageFormat::Dmg),
        Some("wix") => Ok(PackageFormat::Wix),
        Some("nsis") => Ok(PackageFormat::Nsis),
        Some("deb") => Ok(PackageFormat::Deb),
        Some("appimage") => Ok(PackageFormat::AppImage),
        Some("pacman") => Ok(PackageFormat::Pacman),
        _ => Err(Error::UnkownPackageFormat),
    }
}

/// Retrieve the resource path of your app, packaged with cargo packager.
///
/// ## Example
///
/// ```
/// use guic_packager_resource_resolver::{resources_dir, PackageFormat};
///
/// let resource_path = resources_dir(PackageFormat::Nsis).unwrap();
/// ```
pub fn resources_dir(package_format: PackageFormat) -> Result<PathBuf> {
    match package_format {
        PackageFormat::App | PackageFormat::Dmg => {
            let exe = current_exe()?;
            let exe_dir = exe
                .parent()
                .ok_or_else(|| Error::ParentNotFound(exe.clone()))?;
            Ok(exe_dir.join("../Resources"))
        }
        PackageFormat::Wix | PackageFormat::Nsis => {
            let exe = current_exe()?;
            let exe_dir = exe
                .parent()
                .ok_or_else(|| Error::ParentNotFound(exe.clone()))?;
            Ok(exe_dir.to_path_buf())
        }
        PackageFormat::Deb | PackageFormat::Pacman => {
            let exe = current_exe()?;
            let exe_name = exe.file_name().unwrap().to_string_lossy();

            let path = format!("/usr/lib/{exe_name}/");
            Ok(PathBuf::from(path))
        }

        PackageFormat::AppImage => {
            let appdir = std::env::var_os("APPDIR").ok_or(Error::AppDirNotFound)?;

            // validate that we're actually running on an AppImage
            // an AppImage is mounted to `/$TEMPDIR/.mount_${appPrefix}${hash}`
            // see https://github.com/AppImage/AppImageKit/blob/1681fd84dbe09c7d9b22e13cdb16ea601aa0ec47/src/runtime.c#L501
            // note that it is safe to use `std::env::current_exe` here since we just loaded an AppImage.
            let is_temp = std::env::current_exe()
                .map(|p| {
                    p.display()
                        .to_string()
                        .starts_with(&format!("{}/.mount_", std::env::temp_dir().display()))
                })
                .unwrap_or(true);

            if !is_temp {
                return Err(Error::InvalidAppImage);
            }

            let appdir: &std::path::Path = appdir.as_ref();

            let exe = current_exe()?;
            let exe_name = exe.file_name().unwrap().to_string_lossy();

            Ok(PathBuf::from(format!(
                "{}/usr/lib/{}",
                appdir.display(),
                exe_name
            )))
        }
        _ => Err(Error::UnsupportedPackageFormat),
    }
}
