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

#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
use anyhow::Result;
use log::{error, info, warn};

use reniced::actions::apply_rules;
use reniced::cli::Cli;
use reniced::config::{find_rulefile, read_rules};
use reniced::logging::init as init_logging;
use reniced::process::read_processes;

#[cfg(target_os = "linux")]
use reniced::platform::unix::is_privileged;

fn main() -> Result<()> {
    let cli = Cli::parse_args();

    init_logging(&cli.log, cli.verbose)?;

    // Check privileges early if threads are requested on Linux
    #[cfg(target_os = "linux")]
    if cli.threads && !is_privileged() {
        error!("--threads requires root privileges on Linux");
        error!("hint: run with 'sudo' or set CAP_SYS_PTRACE on this binary");
        std::process::exit(1);
    }

    // Non-Linux: exit immediately as threads are unsupported
    #[cfg(not(target_os = "linux"))]
    if cli.threads {
        error!("--threads is only supported on Linux");
        std::process::exit(1);
    }

    let rulefile = find_rulefile(&cli)?;
    let rules = read_rules(&rulefile)?;
    if rules.is_empty() {
        warn!("no valid rules found in {}", rulefile.display());
        return Ok(());
    }

    let processes = read_processes(cli.threads)?;
    if processes.is_empty() {
        return Ok(());
    }

    info!(
        "loaded {} rules and {} processes",
        rules.len(),
        processes.len()
    );

    for process in &processes {
        if let Err(error) = apply_rules(process, &rules, &cli) {
            warn!(
                "failed processing pid {} ({}): {}",
                process.pid, process.cmd, error,
            );
        }
    }

    Ok(())
}
