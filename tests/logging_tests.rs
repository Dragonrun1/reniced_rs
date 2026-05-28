// SPDX-FileCopyrightText: 2026 Michael Cummings <mgcummings@yahoo.com>
// SPDX-License-Identifier: GPL-2.0-or-later

// ///////////////////////////////////////////////////////////////////////////
// reniced_rs - A Rust library for renicing processes
//
// Copyright (C) 2026  Michael Cummings
//
// This program is free software; you can redistribute it and/or
// modify it under the terms of the GNU General Public License
// as published by the Free Software Foundation; either version 2
// of the License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program; if not, see <https://www.gnu.org/licenses/>.
// ///////////////////////////////////////////////////////////////////////////

//! Tests for logging::init.
//!
//! env_logger panics if initialized twice in the same process, so these tests
//! use log::max_level() to verify the level is set correctly after init.
//! Only init_stderr is testable here — init_system (syslog) requires a running
//! syslog socket and is integration-only.
//!
//! Tests are serialized via a Mutex to prevent races between log::set_max_level
//! calls from different threads.

use std::sync::Mutex;

use log::LevelFilter;
use reniced::cli::LogTarget;
use reniced::logging::init;

static LOG_INIT: Mutex<()> = Mutex::new(());

#[test]
fn stderr_non_verbose_sets_warn_level() {
    let _guard = LOG_INIT.lock().unwrap();
    init(&LogTarget::Stderr, false).unwrap();
    assert_eq!(log::max_level(), LevelFilter::Warn);
}

#[test]
fn stderr_verbose_sets_debug_level() {
    let _guard = LOG_INIT.lock().unwrap();
    init(&LogTarget::Stderr, true).unwrap();
    assert_eq!(log::max_level(), LevelFilter::Debug);
}
