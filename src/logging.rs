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

use anyhow::{Context, Result};
use log::LevelFilter;

use crate::cli::LogTarget;

pub fn init(target: &LogTarget, verbose: bool) -> Result<()> {
    let level = if verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Warn
    };

    match target {
        LogTarget::Stderr => init_stderr(level),
        LogTarget::System => init_system(level),
    }
}

fn init_stderr(level: LevelFilter) -> Result<()> {
    // Register the logger once; subsequent calls (e.g. in tests) will get
    // SetLoggerError which we ignore — the important thing is that the level
    // is always updated, so we set it unconditionally after the init attempt.
    let _ = env_logger::Builder::new()
        .filter_level(level)
        // Match the terse output style of the original Perl code —
        // no timestamps, no module paths, just the message.
        .format_timestamp(None)
        .format_module_path(false)
        .format_target(false)
        // Prefix warn/error with the level so it's clear in a terminal;
        // info/debug are noop-mode and verbose output so no prefix needed.
        .format_level(true)
        .try_init();

    // Always update the max level regardless of whether init succeeded.
    // This is safe to call multiple times and is what the tests rely on.
    log::set_max_level(level);

    Ok(())
}

#[cfg(unix)]
fn init_system(level: LevelFilter) -> Result<()> {
    use syslog::{BasicLogger, Facility, Formatter3164};

    let formatter = Formatter3164 {
        facility: Facility::LOG_DAEMON,
        hostname: None,
        process: "reniced".into(),
        pid: std::process::id(),
    };

    let logger = syslog::unix(formatter).context("failed to connect to syslog")?;

    log::set_boxed_logger(Box::new(BasicLogger::new(logger)))
        .context("failed to register syslog logger")?;

    log::set_max_level(level);

    Ok(())
}

#[cfg(windows)]
fn init_system(level: LevelFilter) -> Result<()> {
    // The Windows Event Log source must be registered before first use.
    // winlog::register is idempotent and succeeds silently if already present.
    // It requires elevation; if it fails we fall back to stderr with a warning
    // rather than aborting — the tool can still run, just without Event Log.
    if let Err(e) = winlog::register("reniced") {
        eprintln!(
            "warning: could not register Windows Event Log source \
             (run once as Administrator to suppress this): {e}"
        );
        return init_stderr(level);
    }

    winlog::init("reniced").context("failed to initialise Windows Event Log logger")?;

    log::set_max_level(level);

    Ok(())
}

// FreeBSD, OpenBSD, and any other non-Linux Unix currently lacks a syslog
// backend distinct from the Unix one above — the syslog crate uses the same
// Unix domain socket path on all *nix systems, so the cfg(unix) arm covers
// all of them.
