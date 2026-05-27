#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
use anyhow::Result;

use reniced::actions::apply_rules;
use reniced::cli::Cli;
use reniced::config::{find_rulefile, read_rules};
use reniced::process::read_processes;

#[cfg(target_os = "linux")]
use reniced::platform::unix::is_privileged;


fn main() -> Result<()> {
    let cli = Cli::parse_args();

    // Check privileges early if threads are requested on Linux
    #[cfg(target_os = "linux")]
    if cli.threads && !is_privileged() {
        eprintln!("error: --threads requires root privileges on Linux.");
        eprintln!("hint: run with 'sudo' or set CAP_SYS_PTRACE on this binary.");
        std::process::exit(1);
    }

    // Non-Linux: Exit immediately as threads are unsupported
    if cli.threads {
        eprintln!("error: --threads is only supported on Linux.");
        eprintln!("hint: run on a Linux system to enable thread scanning.");
        std::process::exit(1);
    }

    let rulefile = find_rulefile(&cli)?;
    let rules = read_rules(&rulefile)?;
    if rules.is_empty() {
        return Ok(());
    }

    let processes = read_processes(cli.threads)?;
    if processes.is_empty() {
        return Ok(());
    }

    if cli.verbose {
        println!(
            "loaded {} rules and {} processes",
            rules.len(),
            processes.len(),
        );
    }
    for process in &processes {
        if let Err(error) = apply_rules(process, &rules, &cli) {
            eprintln!(
                "failed processing pid {} ({}): {}",
                process.pid, process.cmd, error,
            );
        }
    }
    Ok(())
}
