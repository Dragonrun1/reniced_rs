use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Clone, Debug, ValueEnum)]
pub enum MatchTarget {
    Name,
    Cmdline,
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
    #[arg(
        short = 'o',
        long,
        value_enum,
        default_value = "name",
    )]
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
