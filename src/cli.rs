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

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

#[derive(Clone, Debug, ValueEnum)]
pub enum MatchTarget {
    /// Process base name (up to 15 chars on Linux)
    Name,
    /// Full argv joined with spaces
    Cmdline,
    /// Full path to the executable
    Exe,
    /// Filename part of the executable path, without truncation
    ExeBasename,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum LogTarget {
    /// Write to stderr (default; good for interactive use and systemd)
    Stderr,
    /// Send to the system logger (syslog on Unix, Event Log on Windows)
    System,
}

#[derive(Debug, Parser)]
#[command(name = "reniced")]
#[command(about, author, version)]
#[command(help_template = "\
{before-help}{name} {version} - {about}
{author-with-newline}
{usage-heading} {usage}

{all-args}{after-help}
")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// No-op mode: show what would be done without making changes
    #[arg(short = 'n')]
    pub noop: bool,

    /// Verbose mode: log successful adjustments as well as errors
    #[arg(short = 'v')]
    pub verbose: bool,

    /// Include threads/tasks in addition to processes (Linux only, requires root)
    #[arg(long)]
    pub threads: bool,

    /// Field to match rules against
    #[arg(short = 'o', long, value_enum, default_value = "name")]
    pub match_target: MatchTarget,

    /// Where to send log output
    ///
    /// Use 'stderr' when running interactively or under systemd.
    /// Use 'system' when running from cron or another context without a
    /// terminal — messages go to syslog (Unix) or the Windows Event Log.
    #[arg(long, value_enum, default_value = "stderr")]
    pub log: LogTarget,

    /// Alternate config file
    pub configfile: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Generate shell completions
    Completions {
        /// The shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}


impl Cli {
    #[must_use]
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
