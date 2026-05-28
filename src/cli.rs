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

#[derive(Debug, Parser)]
#[command(name = "reniced")]
#[command(version)]
pub struct Cli {
    /// No-op mode
    #[arg(short = 'n')]
    pub noop: bool,

    /// Verbose mode
    #[arg(short = 'v')]
    pub verbose: bool,

    /// Include threads/tasks
    #[arg(long)]
    pub threads: bool,

    /// Match target
    #[arg(short = 'o', long, value_enum, default_value = "name")]
    pub match_target: MatchTarget,

    /// Alternate config file
    pub configfile: Option<PathBuf>,
}

impl Cli {
    #[must_use]
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
