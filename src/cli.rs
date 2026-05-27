use std::path::PathBuf;

use clap::Parser;

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

    /// Alternate config file
    pub configfile: Option<PathBuf>,
}

impl Cli {
    #[must_use]
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
