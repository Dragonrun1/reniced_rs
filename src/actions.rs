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

#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

use anyhow::Result;
use log::info;

use crate::cli::{Cli, MatchTarget};
use crate::model::{IoClass, ProcessEntry, Rule};
use crate::platform::set_process_priority;

/// Applies a list of configuration [`Rule`]s to a specific [`ProcessEntry`].
///
/// Iterates through `rules`, matching each against the process using the field
/// selected by `cli.match_target`. On a match, applies any combination of CPU
/// nice, OOM adjustment, and I/O priority settings.
///
/// # Matching Logic
///
/// The target string is selected based on [`MatchTarget`]:
/// - [`MatchTarget::Name`]: `process.name` (limited to 15 chars on Linux).
/// - [`MatchTarget::Cmdline`]: `process.cmd` (full argv joined with spaces).
/// - [`MatchTarget::Exe`]: Full executable path (`process.exe`), if available.
/// - [`MatchTarget::ExeBasename`]: Filename part of the executable path, if available.
///
/// If the target field is `None` (e.g. kernel threads have no exe path), the rule is skipped.
///
/// # Returns
///
/// `Ok(())` if all matching rules were applied (or noop was active).
/// `Err(...)` if any underlying OS adjustment fails.
///
/// # Note
///
/// Rules are applied sequentially. If multiple rules match the same process,
/// later rules overwrite earlier ones for the same property.
pub fn apply_rules(process: &ProcessEntry, rules: &[Rule], cli: &Cli) -> Result<()> {
    for rule in rules {
        let target: Option<&str> = match cli.match_target {
            MatchTarget::Name => Some(&process.name),
            MatchTarget::Cmdline => Some(&process.cmd),
            MatchTarget::Exe => process.exe.as_deref(),
            MatchTarget::ExeBasename => process
                .exe
                .as_deref()
                .and_then(|p| std::path::Path::new(p).file_name())
                .and_then(|n| n.to_str()),
        };

        let Some(target) = target else {
            continue;
        };

        if !rule.regex.is_match(target) {
            continue;
        }

        if let Some(nice) = rule.nice {
            set_priority(process, nice, cli)?;
        }

        if let Some(oom_adj) = rule.oom_adj {
            adjust_oom(process, oom_adj, cli)?;
        }

        if let Some(io_class) = rule.io_class {
            set_io_priority(process, io_class, rule.io_nice, cli)?;
        }
    }

    Ok(())
}

/// Adjusts the CPU scheduling priority (nice value) of a process.
///
/// In noop mode, logs the intended change without calling the OS. In live mode,
/// delegates to [`platform::set_process_priority`].
///
/// # Arguments
///
/// * `process` - The target process.
/// * `nice` - The nice value to apply (-20 highest to 19 lowest).
/// * `cli` - Runtime flags (`noop`, `verbose`).
fn set_priority(process: &ProcessEntry, nice: i32, cli: &Cli) -> Result<()> {
    if cli.noop {
        info!("would set priority of {} to {}", process.pid, nice);
        return Ok(());
    }

    set_process_priority(process.pid, nice)?;

    if cli.verbose {
        info!("nice set to {}: {}/{}", nice, process.pid, process.cmd);
    }

    Ok(())
}

/// Adjusts the OOM killer score for a process.
///
/// In noop mode, logs the intended change. In live mode, delegates to
/// [`platform::linux::adjust_oom`] which writes to `/proc/[pid]/oom_score_adj`.
///
/// This function is a no-op at compile time on non-Linux platforms — rules
/// containing `o` directives are silently ignored where `/proc` is absent.
///
/// # Arguments
///
/// * `process` - The target process.
/// * `score` - The legacy `oom_adj` value (-17 to 15).
/// * `cli` - Runtime flags (`noop`, `verbose`).
fn adjust_oom(process: &ProcessEntry, score: i32, cli: &Cli) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use crate::platform::linux::{adjust_oom as platform_adjust_oom, convert_oom_adj};

        let converted = convert_oom_adj(score);

        if cli.noop {
            info!(
                "would adjust OOM setting of pid {} to {}",
                process.pid, converted
            );
            return Ok(());
        }

        platform_adjust_oom(process.pid, score)
            .map_err(|e| anyhow::anyhow!("OOM adjust failed for pid {}: {}", process.pid, e))?;

        if cli.verbose {
            info!(
                "OOM adjust set to {}: {}/{}",
                converted, process.pid, process.cmd
            );
        }
    }

    // On non-Linux, OOM adjustment is silently skipped — /proc is absent.
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (process, score, cli);
    }

    Ok(())
}

// ── IO priority dispatch ──────────────────────────────────────────────────
//
// Each platform has its own io_priority module with a matching public
// set_io_priority signature. The thin wrapper below lets apply_rules()
// call through without knowing which platform it's on.

