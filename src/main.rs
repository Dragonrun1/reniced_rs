#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
use anyhow::Result;

use reniced::actions::apply_rules;
use reniced::cli::Cli;
use reniced::config::{
    find_rulefile,
    read_rules,
};
use reniced::process::read_processes;

fn main() -> Result<()> {
    let cli = Cli::parse_args();

    let rulefile = find_rulefile(&cli)?;
    let rules = read_rules(&rulefile)?;

    if rules.is_empty() {
        return Ok(());
    }

    let processes = read_processes(cli.threads)?;

    if processes.is_empty() {
        return Ok(());
    }

    for process in &processes {
        if let Err(error) =
            apply_rules(process, &rules, &cli)
        {
            eprintln!(
                "failed processing pid {} ({}): {}",
                process.pid,
                process.cmd,
                error,
            );
        }
    }

    Ok(())
}
