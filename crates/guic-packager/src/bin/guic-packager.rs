// Copyright 2023-2023 CrabNebula Ltd.
// SPDX-License-Identifier: Apache-2.0 OR MIT

fn main() {
    guic_packager::cli::run(std::env::args_os().skip(1), Some("guic-packager".into()))
}