#[cfg(target_os = "linux")]
mod io_priority {
    use anyhow::Result;
    use log::info;

    use crate::cli::Cli;
    use crate::model::{IoClass, ProcessEntry};
    use crate::platform::linux::set_io_priority as platform_set_io_priority;

    /// Sets IO priority for a process on Linux, delegating to [`platform::linux::set_io_priority`].
    pub fn set_io_priority(
        process: &ProcessEntry,
        class: IoClass,
        level: Option<u8>,
        cli: &Cli,
    ) -> Result<()> {
        let class_num: u16 = match class {
            IoClass::Realtime => 1,
            IoClass::BestEffort => 2,
            IoClass::Idle => 3,
        };

        if cli.noop {
            match level {
                Some(level) => info!(
                    "would set IO priority for pid {} to class {} level {}",
                    process.pid, class_num, level,
                ),
                None => info!(
                    "would set IO priority for pid {} to class {}",
                    process.pid, class_num,
                ),
            }
            return Ok(());
        }

        platform_set_io_priority(process.pid, class, level)
            .map_err(|e| anyhow::anyhow!("ioprio_set failed for pid {}: {}", process.pid, e))?;

        if cli.verbose {
            let class_name = match class {
                IoClass::Realtime => "realtime",
                IoClass::BestEffort => "best-effort",
                IoClass::Idle => "idle",
            };
            match level {
                Some(level) => info!(
                    "ionice set to {}, class {}: {}/{}",
                    class_name, level, process.pid, process.cmd,
                ),
                None => info!(
                    "ionice set to {}: {}/{}",
                    class_name, process.pid, process.cmd,
                ),
            }
        }

        Ok(())
    }
}

#[cfg(windows)]
mod io_priority {
    use anyhow::Result;
    use log::info;

    use crate::cli::Cli;
    use crate::model::{IoClass, ProcessEntry};
    use crate::platform::set_io_priority as platform_set_io_priority;

    /// Sets IO priority for a process on Windows. The `level` sub-class is ignored.
    pub fn set_io_priority(
        process: &ProcessEntry,
        class: IoClass,
        _level: Option<u8>,
        cli: &Cli,
    ) -> Result<()> {
        let class_name = match class {
            IoClass::Realtime => "high",
            IoClass::BestEffort => "normal",
            IoClass::Idle => "very-low (background)",
        };

        if cli.noop {
            info!(
                "would set IO priority for pid {} to {} (Windows IO hint)",
                process.pid, class_name,
            );
            return Ok(());
        }

        platform_set_io_priority(process.pid, class)
            .map_err(|e| anyhow::anyhow!("IO priority failed for pid {}: {}", process.pid, e))?;

        if cli.verbose {
            info!(
                "IO priority set to {}: {}/{}",
                class_name, process.pid, process.cmd,
            );
        }

        Ok(())
    }
}

// macOS, BSD, and any other non-Linux Unix: warn once and skip.
// setiopolicy_np(3) only affects the calling process so is not useful here.
#[cfg(all(unix, not(target_os = "linux")))]
mod io_priority {
    use anyhow::Result;
    use log::warn;
    use std::sync::OnceLock;

    use crate::cli::Cli;
    use crate::model::{IoClass, ProcessEntry};

    static IO_PRIO_WARNED: OnceLock<()> = OnceLock::new();

    pub fn set_io_priority(
        _process: &ProcessEntry,
        _class: IoClass,
        _level: Option<u8>,
        _cli: &Cli,
    ) -> Result<()> {
        IO_PRIO_WARNED.get_or_init(|| {
            warn!(
                "IO priority rules are not supported on this platform; \
                 IO priority rules will be skipped"
            );
        });
        Ok(())
    }
}

fn set_io_priority(
    process: &ProcessEntry,
    class: IoClass,
    level: Option<u8>,
    cli: &Cli,
) -> Result<()> {
    io_priority::set_io_priority(process, class, level, cli)
}

/// Re-exported for use by tests. The canonical implementation is in [`platform::linux`].
#[cfg(target_os = "linux")]
pub use crate::platform::linux::convert_oom_adj;


/// Stub for non-Linux builds so tests that import `convert_oom_adj` still compile.
#[cfg(not(target_os = "linux"))]
pub fn convert_oom_adj(score: i32) -> i32 {
    // On non-Linux platforms OOM adjustment is not supported.
    // This stub exists solely for cross-platform test compilation.
    const OOM_ADJUST_MAX: i32 = 15;
    const OOM_SCORE_ADJ_MAX: i32 = 1000;
    const OOM_DISABLE: i32 = -17;
    if score == OOM_ADJUST_MAX {
        OOM_SCORE_ADJ_MAX
    } else {
        (score * OOM_SCORE_ADJ_MAX) / -OOM_DISABLE
    }
}
