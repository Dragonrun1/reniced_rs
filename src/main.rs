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

use std::io;

use anyhow::Result;
use clap::CommandFactory;
use clap_version_flag::{colorful_version, parse_with_version};

use log::error;
use reniced::cli::{Cli, Commands};
use reniced::logging::init as init_logging;
use reniced::run;

fn main() -> Result<()> {
    let version = colorful_version!();
    let cli: Cli = parse_with_version(Cli::command(), &version)?;

    if let Some(Commands::Completions { shell }) = cli.command {
        reniced::print_completions(shell, &mut Cli::command(), &mut io::stdout());
        return Ok(());
    }

    init_logging(&cli.log, cli.verbose)?;

    if let Err(e) = run(cli) {
        error!("{e}");
        std::process::exit(1);
    }

    Ok(())
}
