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

use crate::cli::Cli;
use crate::model::{IoClass, Rule};
use crate::platform::is_privileged;
use anyhow::{Context, Result};
use log::warn;
use regex::Regex;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur while parsing a priority rule command string.
///
/// This enum covers validation failures for the command syntax, regular expression compilation,
/// and integer parsing within tokens (e.g., nice values, OOM scores, I/O levels).
#[derive(Debug, Error)]
pub enum ParseRuleError {
    /// Indicates that the command string contained no valid action tokens.
    ///
    /// A valid rule must specify at least one of:
    /// - Nice value (`n...`)
    /// - OOM adjustment (`o...`)
    /// - I/O priority class (`r`, `b`, or `i`)
    ///
    /// The attached `String` contains the original command that failed validation.
    #[error("no recognised actions (nice/oom/ionice) in command '{0}'")]
    NoActions(String),

    /// Indicates that the provided regular expression pattern is invalid.
    ///
    /// This occurs when `regex::Regex::new` fails to compile the pattern string.
    ///
    /// # Fields
    ///
    /// * `pattern`: The original regex string that failed to compile.
    /// * `source`: The underlying `regex::Error` detailing the syntax issue.
    #[error("invalid regex '{pattern}': {source}")]
    InvalidRegex {
        pattern: String,
        source: regex::Error,
    },

