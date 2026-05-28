use std::path::PathBuf;

use clap::{Parser, ValueEnum};

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
#[command(version)]
pub struct Cli {
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
    #[arg(
        short = 'o',
        long,
        value_enum,
        default_value = "name",
    )]
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

impl Cli {
    #[must_use]
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
