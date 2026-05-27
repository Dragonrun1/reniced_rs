use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use nix::unistd::Uid;
use regex::Regex;

use crate::cli::Cli;
use crate::model::{IoClass, Rule};

pub fn find_rulefile(cli: &Cli) -> Result<PathBuf> {
    if let Some(path) = &cli.configfile {
        return Ok(path.clone());
    }

    if Uid::effective().is_root() {
        return Ok(PathBuf::from("/etc/reniced.conf"));
    }

    let home = std::env::var("HOME")
        .context("HOME environment variable not set")?;

    Ok(PathBuf::from(home).join(".reniced"))
}

pub fn read_rules(path: &Path) -> Result<Vec<Rule>> {
    let content = fs::read_to_string(path)
        .with_context(|| {
            format!(
                "failed to read config file {}",
                path.display()
            )
        })?;

    let mut rules = Vec::new();

    for (idx, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((command, regex_str)) =
            line.split_once(char::is_whitespace)
        else {
            eprintln!(
                "invalid rule line {}: {}",
                idx + 1,
                line
            );
            continue;
        };

        let rule = parse_rule(
            command,
            regex_str.trim(),
        )
        .with_context(|| {
            format!("failed parsing rule {}", idx + 1)
        })?;

        rules.push(rule);
    }

    Ok(rules)
}

pub fn parse_rule(
    command: &str,
    regex_str: &str,
) -> Result<Rule> {
    let mut command = command.to_string();

    if command.starts_with('-')
        || command
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        command = format!("n{command}");
    }

    let mut nice = None;
    let mut oom_adj = None;
    let mut io_class = None;
    let mut io_nice = None;

    let token_re = Regex::new(
        r"(n-?\d+|o-?\d+|[rbi]\d*)",
    )?;

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

        let prefix = chars
            .next()
            .context("missing IO class")?;

        let value = chars.as_str();

        match prefix {
            'r' => io_class = Some(IoClass::Realtime),
            'b' => io_class = Some(IoClass::BestEffort),
            'i' => io_class = Some(IoClass::Idle),
            _ => bail!("unknown IO class"),
        }

        if !value.is_empty() {
            io_nice = Some(value.parse()?);
        }
    }

    Ok(Rule {
        regex: Regex::new(regex_str)?,
        nice,
        oom_adj,
        io_class,
        io_nice,
    })
}
