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

use reniced::config::{parse_rule, ParseRuleError};
use reniced::model::IoClass;

type Result<T> = anyhow::Result<T>;

// ── nice-only rules ────────────────────────────────────────────────────────

#[test]
fn parses_positive_nice() -> Result<()> {
    let rule = parse_rule("5", "myproc")?;
    assert_eq!(rule.nice, Some(5));
    assert!(rule.oom_adj.is_none());
    assert!(rule.io_class.is_none());
    Ok(())
}

#[test]
fn parses_negative_nice() -> Result<()> {
    let rule = parse_rule("-10", "myproc")?;
    assert_eq!(rule.nice, Some(-10));
    Ok(())
}

#[test]
fn parses_explicit_n_prefix() -> Result<()> {
    let rule = parse_rule("n5", "myproc")?;
    assert_eq!(rule.nice, Some(5));
    Ok(())
}

#[test]
fn parses_nice_zero() -> Result<()> {
    let rule = parse_rule("0", "myproc")?;
    assert_eq!(rule.nice, Some(0));
    Ok(())
}

// ── OOM-only rules ─────────────────────────────────────────────────────────

#[test]
fn parses_positive_oom() -> Result<()> {
    let rule = parse_rule("o5", "myproc")?;
    assert_eq!(rule.oom_adj, Some(5));
    assert!(rule.nice.is_none());
    Ok(())
}

#[test]
fn parses_negative_oom() -> Result<()> {
    let rule = parse_rule("o-10", "myproc")?;
    assert_eq!(rule.oom_adj, Some(-10));
    Ok(())
}

// ── IO class rules ─────────────────────────────────────────────────────────

#[test]
fn parses_realtime_with_level() -> Result<()> {
    let rule = parse_rule("r4", "myproc")?;
    assert!(matches!(rule.io_class, Some(IoClass::Realtime)));
    assert_eq!(rule.io_nice, Some(4));
    Ok(())
}

#[test]
fn parses_best_effort_with_level() -> Result<()> {
    let rule = parse_rule("b2", "myproc")?;
    assert!(matches!(rule.io_class, Some(IoClass::BestEffort)));
    assert_eq!(rule.io_nice, Some(2));
    Ok(())
}

#[test]
fn parses_idle_without_level() -> Result<()> {
    let rule = parse_rule("i", "myproc")?;
    assert!(matches!(rule.io_class, Some(IoClass::Idle)));
    assert!(rule.io_nice.is_none());
    Ok(())
}

#[test]
fn parses_idle_with_level() -> Result<()> {
    // Idle class accepts a level even though it is ignored at the syscall layer.
    let rule = parse_rule("i0", "myproc")?;
    assert!(matches!(rule.io_class, Some(IoClass::Idle)));
    assert_eq!(rule.io_nice, Some(0));
    Ok(())
}

// ── combined rules ─────────────────────────────────────────────────────────

#[test]
fn parses_combined_rule() -> Result<()> {
    let rule = parse_rule("n-10r4o5", "^seti")?;
    assert_eq!(rule.nice, Some(-10));
    assert_eq!(rule.oom_adj, Some(5));
    assert!(matches!(rule.io_class, Some(IoClass::Realtime)));
    assert_eq!(rule.io_nice, Some(4));
    Ok(())
}

#[test]
fn parses_nice_and_idle_io() -> Result<()> {
    let rule = parse_rule("5i", "myproc")?;
    assert_eq!(rule.nice, Some(5));
    assert!(matches!(rule.io_class, Some(IoClass::Idle)));
    Ok(())
}

#[test]
fn parses_negative_nice_and_oom() -> Result<()> {
    let rule = parse_rule("-5o-10", "myproc")?;
    assert_eq!(rule.nice, Some(-5));
    assert_eq!(rule.oom_adj, Some(-10));
    Ok(())
}

// ── regex attachment ───────────────────────────────────────────────────────

#[test]
fn rule_regex_matches_correctly() -> Result<()> {
    let rule = parse_rule("5", "^python")?;
    assert!(rule.regex.is_match("python3"));
    assert!(rule.regex.is_match("python"));
    assert!(!rule.regex.is_match("mypython"));
    Ok(())
}

#[test]
fn rule_regex_is_case_sensitive() -> Result<()> {
    let rule = parse_rule("5", "Python")?;
    assert!(rule.regex.is_match("Python3"));
    assert!(!rule.regex.is_match("python3"));
    Ok(())
}

// ── error cases ────────────────────────────────────────────────────────────

#[test]
fn rejects_rule_with_no_actions() {
    let err = parse_rule("xyz", "someprocess").unwrap_err();
    assert!(matches!(err, ParseRuleError::NoActions(_)));
}

#[test]
fn rejects_invalid_regex() {
    let err = parse_rule("5", "[invalid").unwrap_err();
    assert!(matches!(err, ParseRuleError::InvalidRegex { .. }));
}

#[test]
fn rejects_empty_command() {
    // An empty command string has no tokens so no actions are parsed.
    let err = parse_rule("", "myproc").unwrap_err();
    assert!(matches!(err, ParseRuleError::NoActions(_)));
}
