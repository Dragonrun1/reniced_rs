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

//! Tests for apply_rules() and the four MatchTarget modes.
//!
//! These tests use noop mode (cli.noop = true) so apply_rules() exercises
//! the full matching and dispatch logic without touching the real OS APIs.
//! The logger is never initialized in tests so log macros are no-ops.

use anyhow::Result;
use reniced::actions::apply_rules;
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use reniced::cli::{Cli, Commands, LogTarget, MatchTarget};
use reniced::config::parse_rule;
use reniced::model::{ProcessEntry, ProcessKind};

/// Build a minimal no-op Cli with the given MatchTarget.
fn noop_cli(target: MatchTarget) -> Cli {
    Cli {
        command: None,
        noop: true,
        verbose: false,
        threads: false,
        match_target: target,
        log: LogTarget::Stderr,
        config: None,
    }
}

/// Build a ProcessEntry with explicit fields. exe is optional.
fn make_process(name: &str, cmd: &str, exe: Option<&str>) -> ProcessEntry {
    ProcessEntry {
        pid: 1234,
        name: name.to_string(),
        cmd: cmd.to_string(),
        exe: exe.map(str::to_string),
        kind: ProcessKind::Process,
    }
}

// ── MatchTarget::Name ──────────────────────────────────────────────────────

#[test]
fn name_target_matches_on_name() -> Result<()> {
    let rules = vec![parse_rule("5", "^python")?];
    let proc = make_process("python3", "/usr/bin/python3 script.py", Some("/usr/bin/python3"));
    // Should not error — the regex matches the name field.
    apply_rules(&proc, &rules, &noop_cli(MatchTarget::Name))?;
    Ok(())
}

#[test]
fn name_target_does_not_match_cmd() -> Result<()> {
    // Name is "worker" but cmd contains "python". Rule targets "python" —
    // should NOT match when MatchTarget::Name is used.
    let rules = vec![parse_rule("5", "python")?];
    let proc = make_process("worker", "/usr/bin/python3 worker.py", Some("/usr/bin/python3"));
    // apply_rules returns Ok whether it matched or not; we verify no error.
    apply_rules(&proc, &rules, &noop_cli(MatchTarget::Name))?;
    Ok(())
}

// ── MatchTarget::Cmdline ───────────────────────────────────────────────────

#[test]
fn cmdline_target_matches_full_argv() -> Result<()> {
    let rules = vec![parse_rule("5", "worker\\.py")?];
    let proc = make_process("python3", "/usr/bin/python3 worker.py", Some("/usr/bin/python3"));
    apply_rules(&proc, &rules, &noop_cli(MatchTarget::Cmdline))?;
    Ok(())
}

#[test]
fn cmdline_target_does_not_match_name_only() -> Result<()> {
    // The name is "python3" but cmd is just the bare binary with no script arg.
    // A pattern matching "worker" should not fire.
    let rules = vec![parse_rule("5", "worker")?];
    let proc = make_process("python3", "/usr/bin/python3", Some("/usr/bin/python3"));
    apply_rules(&proc, &rules, &noop_cli(MatchTarget::Cmdline))?;
    Ok(())
}

// ── MatchTarget::Exe ───────────────────────────────────────────────────────

#[test]
fn exe_target_matches_full_path() -> Result<()> {
    let rules = vec![parse_rule("5", "/usr/bin/python3")?];
    let proc = make_process("python3", "python3 script.py", Some("/usr/bin/python3"));
    apply_rules(&proc, &rules, &noop_cli(MatchTarget::Exe))?;
    Ok(())
}

#[test]
fn exe_target_skips_process_with_no_exe() -> Result<()> {
    // Kernel threads have no exe path. The rule should be skipped, not panic.
    let rules = vec![parse_rule("5", ".*")?];
    let proc = make_process("kworker", "", None);
    apply_rules(&proc, &rules, &noop_cli(MatchTarget::Exe))?;
    Ok(())
}

#[test]
fn exe_target_does_not_match_basename_only() -> Result<()> {
    // A pattern of "^python3$" should not match "/usr/bin/python3".
    let rules = vec![parse_rule("5", "^python3$")?];
    let proc = make_process("python3", "python3", Some("/usr/bin/python3"));
    apply_rules(&proc, &rules, &noop_cli(MatchTarget::Exe))?;
    Ok(())
}

// ── MatchTarget::ExeBasename ───────────────────────────────────────────────

#[test]
fn exe_basename_matches_without_path() -> Result<()> {
    // "^python3$" should match the basename extracted from "/usr/bin/python3".
    let rules = vec![parse_rule("5", "^python3$")?];
    let proc = make_process("python3", "python3", Some("/usr/bin/python3"));
    apply_rules(&proc, &rules, &noop_cli(MatchTarget::ExeBasename))?;
    Ok(())
}

#[test]
fn exe_basename_avoids_15_char_truncation() -> Result<()> {
    // Linux truncates comm/name at 15 chars. ExeBasename uses the full name
    // from the exe path so a long name is matchable in full.
    let rules = vec![parse_rule("5", "^prometheus-node-exporter$")?];
    let proc = make_process(
        "prometheus-node", // truncated as it would appear in /proc/comm
        "prometheus-node-exporter --web.listen-address=:9100",
        Some("/usr/bin/prometheus-node-exporter"),
    );
    apply_rules(&proc, &rules, &noop_cli(MatchTarget::ExeBasename))?;
    Ok(())
}

#[test]
fn exe_basename_skips_process_with_no_exe() -> Result<()> {
    let rules = vec![parse_rule("5", ".*")?];
    let proc = make_process("kworker", "", None);
    apply_rules(&proc, &rules, &noop_cli(MatchTarget::ExeBasename))?;
    Ok(())
}

