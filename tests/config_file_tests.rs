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

//! Tests for read_rules() — config file parsing at the file level.
//! parse_rule() unit tests live in rules.rs; these tests cover the
//! higher-level concerns: comment stripping, blank line handling,
//! multi-rule files, and bad-line skipping.

use std::io::Write;

use anyhow::Result;
use tempfile::NamedTempFile;

use reniced::config::read_rules;

fn write_config(content: &str) -> Result<NamedTempFile> {
    let mut f = NamedTempFile::new()?;
    f.write_all(content.as_bytes())?;
    Ok(f)
}

// ── happy-path parsing ─────────────────────────────────────────────────────

#[test]
fn reads_single_valid_rule() -> Result<()> {
    let f = write_config("5 myprocess\n")?;
    let rules = read_rules(f.path())?;
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].nice, Some(5));
    Ok(())
}

#[test]
fn reads_multiple_rules() -> Result<()> {
    let f = write_config("5 proc_a\n-10 proc_b\nb2 proc_c\n")?;
    let rules = read_rules(f.path())?;
    assert_eq!(rules.len(), 3);
    Ok(())
}

#[test]
fn strips_comment_lines() -> Result<()> {
    let f = write_config("# this is a comment\n5 myprocess\n")?;
    let rules = read_rules(f.path())?;
    assert_eq!(rules.len(), 1);
    Ok(())
}

#[test]
fn strips_blank_lines() -> Result<()> {
    let f = write_config("\n\n5 myprocess\n\n")?;
    let rules = read_rules(f.path())?;
    assert_eq!(rules.len(), 1);
    Ok(())
}

#[test]
fn strips_inline_leading_whitespace() -> Result<()> {
    // Lines indented with spaces should still parse.
    let f = write_config("   5 myprocess\n")?;
    let rules = read_rules(f.path())?;
    assert_eq!(rules.len(), 1);
    Ok(())
}

#[test]
fn empty_file_returns_empty_vec() -> Result<()> {
    let f = write_config("")?;
    let rules = read_rules(f.path())?;
    assert!(rules.is_empty());
    Ok(())
}

#[test]
fn comments_only_returns_empty_vec() -> Result<()> {
    let f = write_config("# comment one\n# comment two\n")?;
    let rules = read_rules(f.path())?;
    assert!(rules.is_empty());
    Ok(())
}

// ── bad-line skipping ──────────────────────────────────────────────────────

#[test]
fn skips_no_action_line_and_continues() -> Result<()> {
    // "xyz proc" has no valid action — should be skipped, not fatal.
    let f = write_config("xyz badline\n5 goodline\n")?;
    let rules = read_rules(f.path())?;
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].nice, Some(5));
    Ok(())
}

#[test]
fn skips_unseparated_line_and_continues() -> Result<()> {
    // A line with no whitespace has no regex — should be skipped, not fatal.
    let f = write_config("noseperator\n5 goodline\n")?;
    let rules = read_rules(f.path())?;
    assert_eq!(rules.len(), 1);
    Ok(())
}

#[test]
fn skips_invalid_regex_line_and_continues() -> Result<()> {
    let f = write_config("5 [invalid\n5 goodline\n")?;
    let rules = read_rules(f.path())?;
    assert_eq!(rules.len(), 1);
    Ok(())
}

#[test]
fn all_bad_lines_returns_empty_vec() -> Result<()> {
    let f = write_config("noseperator\nxyz bad\n")?;
    let rules = read_rules(f.path())?;
    assert!(rules.is_empty());
    Ok(())
}

// ── error cases ────────────────────────────────────────────────────────────

#[test]
fn errors_on_missing_file() {
    let result = read_rules(std::path::Path::new("/nonexistent/path/reniced.conf"));
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("failed to read config file"));
}

// ── find_rulefile ──────────────────────────────────────────────────────────

use reniced::cli::{Cli, LogTarget, MatchTarget};
use reniced::config::find_rulefile;
use std::path::PathBuf;

fn cli_with_config(path: Option<PathBuf>) -> Cli {
    Cli {
        command: None,
        noop: false,
        verbose: false,
        threads: false,
        match_target: MatchTarget::Name,
        log: LogTarget::Stderr,
        configfile: path,
    }
}

#[test]
fn find_rulefile_returns_explicit_path() -> Result<()> {
    let f = write_config("5 myprocess\n")?;
    let cli = cli_with_config(Some(f.path().to_path_buf()));
    let result = find_rulefile(&cli)?;
    assert_eq!(result, f.path());
    Ok(())
}

#[test]
fn find_rulefile_uses_home_when_not_root() -> Result<()> {
    // Non-root: should use ~/.reniced. We can't guarantee the test runner is
    // non-root, so only assert the path ends with ".reniced" when HOME is set
    // and geteuid() != 0.
    if std::env::var("HOME").is_err() {
        return Ok(());
    }
    // Skip if running as root — the root branch takes precedence.
    if unsafe { libc::geteuid() } == 0 {
        return Ok(());
    }
    let cli = cli_with_config(None);
    let result = find_rulefile(&cli)?;
    assert!(
        result.ends_with(".reniced"),
        "expected path ending with .reniced, got {:?}",
        result
    );
    Ok(())
}

#[test]
fn find_rulefile_errors_when_home_missing() {
    if unsafe { libc::geteuid() } == 0 {
        return; // root path is taken first, HOME irrelevant
    }
    // Temporarily unset HOME, test, then restore.
    let original = std::env::var("HOME").ok();
    std::env::remove_var("HOME");
    let cli = cli_with_config(None);
    let result = find_rulefile(&cli);
    if let Some(val) = original {
        std::env::set_var("HOME", val);
    }
    assert!(result.is_err());
    assert!(format!("{}", result.unwrap_err()).contains("HOME"));
}
