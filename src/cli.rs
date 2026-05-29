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

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use std::path::PathBuf;

/// Available subcommands for `reniced`.
///
/// Subcommands allow grouping specific utility functions separate from the default priority adjustment logic.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Generate shell completion scripts for the specified shell.
    ///
    /// Output the script to stdout and redirect it to your shell's completion directory.
    ///
    /// # Example
    /// ```bash
    /// reniced completions bash > /etc/bash_completion.d/reniced
    /// ```
    Completions {
        /// The shell to generate completions for.
        ///
        /// Supported values: `bash`, `elvish`, `fish`, `powershell`, `zsh`.
        #[arg(value_enum)]
        shell: Shell,
    },
}

/// Specifies the destination for application log output.
#[derive(Clone, Debug, ValueEnum)]
pub enum LogTarget {
    /// **Standard Error** (default).
    ///
    /// Logs are written to `stderr`. Ideal for interactive use, debugging, or when running
    /// under systemd (where stderr is captured by the journal).
    Stderr,

    /// **System Logger**.
    ///
    /// Logs are sent to the native OS logging facility:
    /// - **Unix**: `syslog`
    /// - **Windows**: Event Log
    ///
    /// Ideal for background daemons or cron jobs where terminal output is unavailable.
    System,
}

/// Specifies which process attribute to match against rule regular expressions.
#[derive(Clone, Debug, ValueEnum)]
pub enum MatchTarget {
    /// **Process Name** (`comm`).
    ///
    /// Matches the base name of the process.
    /// - **Note on Linux**: Limited to **15 characters** (kernel `TASK_COMM_LEN` limit).
    Name,

    /// **Full Command Line** (`argv`).
    ///
    /// Matches the complete command line string (executable + arguments), joined by spaces.
    /// Useful for distinguishing processes running the same binary with different arguments.
    Cmdline,

    /// **Full Executable Path**.
    ///
    /// Matches the absolute path to the binary (e.g., `/usr/bin/python3`).
    /// Most precise for distinguishing between different versions or installations of the same tool.
    Exe,

    /// **Executable Basename**.
    ///
    /// Matches the filename of the executable without the directory path.
    /// - **Advantage**: Unlike `Name`, this is **not truncated** on Linux, supporting full-length names.
    ExeBasename,
}

/// Command-line interface arguments for `reniced`.
///
/// This struct defines the global flags and subcommands used to configure
/// process priority adjustments.
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
    /// Path to an alternate configuration file.
    ///
    /// If omitted, defaults to `/etc/reniced.conf` (if root) or `$HOME/.reniced` (if user).
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// No-op mode: show what would be done without making changes.
    ///
    /// Useful for validating configuration files or rules safely.
    #[arg(short = 'n', long = "dry-run")]
    pub noop: bool,

    /// Destination for log output.
    ///
    /// Use `stderr` for interactive/systemd use.
    /// Use `system` for cron/background daemons (sends to syslog/Event Log).
    #[arg(long, value_enum, default_value = "stderr")]
    pub log: LogTarget,

    /// Field to match rules against.
    ///
    /// Determines which process attribute (name, cmdline, exe, etc.) is tested
    /// against the regular expressions in the configuration file.
    #[arg(short = 'o', long, value_enum, default_value = "name")]
    pub match_target: MatchTarget,

    /// Include threads/tasks in addition to processes.
    ///
    /// **Linux only**: Requires root privileges to access thread-level information.
    /// Ignored on other platforms.
    #[arg(short, long)]
    pub threads: bool,

    /// Verbose mode: log successful adjustments as well as errors.
    ///
    /// By default, only errors or significant state changes are logged.
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// The subcommand to execute (e.g., `completions`).
    ///
    /// If omitted, the program runs in its default mode (scanning and applying rules).
    #[command(subcommand)]
    pub command: Option<Commands>,
}

impl Cli {
    /// Parses command-line arguments into a [`Cli`] struct.
    ///
    /// This is a convenience wrapper around `clap::Parser::parse()`.
    /// It is marked `#[must_use]` to ensure the parsed configuration is not accidentally discarded.
    ///
    /// # Panics
    ///
    /// Panics if the arguments are invalid or if the user requests help/version information
    /// (which triggers an exit).
    #[must_use]
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