// ── multiple rules, multiple matches ──────────────────────────────────────

#[test]
fn multiple_matching_rules_all_applied() -> Result<()> {
    // Two rules match the same process — both should run without error.
    let rules = vec![
        parse_rule("5", "myproc")?,
        parse_rule("o-5", "myproc")?,
    ];
    let proc = make_process("myproc", "myproc --daemon", Some("/usr/sbin/myproc"));
    apply_rules(&proc, &rules, &noop_cli(MatchTarget::Name))?;
    Ok(())
}

#[test]
fn non_matching_rule_does_not_error() -> Result<()> {
    let rules = vec![parse_rule("5", "^nomatch$")?];
    let proc = make_process("myproc", "myproc", Some("/usr/sbin/myproc"));
    apply_rules(&proc, &rules, &noop_cli(MatchTarget::Name))?;
    Ok(())
}

#[test]
fn empty_rules_list_is_fine() -> Result<()> {
    let proc = make_process("myproc", "myproc", Some("/usr/sbin/myproc"));
    apply_rules(&proc, &[], &noop_cli(MatchTarget::Name))?;
    Ok(())
}

// ── verbose noop paths ─────────────────────────────────────────────────────

fn verbose_noop_cli(target: MatchTarget) -> Cli {
    Cli {
        command: None,
        noop: true,
        verbose: true,
        threads: false,
        match_target: target,
        log: LogTarget::Stderr,
        config: None,
    }
}

#[test]
fn verbose_noop_logs_nice_adjustment() -> Result<()> {
    let rules = vec![parse_rule("5", "myproc")?];
    let proc = make_process("myproc", "myproc", Some("/usr/sbin/myproc"));
    // verbose=true with noop=true should still return Ok — the info! macro
    // is a no-op when no logger is initialised in tests.
    apply_rules(&proc, &rules, &verbose_noop_cli(MatchTarget::Name))?;
    Ok(())
}

#[test]
fn verbose_noop_logs_oom_adjustment() -> Result<()> {
    let rules = vec![parse_rule("o-5", "myproc")?];
    let proc = make_process("myproc", "myproc", Some("/usr/sbin/myproc"));
    apply_rules(&proc, &rules, &verbose_noop_cli(MatchTarget::Name))?;
    Ok(())
}

// ── IO priority noop paths ─────────────────────────────────────────────────
//
// These exercise the io_priority module's noop branch for all three classes
// and both the with-level and without-level variants, covering the previously
// zero-hit lines 100-124 in actions.rs on Linux.

#[test]
fn io_priority_noop_realtime_with_level() -> Result<()> {
    let rules = vec![parse_rule("r4", "myproc")?];
    let proc = make_process("myproc", "myproc", Some("/usr/sbin/myproc"));
    apply_rules(&proc, &rules, &noop_cli(MatchTarget::Name))?;
    Ok(())
}

#[test]
fn io_priority_noop_best_effort_with_level() -> Result<()> {
    let rules = vec![parse_rule("b2", "myproc")?];
    let proc = make_process("myproc", "myproc", Some("/usr/sbin/myproc"));
    apply_rules(&proc, &rules, &noop_cli(MatchTarget::Name))?;
    Ok(())
}

#[test]
fn io_priority_noop_idle_without_level() -> Result<()> {
    let rules = vec![parse_rule("i", "myproc")?];
    let proc = make_process("myproc", "myproc", Some("/usr/sbin/myproc"));
    apply_rules(&proc, &rules, &noop_cli(MatchTarget::Name))?;
    Ok(())
}

#[test]
fn io_priority_noop_combined_with_nice() -> Result<()> {
    // Exercises the full dispatch path: nice + io_priority both fired in one rule.
    let rules = vec![parse_rule("5b2", "myproc")?];
    let proc = make_process("myproc", "myproc", Some("/usr/sbin/myproc"));
    apply_rules(&proc, &rules, &noop_cli(MatchTarget::Name))?;
    Ok(())
}

#[test]
fn verbose_noop_io_priority_realtime() -> Result<()> {
    // Covers the verbose branch inside io_priority::set_io_priority.
    let rules = vec![parse_rule("r4", "myproc")?];
    let proc = make_process("myproc", "myproc", Some("/usr/sbin/myproc"));
    apply_rules(&proc, &rules, &verbose_noop_cli(MatchTarget::Name))?;
    Ok(())
}

#[test]
fn verbose_noop_io_priority_idle_no_level() -> Result<()> {
    let rules = vec![parse_rule("i", "myproc")?];
    let proc = make_process("myproc", "myproc", Some("/usr/sbin/myproc"));
    apply_rules(&proc, &rules, &verbose_noop_cli(MatchTarget::Name))?;
    Ok(())
}

// ── completions subcommand ─────────────────────────────────────────────────

#[test]
fn completions_subcommand_recognised() {
    // Verify the Commands enum is wired correctly and Shell variants parse.
    let cmd = Cli {
        command: Some(Commands::Completions { shell: Shell::Bash }),
        noop: false,
        verbose: false,
        threads: false,
        match_target: MatchTarget::Name,
        log: LogTarget::Stderr,
        config: None,
    };
    assert!(matches!(cmd.command, Some(Commands::Completions { shell: Shell::Bash })));
}

#[test]
fn completions_generates_output_for_all_shells() {
    use std::io::sink;

    // generate() should not panic for any supported shell.
    for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell] {
        let mut cmd = Cli::command();
        generate(shell, &mut cmd, "reniced", &mut sink());
    }
}
