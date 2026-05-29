// SPDX-FileCopyrightText: 2026 Michael Cummings <mgcummings@yahoo.com>
// SPDX-License-Identifier: GPL-2.0-or-later

// ///////////////////////////////////////////////////////////////////////////
// reniced_rs - A Rust library for renicing processes
// (license header abbreviated for brevity — same as other files)
// ///////////////////////////////////////////////////////////////////////////

//! Tests for run(), print_completions(), and collect_entries().
//!
//! run() is tested with noop=true so it exercises the full rule-loading and
//! process-matching pipeline without making real OS priority changes.
//! collect_entries() is tested by passing a pre-built sysinfo::System so the
//! test controls what processes are visible without mocking the OS.

use std::io::Write;

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::Shell;
use sysinfo::System;
use tempfile::NamedTempFile;

use reniced::cli::{Cli, LogTarget, MatchTarget};
use reniced::process::collect_entries;
use reniced::print_completions;

fn write_config(content: &str) -> Result<NamedTempFile> {
    let mut f = NamedTempFile::new()?;
    f.write_all(content.as_bytes())?;
    Ok(f)
}

fn noop_cli_with_config(path: std::path::PathBuf) -> Cli {
    Cli {
        command: None,
        noop: true,
        verbose: false,
        threads: false,
        match_target: MatchTarget::Name,
        log: LogTarget::Stderr,
        configfile: Some(path),
    }
}

// ── run() ──────────────────────────────────────────────────────────────────

#[test]
fn run_with_empty_rules_returns_ok() -> Result<()> {
    let f = write_config("# no rules\n")?;
    let cli = noop_cli_with_config(f.path().to_path_buf());
    // Empty rules should return Ok without touching any processes.
    reniced::run(cli)?;
    Ok(())
}

#[test]
fn run_with_valid_rules_returns_ok() -> Result<()> {
    let f = write_config("5 doesnotexist_zzz\n")?;
    let cli = noop_cli_with_config(f.path().to_path_buf());
    // Rules that match nothing should still return Ok.
    reniced::run(cli)?;
    Ok(())
}

#[test]
fn run_with_missing_config_errors() {
    let cli = noop_cli_with_config(
        std::path::PathBuf::from("/nonexistent/reniced_test.conf")
    );
    let result = reniced::run(cli);
    assert!(result.is_err());
}

#[test]
fn run_noop_verbose_returns_ok() -> Result<()> {
    let f = write_config("5 doesnotexist_zzz\n")?;
    let cli = Cli {
        command: None,
        noop: true,
        verbose: true,
        threads: false,
        match_target: MatchTarget::Name,
        log: LogTarget::Stderr,
        configfile: Some(f.path().to_path_buf()),
    };
    reniced::run(cli)?;
    Ok(())
}

// ── print_completions() ────────────────────────────────────────────────────

#[test]
fn print_completions_bash_produces_output() {
    let mut cmd = Cli::command();
    let mut buf = Vec::new();
    print_completions(Shell::Bash, &mut cmd, &mut buf);
    assert!(!buf.is_empty());
}

#[test]
fn print_completions_all_shells_produce_output() {
    for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell] {
        let mut cmd = Cli::command();
        let mut buf: Vec<u8> = Vec::new();
        print_completions(shell, &mut cmd, &mut buf);
        assert!(!buf.is_empty(), "shell {shell:?} produced no output");
    }
}

#[test]
fn print_completions_output_contains_binary_name() {
    let mut cmd = Cli::command();
    let mut buf = Vec::new();
    print_completions(Shell::Bash, &mut cmd, &mut buf);
    let output = String::from_utf8_lossy(&buf);
    assert!(output.contains("reniced"), "completions don't reference binary name");
}


// ── run() threads error paths ──────────────────────────────────────────────

#[test]
#[cfg(not(target_os = "linux"))]
fn run_threads_on_non_linux_returns_err() {
    let f = write_config("5 myprocess\n").unwrap();
    let cli = Cli {
        command: None,
        noop: true,
        verbose: false,
        threads: true,
        match_target: MatchTarget::Name,
        log: LogTarget::Stderr,
        configfile: Some(f.path().to_path_buf()),
    };
    let result = reniced::run(cli);
    assert!(result.is_err());
    assert!(format!("{}", result.unwrap_err()).contains("only supported on Linux"));
}

#[test]
#[cfg(target_os = "linux")]
fn run_threads_without_privilege_returns_err() {
    // Only meaningful when not running as root.
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let f = write_config("5 myprocess\n").unwrap();
    let cli = Cli {
        command: None,
        noop: true,
        verbose: false,
        threads: true,
        match_target: MatchTarget::Name,
        log: LogTarget::Stderr,
        configfile: Some(f.path().to_path_buf()),
    };
    let result = reniced::run(cli);
    assert!(result.is_err());
    assert!(format!("{}", result.unwrap_err()).contains("root privileges"));
}

// ── collect_entries() ──────────────────────────────────────────────────────

#[test]
fn collect_entries_returns_current_process() -> Result<()> {
    let mut system = System::new_all();
    system.refresh_all();
    let entries = collect_entries(&system, false)?;
    // The test process itself must appear in the list.
    let self_pid = std::process::id() as i32;
    assert!(
        entries.iter().any(|e| e.pid == self_pid),
        "own pid {self_pid} not found in entries"
    );
    Ok(())
}

#[test]
fn collect_entries_without_threads_has_no_thread_kind() -> Result<()> {
    use reniced::model::ProcessKind;
    let mut system = System::new_all();
    system.refresh_all();
    let entries = collect_entries(&system, false)?;
    // Without include_threads=true, no Thread entries should appear.
    assert!(
        entries.iter().all(|e| matches!(e.kind, ProcessKind::Process)),
        "unexpected Thread entries when include_threads=false"
    );
    Ok(())
}

#[test]
fn collect_entries_entries_have_non_empty_names() -> Result<()> {
    let mut system = System::new_all();
    system.refresh_all();
    let entries = collect_entries(&system, false)?;
    assert!(!entries.is_empty());
    // Every process should have a non-empty name.
    for entry in &entries {
        assert!(!entry.name.is_empty(), "entry pid {} has empty name", entry.pid);
    }
    Ok(())
}
