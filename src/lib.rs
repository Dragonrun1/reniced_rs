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

pub mod actions;
pub mod cli;
pub mod config;
pub mod logging;
pub mod model;
pub mod platform;
pub mod process;

use std::io;

use anyhow::Result;
use clap_complete::{Generator, generate};
use log::{info, warn};

use crate::actions::apply_rules;
use crate::cli::Cli;
use crate::config::{find_rulefile, read_rules};
use crate::process::read_processes;

#[cfg(target_os = "linux")]
use crate::platform::unix::is_privileged;

/// Core application logic, separated from CLI parsing and logging init
/// so it can be called from tests with a constructed Cli.
pub fn run(cli: Cli) -> Result<()> {
    // Check privileges early if threads are requested on Linux
    #[cfg(target_os = "linux")]
    if cli.threads && !is_privileged() {
        return Err(anyhow::anyhow!(
            "--threads requires root privileges on Linux;              run with 'sudo' or set CAP_SYS_PTRACE on this binary"
        ));
    }

    // Non-Linux: return error as threads are unsupported
    #[cfg(not(target_os = "linux"))]
    if cli.threads {
        return Err(anyhow::anyhow!("--threads is only supported on Linux"));
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

/// Write shell completions for the given shell to the provided writer.
/// Pass `&mut io::stdout()` for production use, or `&mut Vec::new()` in tests.
pub fn print_completions<G: Generator>(shell: G, cmd: &mut clap::Command, writer: &mut dyn io::Write) {
    let bin_name = cmd.get_name().to_string();
    generate(shell, cmd, bin_name, writer);
}