    /// Indicates that a numeric value within the command string could not be parsed.
    ///
    /// This applies to:
    /// - Nice values (e.g., `nabc`)
    /// - OOM adjustments (e.g., `oxyz`)
    /// - I/O priority levels (e.g., `b99` where parsing fails)
    ///
    /// The `#[from]` attribute automatically implements `From<std::num::ParseIntError>`
    /// for this variant, allowing seamless propagation using the `?` operator.
    #[error("invalid number in command: {0}")]
    InvalidNumber(#[from] std::num::ParseIntError),
}

/// Resolves the path to the configuration file based on CLI arguments and current privileges.
///
/// This is the public wrapper around [`find_rulefile_inner`]. It automatically detects
/// whether the process is running with elevated privileges (via [`is_privileged`]) and
/// passes this status along with any explicit `--config` path from the CLI.
///
/// # Resolution Logic
///
/// 1. **CLI Override**: If `cli.configfile` is set, that path is used immediately.
/// 2. **Privilege Check**:
///    - If running as **root/admin**: Defaults to `/etc/reniced.conf`.
///    - If running as **standard user**: Defaults to `$HOME/.reniced`.
///
/// # Arguments
///
/// * `cli` - A reference to the parsed command-line arguments ([`Cli`]).
///
/// # Returns
///
/// * `Ok(PathBuf)` containing the resolved configuration file path.
/// * `Err(anyhow::Error)` if the path cannot be resolved (e.g., `HOME` is missing for unprivileged users).
///
/// # Example
///
/// ```no_run
/// # use your_crate::{Cli, find_rulefile};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let cli = Cli::parse(); // Assume clap or similar
/// let config_path = find_rulefile(&cli)?;
/// println!("Using config: {}", config_path.display());
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// * [`find_rulefile_inner`] - The core logic allowing manual injection of privilege status for testing.
/// * [`is_privileged`] - The platform-specific check for root/admin rights.
pub fn find_rulefile(cli: &Cli) -> Result<PathBuf> {
    find_rulefile_inner(cli.config.as_deref(), is_privileged())
}

/// Determines the path to the configuration (rules) file based on explicit arguments and privilege status.
///
/// This function implements the following resolution order:
/// 1. **Explicit Path**: If `configfile` is provided, it is used directly.
/// 2. **System-Wide**: If running as privileged (`privileged` is `true`), defaults to `/etc/reniced.conf`.
/// 3. **User-Specific**: If unprivileged, defaults to `$HOME/.reniced`.
///
/// This logic is separated from the main entry point to allow injecting the `privileged` status
/// during testing, avoiding the need for actual root permissions or OS calls.
///
/// # Arguments
///
/// * `configfile` - An optional explicit path to the configuration file. If `Some`, this takes precedence.
/// * `privileged` - A boolean indicating if the process has root/administrator privileges.
///   - `true`: Resolves to the system-wide path (`/etc/reniced.conf`).
///   - `false`: Resolves to the user-specific path (`$HOME/.reniced`).
///
/// # Returns
///
/// * `Ok(PathBuf)` containing the resolved absolute path to the configuration file.
/// * `Err(anyhow::Error)` if:
///   - `configfile` is `None`.
///   - `privileged` is `false`.
///   - The `HOME` environment variable is not set (required for user-specific resolution).
///
/// # Errors
///
/// Returns a context-rich error if the `HOME` environment variable is missing when attempting
/// to resolve the user-specific path.
///
/// # Example
///
/// ```
/// # use your_crate::find_rulefile_inner;
/// # use std::path::{Path, PathBuf};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Explicit path takes precedence
/// let path = find_rulefile_inner(Some(Path::new("./custom.conf")), false)?;
/// assert_eq!(path, PathBuf::from("./custom.conf"));
///
/// // Privileged mode -> system config
/// let sys_path = find_rulefile_inner(None, true)?;
/// assert_eq!(sys_path, PathBuf::from("/etc/reniced.conf"));
///
/// // Unprivileged mode -> user config (requires HOME to be set)
/// // let user_path = find_rulefile_inner(None, false)?;
/// # Ok(())
/// # }
/// ```
///
/// # Platform Specifics
///
/// - **Unix-like**: Uses `/etc/reniced.conf` for system-wide and `$HOME/.reniced` for user-specific.
/// - **Windows**: If ported, the `privileged` branch would likely point to a path like `C:\ProgramData\reniced\config.toml`,
///   and the user branch to `%APPDATA%`. The current implementation assumes Unix-style paths.
pub fn find_rulefile_inner(config: Option<&Path>, privileged: bool) -> Result<PathBuf> {
    if let Some(path) = config {
        return Ok(path.to_path_buf());
    }

    if privileged {
        return Ok(PathBuf::from("/etc/reniced.conf"));
    }

    let home = std::env::var("HOME").context("HOME environment variable not set")?;

    Ok(PathBuf::from(home).join(".reniced"))
}

/// Reads and parses a configuration file containing process priority rules.
///
/// The file format expects one rule per line with the syntax:
/// `<command> <regex_pattern>`
///
/// - **Command**: A string containing priority tokens (e.g., `n-5 b4`, `o-17`, `r0`).
///   See [`parse_rule`] for detailed syntax.
/// - **Regex Pattern**: A regular expression used to match process names or command lines.
///
/// # File Format Rules
///
/// - **Comments**: Lines starting with `#` are ignored.
/// - **Empty Lines**: Blank lines or lines containing only whitespace are skipped.
/// - **Separation**: The command and regex must be separated by at least one whitespace character.
/// - **Error Handling**:
///   - If a line lacks whitespace separation, it is skipped with a warning.
///   - If [`parse_rule`] fails (e.g., invalid tokens or regex), the specific line is skipped with a warning.
///   - Only valid rules are included in the returned vector; the function does **not** fail on individual line errors.
///
/// # Arguments
///
/// * `path` - The path to the configuration file.
///
/// # Returns
///
/// * `Ok(Vec<Rule>)` containing all successfully parsed rules.
/// * `Err(anyhow::Error)` if the file cannot be read (e.g., missing file, permission denied).
///
/// # Errors
///
/// The function returns an error **only** if the file itself cannot be accessed or read.
/// Individual parsing errors for specific lines are logged as warnings and do not abort the process.
///
/// # Example
///
/// Assuming a config file `rules.conf`:
/// ```text
/// # Set firefox to lower CPU priority
/// n10 ^firefox
///
/// # Set database to high IO priority
/// b0 ^postgres
///
/// # Invalid line (no space) - will be skipped
/// n5^invalid
/// ```
///
/// ```no_run
/// # use your_crate::read_rules;
/// # use std::path::Path;
/// let rules = read_rules(Path::new("rules.conf"))?;
/// println!("Loaded {} valid rules", rules.len());
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// # Note
///
/// Line numbers in warning logs are 1-based (matching standard editor conventions),
/// derived from `enumerate()` + 1.
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

/// Parses a rule definition string into a [`Rule`] struct.
///
/// This function interprets a compact command syntax to configure process priorities.
/// It extracts CPU nice values, OOM adjustments, and I/O priority classes/levels from
/// the `command` string, and compiles the `regex_str` into a [`Regex`] for process matching.
///
/// # Command Syntax
///
/// The `command` string consists of space-separated tokens:
/// - **Nice Value**: `n<value>` (e.g., `n10`, `n-5`).
///   - *Normalization*: If the command starts with a digit or `-` (e.g., `10` or `-5`),
///     it is automatically prefixed with `n` to become `n10` or `n-5`.
/// - **OOM Adjustment**: `o<value>` (e.g., `o-17`, `o15`).
/// - **I/O Priority**: `<class>[level]`
///   - `r[level]`: Realtime (e.g., `r`, `r0`).
///   - `b[level]`: Best Effort (e.g., `b`, `b4`).
///   - `i[level]`: Idle (e.g., `i`, `i7`).
///   - If `level` is omitted, it defaults to `None` in the resulting [`Rule`].
///
/// # Arguments
///
/// * `command` - The string containing priority tokens (e.g., `"n-5 b4"`).
/// * `regex_str` - The regular expression pattern used to match process names/commands.
///
/// # Returns
///
/// * `Ok(Rule)` if the command is valid and contains at least one action.
/// * `Err(ParseRuleError)` if:
///   - No valid action tokens are found (`NoActions`).
///   - The `regex_str` is invalid (`InvalidRegex`).
///   - A token value fails to parse as an integer (`ParseIntError` wrapped in the error type).
///
/// # Validation
///
/// - **Action Requirement**: At least one of `nice`, `oom_adj`, or `io_class` must be specified.
///   If the command contains no valid tokens, `ParseRuleError::NoActions` is returned.
/// - **Regex Invariant**: The internal token regex `r"(n-?\d+|o-?\d+|[rbi]\d*)"` guarantees that
///   I/O tokens start with `r`, `b`, or `i`. The code uses `unreachable!()` to enforce this invariant;
///   if the regex changes, this will panic in debug builds rather than silently misbehaving.
///
/// # Example
///
/// ```
/// # use your_crate::{parse_rule, Rule, IoClass};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Parse a rule: Nice -10, Best Effort IO level 4
/// let rule = parse_rule("n-10 b4", "^firefox.*")?;
///
/// assert_eq!(rule.nice, Some(-10));
/// assert_eq!(rule.io_class, Some(IoClass::BestEffort));
/// assert_eq!(rule.io_nice, Some(4));
///
/// // Normalization: bare number becomes nice value
/// let rule2 = parse_rule("5", "bash")?;
/// assert_eq!(rule2.nice, Some(5));
/// # Ok(())
/// # }
/// ```
///
/// # Error Handling
///
/// - Invalid integers in tokens (e.g., `nabc`) will propagate a parse error.
/// - Invalid regex patterns are wrapped in `ParseRuleError::InvalidRegex` with the original pattern string.
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
