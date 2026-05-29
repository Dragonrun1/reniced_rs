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

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use log::warn;
use regex::Regex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseRuleError {
    #[error("no recognised actions (nice/oom/ionice) in command '{0}'")]
    NoActions(String),
    #[error("invalid regex '{pattern}': {source}")]
    InvalidRegex {
        pattern: String,
        source: regex::Error,
    },
    #[error("invalid number in command: {0}")]
    InvalidNumber(#[from] std::num::ParseIntError),
}

use crate::cli::Cli;
use crate::model::{IoClass, Rule};
use crate::platform::is_privileged;

pub fn find_rulefile(cli: &Cli) -> Result<PathBuf> {
    find_rulefile_inner(cli.configfile.as_deref(), is_privileged())
}

// Separated from find_rulefile so privilege status can be injected in tests
// without requiring a real OS call or running as root.
pub fn find_rulefile_inner(configfile: Option<&Path>, privileged: bool) -> Result<PathBuf> {
    if let Some(path) = configfile {
        return Ok(path.to_path_buf());
    }

    if privileged {
        return Ok(PathBuf::from("/etc/reniced.conf"));
    }

    let home = std::env::var("HOME").context("HOME environment variable not set")?;

    Ok(PathBuf::from(home).join(".reniced"))
}

pub fn read_rules(path: &Path) -> Result<Vec<Rule>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;

    let mut rules = Vec::new();

    for (idx, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((command, regex_str)) = line.split_once(char::is_whitespace) else {
            warn!(
                "rule line #{} skipped: no command/regex separation found",
                idx + 1
            );
            continue;
        };

        match parse_rule(command, regex_str.trim()) {
            Ok(rule) => rules.push(rule),
            Err(e) => warn!("rule line #{} skipped: {}", idx + 1, e),
        }
    }

    Ok(rules)
}

pub fn parse_rule(command: &str, regex_str: &str) -> Result<Rule, ParseRuleError> {
    let mut command = command.to_string();

    if command.starts_with('-') || command.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        command = format!("n{command}");
    }

    let mut nice = None;
    let mut oom_adj = None;
    let mut io_class = None;
    let mut io_nice = None;

    let token_re = Regex::new(r"(n-?\d+|o-?\d+|[rbi]\d*)").unwrap();

    for token in token_re.find_iter(&command) {
        let token = token.as_str();

        if let Some(rest) = token.strip_prefix('n') {
            nice = Some(rest.parse()?);
            continue;
        }

        if let Some(rest) = token.strip_prefix('o') {
            oom_adj = Some(rest.parse()?);
            continue;
        }

        let mut chars = token.chars();

        // token_re only matches [rbi]\d* after the n/o prefixes are consumed
        // above, so next() always yields r, b, or i here. unreachable! below
        // documents the invariant so any future regex change that breaks it
        // fails loudly in debug builds rather than silently doing the wrong thing.
        let prefix = chars.next().unwrap_or_else(|| {
            unreachable!("token_re matched an empty IO class token");
        });

        let value = chars.as_str();

        match prefix {
            'r' => io_class = Some(IoClass::Realtime),
            'b' => io_class = Some(IoClass::BestEffort),
            'i' => io_class = Some(IoClass::Idle),
            _ => unreachable!(
                "token_re produced IO class prefix {:?} which is not r/b/i — regex and match are out of sync",
                prefix
            ),
        }

        if !value.is_empty() {
            io_nice = Some(value.parse()?);
        }
    }

    if nice.is_none() && oom_adj.is_none() && io_class.is_none() {
        return Err(ParseRuleError::NoActions(command));
    }

    Ok(Rule {
        regex: Regex::new(regex_str).map_err(|e| ParseRuleError::InvalidRegex {
            pattern: regex_str.to_string(),
            source: e,
        })?,
        nice,
        oom_adj,
        io_class,
        io_nice,
    })
}
