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

use crate::cli::LogTarget;
use anyhow::{Context, Result};
use log::LevelFilter;

/// Initializes the global logger based on the specified target and verbosity.
///
/// This function configures the logging backend and filter level.
/// - **Level**: `Debug` if `verbose` is true, otherwise `Warn`.
/// - **Target**: Delegates to `init_stderr` or `init_system` based on `target`.
///
/// # Arguments
///
/// * `target` - The [`LogTarget`] (e.g., `Stderr`, `System`).
/// * `verbose` - If true, sets the log level to `Debug`; otherwise, `Warn`.
///
/// # Returns
///
/// * `Ok(())` if the logger was successfully initialized.
/// * `Err(...)` if `init_system` fails (e.g., cannot connect to syslog).
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

/// Initializes the `env_logger` backend for writing logs to `stderr`.
///
/// This function attempts to register `env_logger` as the global logger.
/// It is idempotent: if registration fails (e.g., because a logger is already set),
/// it is ignored, but the log level is always updated via `log::set_max_level`.
///
/// # Formatting
///
/// The output is configured to be terse, matching the original Perl tool:
/// - No timestamps.
/// - No module paths.
/// - The log level is prefixed for `Warn` and `Error` messages.
fn init_stderr(level: LevelFilter) -> Result<()> {
    let _ = env_logger::Builder::new()
        .filter_level(level)
        .format_timestamp(None)
        .format_module_path(false)
        .format_target(false)
        .format_level(true)
        .try_init();

    log::set_max_level(level);

    Ok(())
}

/// Initializes the `syslog` backend for Unix-like systems.
///
/// This function connects to the local syslog daemon via a Unix domain socket
/// (typically `/dev/log`). It uses `Formatter3164` with the `LOG_DAEMON` facility.
///
/// # Errors
///
/// Returns an error if:
/// - The connection to the syslog socket fails.
/// - Registering the logger with the `log` crate fails.
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

/// Initializes the Windows Event Log backend.
///
/// This function first attempts to register the "reniced" source with the Event Log.
/// If this fails (typically due to lack of admin rights), it falls back to `init_stderr`
/// with a warning. If registration succeeds, it initializes the `winlog` logger.
///
/// # Errors
///
/// Returns an error only if `winlog::init` fails after a successful registration.
#[cfg(windows)]
fn init_system(level: LevelFilter) -> Result<()> {
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
