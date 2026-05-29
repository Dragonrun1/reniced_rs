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

use std::fs;

use anyhow::Result;

use log::info;

use crate::cli::{Cli, MatchTarget};
use crate::model::{IoClass, ProcessEntry, Rule};
use crate::platform::set_process_priority;

/// Applies a list of configuration [`Rule`]s to a specific [`ProcessEntry`].
///
/// This function iterates through the provided `rules` and attempts to match the process
/// against each rule's regular expression. The field used for matching is determined by
/// `cli.match_target`. If a match is found, the function applies the specified CPU nice value,
/// OOM adjustment, and I/O priority settings.
///
/// # Matching Logic
///
/// The target string for regex matching is selected based on [`MatchTarget`]:
/// - [`MatchTarget::Name`]: Uses `process.name`.
/// - [`MatchTarget::Cmdline`]: Uses `process.cmd` (full command line).
/// - [`MatchTarget::Exe`]: Uses the full executable path (`process.exe`), if available.
/// - [`MatchTarget::ExeBasename`]: Uses only the filename of the executable path, if available.
///
/// If the target field is unavailable (e.g., `exe` is `None`) or the regex does not match,
/// the rule is skipped.
///
/// # Arguments
///
/// * `process` - The process to evaluate and potentially modify.
/// * `rules` - A slice of [`Rule`] definitions containing regex patterns and priority settings.
/// * `cli` - Runtime configuration, including the `match_target` strategy and flags like `noop`/`verbose`.
///
/// # Returns
///
/// * `Ok(())` if all matching rules were successfully applied (or if `noop` mode was active).
/// * `Err(...)` if any underlying priority adjustment function ([`set_priority`], [`adjust_oom`], [`set_io_priority`]) fails.
///
/// # Side Effects
///
/// If a rule matches and `cli.noop` is false, this function modifies the process's:
/// - CPU scheduling priority (nice value).
/// - OOM killer score.
/// - I/O scheduling class and priority.
///
/// # Example
///
/// ```no_run
/// # use your_crate::{ProcessEntry, Rule, Cli, MatchTarget, apply_rules};
/// # use regex::Regex;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let process = ProcessEntry {
///     pid: 1234,
///     name: "firefox".to_string(),
///     cmd: "/usr/bin/firefox --no-remote".to_string(),
///     exe: Some("/usr/bin/firefox".to_string()),
/// };
///
/// let rules = vec![
///     Rule {
///         regex: Regex::new("firefox")?,
///         nice: Some(-5),
///         oom_adj: None,
///         io_class: None,
///         io_nice: None,
///     }
/// ];
///
/// let cli = Cli {
///     match_target: MatchTarget::Name,
///     noop: false,
///     verbose: true,
/// };
///
/// apply_rules(&process, &rules, &cli)?;
/// # Ok(())
/// # }
/// ```
///
/// # Note
///
/// Rules are applied **sequentially**. If multiple rules match the same process,
/// the settings from the **last matching rule** will effectively overwrite previous ones
/// for the same property (e.g., if two rules set `nice`, the second one wins).
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

/// Adjusts the Out-Of-Memory (OOM) score for a specific process by writing to `/proc/[pid]/oom_score_adj`.
///
/// This function converts a legacy `oom_adj` value to the modern `oom_score_adj` scale using
/// [`convert_oom_adj`], then writes the result to the kernel interface. This influences the
/// likelihood of the process being selected by the OOM killer when system memory is exhausted.
///
/// # Arguments
///
/// * `process` - A reference to the [`ProcessEntry`] containing the target PID.
/// * `score` - The legacy OOM adjustment value (typically -17 to 15).
/// * `cli` - A reference to the [`Cli`] configuration.
///   - If `cli.noop` is true, logs the intended action without modifying the file.
///   - If `cli.verbose` is true, logs a confirmation message upon success.
///
/// # Returns
///
/// * `Ok(())` if the value was successfully written (or if `noop` mode was active).
/// * `Err(...)` if the file write fails.
///
/// # Errors
///
/// Returns an `anyhow::Error` if writing to `/proc/[pid]/oom_score_adj` fails. Common causes include:
/// - `ENOENT`: The process ID does not exist (the file path is invalid).
/// - `EACCES`/`EPERM`: Insufficient permissions (usually requires root) to modify another process's OOM score.
/// - `EINVAL`: The converted score is outside the valid range (-1000 to 1000), though [`convert_oom_adj`] should prevent this.
///
/// # Example
///
/// ```no_run
/// # use your_crate::{ProcessEntry, Cli, adjust_oom};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let process = ProcessEntry { pid: 1234, cmd: "my_app".to_string() };
/// let cli = Cli { noop: false, verbose: true };
///
/// // Set OOM score to "immune" (-1000) using legacy value -17
/// adjust_oom(&process, -17, &cli)?;
/// # Ok(())
/// # }
/// ```
///
/// # Platform Specifics
///
/// This function is **Linux-specific**. It relies on the `procfs` filesystem being mounted
/// at `/proc`. The `oom_score_adj` interface replaced the deprecated `oom_adj` in kernel 2.6.36.
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


/// Adjusts the OOM (Out-Of-Memory) score for a specific process by writing to `/proc/[pid]/oom_score_adj`.
///
/// This function converts a legacy `oom_adj` value to the modern `oom_score_adj` scale using
/// [`convert_oom_adj`], then writes the result to the kernel interface. This influences the
/// likelihood of the process being selected by the OOM killer when system memory is exhausted.
///
/// # Arguments
///
/// * `process` - A reference to the [`ProcessEntry`] containing the target PID.
/// * `score` - The legacy OOM adjustment value (typically -17 to 15).
/// * `cli` - A reference to the [`Cli`] configuration.
///   - If `cli.noop` is true, logs the intended action without modifying the file.
///   - If `cli.verbose` is true, logs a confirmation message upon success.
///
/// # Returns
///
/// * `Ok(())` if the value was successfully written (or if `noop` mode was active).
/// * `Err(...)` if the file write fails.
///
/// # Errors
///
/// Returns an `anyhow::Error` if writing to `/proc/[pid]/oom_score_adj` fails. Common causes include:
/// - `ENOENT`: The process ID does not exist (the file path is invalid).
/// - `EACCES`/`EPERM`: Insufficient permissions (usually requires root) to modify another process's OOM score.
/// - `EINVAL`: The converted score is outside the valid range (-1000 to 1000), though [`convert_oom_adj`] should prevent this.
///
/// # Example
///
/// ```no_run
/// # use your_crate::{ProcessEntry, Cli, adjust_oom};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let process = ProcessEntry { pid: 1234, cmd: "my_app".to_string() };
/// let cli = Cli { noop: false, verbose: true };
///
/// // Set OOM score to "immune" (-1000) using legacy value -17
/// adjust_oom(&process, -17, &cli)?;
/// # Ok(())
/// # }
/// ```
///
/// # Platform Specifics
///
/// This function is **Linux-specific**. It relies on the `procfs` filesystem being mounted
/// at `/proc`. The `oom_score_adj` interface replaced the deprecated `oom_adj` in kernel 2.6.36.
fn adjust_oom(process: &ProcessEntry, score: i32, cli: &Cli) -> Result<()> {
    let converted = convert_oom_adj(score);

    if cli.noop {
        info!(
            "would adjust OOM setting of pid {} to {}",
            process.pid, converted
        );

        return Ok(());
    }

    let path = format!("/proc/{}/oom_score_adj", process.pid);

    fs::write(&path, format!("{converted}\n"))
        .map_err(|error| anyhow::anyhow!("failed writing {}: {}", path, error,))?;

    if cli.verbose {
        info!(
            "OOM adjust set to {}: {}/{}",
            converted, process.pid, process.cmd
        );
    }

    Ok(())
}

#[cfg(target_os = "linux")]
mod io_priority {
    use crate::cli::Cli;
    use crate::model::{IoClass, ProcessEntry};
    use anyhow::Result;
    use log::info;

    const IOPRIO_WHO_PROCESS: libc::c_int = 1;

    /// Constructs the priority value argument for the `ioprio_set` syscall.
    ///
    /// Linux I/O priority is encoded into a single integer by combining the
    /// scheduling class and the priority level (data) using bit shifts.
    ///
    /// # Encoding Logic
    ///
    /// The kernel expects the value in the following format:
    /// `value = (class << 13) | data`
    ///
    /// - **Class**: Occupies the upper bits (shifted left by 13).
    ///   - `1`: Realtime
    ///   - `2`: Best Effort
    ///   - `3`: Idle
    /// - **Data**: Occupies the lower 13 bits (0–7 for priority level).
    ///
    /// # Arguments
    ///
    /// * `class` - The I/O scheduling class (1, 2, or 3).
    /// * `data` - The priority level within the class (typically 0–7, where 0 is highest).
    ///
    /// # Returns
    ///
    /// A `libc::c_int` representing the packed priority value suitable for passing
    /// as the `ioprio` argument to `libc::syscall(libc::SYS_ioprio_set, ...)`.
    ///
    /// # Example
    ///
    /// ```
    /// # // Simulating libc::c_int as i32
    /// # fn ioprio_value(class: u16, data: u16) -> i32 {
    /// #     ((class << 13) | data) as i32
    /// # }
    /// // Best Effort (class 2), Level 4
    /// let prio = ioprio_value(2, 4);
    /// assert_eq!(prio, (2 << 13) | 4); // 16388
    ///
    /// // Idle (class 3), Level 0 (highest within Idle)
    /// let idle_prio = ioprio_value(3, 0);
    /// assert_eq!(idle_prio, 3 << 13); // 24576
    /// ```
    ///
    /// # References
    ///
    /// - See `ioprio_set(2)` man page for details on the encoding.
    /// - Kernel source: `include/uapi/linux/ioprio.h` (macro `IOPRIO_PRIO_VALUE`).
    fn ioprio_value(class: u16, data: u16) -> libc::c_int {
        ((class << 13) | data) as libc::c_int
    }

    /// Sets the I/O priority for a specific process using the Linux `ioprio_set` syscall.
    ///
    /// This function assigns an I/O scheduling class and optional priority level to a process,
    /// influencing how the kernel schedules disk I/O operations for that process.
    ///
    /// # Arguments
    ///
    /// * `process` - A reference to the [`ProcessEntry`] containing the target PID and command name.
    /// * `class` - The I/O scheduling class ([`IoClass::Realtime`], [`IoClass::BestEffort`], or [`IoClass::Idle`]).
    /// * `level` - An optional priority level (0-7). If `None`, defaults to 0 (highest priority within the class).
    /// * `cli` - A reference to the [`Cli`] configuration. If `cli.noop` is true, the function logs the intended action without making the syscall.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the priority was successfully set (or if `noop` mode was active).
    /// * `Err(...)` if the underlying syscall fails (e.g., insufficient permissions or invalid PID).
    ///
    /// # Errors
    ///
    /// Returns a `std::io::Error` wrapped in the crate's `Result` type if the `ioprio_set` syscall returns -1.
    /// Common causes include:
    /// - `EPERM`: The caller lacks the `CAP_SYS_NICE` capability.
    /// - `ESRCH`: The process ID does not exist.
    /// - `EINVAL`: Invalid class or priority value.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use your_crate::{ProcessEntry, IoClass, Cli, set_io_priority};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let process = ProcessEntry { pid: 1234, cmd: "my_app".to_string() };
    /// let cli = Cli { noop: false, verbose: true };
    ///
    /// // Set to Best Effort, level 4
    /// set_io_priority(&process, IoClass::BestEffort, Some(4), &cli)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Safety
    ///
    /// This function uses `unsafe` code to invoke a raw system call (`libc::syscall`).
    /// The caller must ensure that the `process.pid` is valid and that the program
    /// has the necessary permissions to modify I/O priority for the target process.
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

        let data = u16::from(level.unwrap_or(0));
        let prio = ioprio_value(class_num, data);

        let result =
            unsafe { libc::syscall(libc::SYS_ioprio_set, IOPRIO_WHO_PROCESS, process.pid, prio) };

        if result != 0 {
            return Err(std::io::Error::last_os_error().into());
        }

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
    use crate::cli::Cli;
    use crate::model::{IoClass, ProcessEntry};
    use crate::platform::set_io_priority as platform_set_io_priority;
    use anyhow::Result;
    use log::info;

    /// Sets the I/O priority for a process on Windows.
    ///
    /// This function maps the generic [`IoClass`] to Windows-specific I/O priority hints
    /// (e.g., "high", "normal", "very-low"). Unlike the Linux implementation, Windows
    /// does not support subclass levels, so the `level` argument is intentionally ignored.
    ///
    /// # Arguments
    ///
    /// * `process` - A reference to the [`ProcessEntry`] containing the target PID and command name.
    /// * `class` - The I/O scheduling class to apply.
    /// * `_level` - An optional priority level. **Ignored on Windows** as the OS does not support fine-grained I/O priority levels within classes.
    /// * `cli` - A reference to the [`Cli`] configuration.
    ///   - If `cli.noop` is true, logs the intended action without modifying the process.
    ///   - If `cli.verbose` is true, logs a confirmation message upon success.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the priority was successfully set (or if `noop` mode was active).
    /// * `Err(...)` if the underlying platform helper (`platform_set_io_priority`) fails.
    ///
    /// # Errors
    ///
    /// Returns an `anyhow::Error` if `platform_set_io_priority` fails. Common causes include:
    /// - Insufficient privileges to modify the target process.
    /// - The target process ID no longer exists.
    /// - Invalid handle access.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use your_crate::{ProcessEntry, IoClass, Cli, set_io_priority};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let process = ProcessEntry { pid: 1234, cmd: "my_app.exe".to_string() };
    /// let cli = Cli { noop: false, verbose: true };
    ///
    /// // Sets to "normal" priority on Windows; level is ignored.
    /// set_io_priority(&process, IoClass::BestEffort, Some(5), &cli)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Platform Specifics
    ///
    /// This implementation is specific to **Windows**. It uses `platform_set_io_priority`
    /// to apply I/O hints. The mapping is as follows:
    /// - [`IoClass::Realtime`] → "high"
    /// - [`IoClass::BestEffort`] → "normal"
    /// - [`IoClass::Idle`] → "very-low (background)"
    pub fn set_io_priority(
        process: &ProcessEntry,
        class: IoClass,
        _level: Option<u8>,
        cli: &Cli,
    ) -> Result<()> {
        // Windows has no subclass level, so _level is intentionally ignored.
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

        platform_set_io_priority(process.pid, class).map_err(|e| {
            anyhow::anyhow!("failed setting IO priority for pid {}: {}", process.pid, e)
        })?;

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
// setiopolicy_np(3) on macOS only affects the calling process, not arbitrary
// pids, so it is not useful for reniced's use case.
#[cfg(all(unix, not(target_os = "linux")))]
mod io_priority {
    use crate::cli::Cli;
    use crate::model::{IoClass, ProcessEntry};
    use anyhow::Result;
    use log::{info, warn};
    use std::sync::OnceLock;

    static IO_PRIO_WARNED: OnceLock<()> = OnceLock::new();

    pub fn set_io_priority(
        _process: &ProcessEntry,
        _class: IoClass,
        _level: Option<u8>,
        _cli: &Cli,
    ) -> Result<()> {
        IO_PRIO_WARNED.get_or_init(|| {
            warn!("IO priority rules are not supported on this platform; IO priority rules will be skipped");
        });
        Ok(())
    }
}

/// Sets the I/O priority for a process by delegating to the platform-specific implementation.
///
/// This is a convenience wrapper that forwards all arguments to [`io_priority::set_io_priority`].
/// It exists to expose the functionality at the crate root or a higher module level without
/// duplicating logic.
///
/// # Arguments
///
/// * `process` - The target process entry.
/// * `class` - The desired I/O scheduling class.
/// * `level` - The optional priority level (ignored on platforms that do not support it, e.g., Windows).
/// * `cli` - Runtime configuration flags (e.g., `noop`, `verbose`).
///
/// # Returns
///
/// Propagates the result from [`io_priority::set_io_priority`]:
/// - `Ok(())` on success.
/// - `Err(...)` if the underlying platform implementation fails.
///
/// # Platform Behavior
///
/// The actual behavior depends on the implementation in the `io_priority` module:
/// - **Linux**: Uses `ioprio_set` syscall; `level` is respected.
/// - **Windows**: Maps `class` to Windows priority hints; `level` is ignored.
///
/// # Example
///
/// ```no_run
/// # use your_crate::{ProcessEntry, IoClass, Cli, set_io_priority};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let process = ProcessEntry { pid: 1234, cmd: "app".to_string() };
/// let cli = Cli { noop: false, verbose: true };
///
/// // Delegates to the platform-specific logic
/// set_io_priority(&process, IoClass::BestEffort, Some(4), &cli)?;
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// * [`io_priority::set_io_priority`] - The canonical implementation containing the actual logic.
fn set_io_priority(
    process: &ProcessEntry,
    class: IoClass,
    level: Option<u8>,
    cli: &Cli,
) -> Result<()> {
    io_priority::set_io_priority(process, class, level, cli)
}

/// Converts a legacy Linux OOM adjustment value (`oom_adj`) to the modern score (`oom_score_adj`).
///
/// Older Linux kernels (pre-2.6.36) used `oom_adj` with a range of **-17 to 15**.
/// Modern kernels use `oom_score_adj` with a range of **-1000 to 1000**.
/// This function performs the linear scaling required to map the legacy value to the modern range.
///
/// # Conversion Logic
///
/// The standard formula used by the kernel is:
/// `oom_score_adj = (oom_adj * 1000) / 17`
///
/// | Legacy `oom_adj` | Modern `oom_score_adj` | Meaning |
/// | :--- | :--- | :--- |
/// | `-17` | `-1000` (Special Case) | **OOM Disabled**. :inlineCitations{data="&#91;&#123;&quot;url&quot;&#58;&quot;https&#58;//unix.stackexchange.com/questions/128013/oom-killer-value-always-one-less-than-set&quot;,&quot;favicon&quot;&#58;&quot;https&#58;//imgs.search.brave.com/qglkVyTbwZb9AC4TkmZO1Sx1ej6YcIjfcZg1CB431FQ/rs&#58;fit&#58;32&#58;32&#58;1&#58;0/g&#58;ce/aHR0cDovL2Zhdmlj/b25zLnNlYXJjaC5i/cmF2ZS5jb20vaWNv/bnMvNTIzMTcxZjI3/MGNlY2FiNjE0NTk5/OGVmZjA2NDM3MmJk/MmE4NDJmMjdlYWFl/YTI0ODY4ZGEzODJk/NTNlMjIzZi91bml4/LnN0YWNrZXhjaGFu/Z2UuY29tLw&quot;,&quot;title&quot;&#58;&quot;kernel&#32;-&#32;OOM&#32;Killer&#32;value&#32;always&#32;one&#32;less&#32;than&#32;set&#32;-&#32;Unix&#32;&amp;&#32;Linux&#32;...&quot;,&quot;snippet&quot;&#58;&quot;…oom_adj&#32;(let's&#32;say&#32;9)&#32;the&#32;kernel&#32;does&#32;this&#58;&#32;oom_adj&#32;=&#32;(oom_adj&#32;*&#32;OOM_SCORE_ADJ_MAX)&#32;/&#32;-OOM_DISABLE;&#32;and&#32;stores&#32;that&#32;to&#32;oom_score_adj.&#32;OOM_SCORE_ADJ_MAX&#32;is&#32;1000&#32;and&#32;OOM_DISABLE&#32;is&#32;-17.&quot;&#125;,&#123;&quot;url&quot;&#58;&quot;https&#58;//docs.rackspace.com/docs/linux-out-of-memory-killer&quot;,&quot;favicon&quot;&#58;&quot;https&#58;//imgs.search.brave.com/2CVpomPUAYwmYg2fuTboICQ1cPCrzAfJi7D8PIGatao/rs&#58;fit&#58;32&#58;32&#58;1&#58;0/g&#58;ce/aHR0cDovL2Zhdmlj/b25zLnNlYXJjaC5i/cmF2ZS5jb20vaWNv/bnMvNjU2MDYzMThk/Zjk5Y2Y1YzFkNjcx/MmE4NDU3NTZiOGMy/OWZhNDA5MzUxY2Fh/ZmJmNDk0ZTY3YzAw/NGVmN2RlMy9kb2Nz/LnJhY2tzcGFjZS5j/b20v&quot;,&quot;title&quot;&#58;&quot;Linux&#32;Out-of-Memory&#32;Killer&#32;-&#32;Rackspace&#32;Technology&quot;,&quot;snippet&quot;&#58;&quot;Because&#32;the&#32;valid&#32;range&#32;for&#32;OOM&#32;Killer&#32;adjustments&#32;is&#32;between&#32;-16&#32;and&#32;+15,&#32;a&#32;setting&#32;of&#32;-17&#32;exempts&#32;a&#32;process&#32;entirely&#32;because&#32;it&#32;falls&#32;outside&#32;the&#32;scope&#32;of&#32;acceptable&#32;integers&#32;for&#32;the&#32;OOM&#32;Killer’s&#32;adjustment&#32;scale.&#32;The&#32;general&#32;rule&#32;i…&quot;&#125;,&#123;&quot;url&quot;&#58;&quot;https&#58;//www.oracle.com/technical-resources/articles/it-infrastructure/dev-oom-killer.html&quot;,&quot;favicon&quot;&#58;&quot;https&#58;//imgs.search.brave.com/8NYWA9PXfntPl-_mw9jwVc6P47zesMKLB9OVUHEHZyQ/rs&#58;fit&#58;32&#58;32&#58;1&#58;0/g&#58;ce/aHR0cDovL2Zhdmlj/b25zLnNlYXJjaC5i/cmF2ZS5jb20vaWNv/bnMvMWE0NDI0ZmE5/OTk3ZWQzYjk3NDI5/YjQ5YWM2ZGIzODU1/MzI2ZjVhNzViNTcy/Zjc0OWViYzcwNjRi/ZWM1NzgzYS93d3cu/b3JhY2xlLmNvbS8&quot;,&quot;title&quot;&#58;&quot;How&#32;to&#32;Configure&#32;the&#32;Linux&#32;Out&#32;of&#32;Memory&#32;Killer&quot;,&quot;snippet&quot;&#58;&quot;…score&#32;of&#32;0&#32;is&#32;an&#32;indication&#32;that&#32;our&#32;process&#32;is&#32;exempt&#32;from&#32;the&#32;OOM&#32;killer.&#32;&#32;The&#32;higher&#32;the&#32;OOM&#32;score,&#32;the&#32;more&#32;likely&#32;a&#32;process&#32;will&#32;be&#32;killed&#32;in&#32;an&#32;OOM&#32;condition.&#92;nThe&#32;OOM&#32;killer&#32;can&#32;be&#32;completely&#32;disabled&#32;with…&quot;&#125;,&#123;&quot;url&quot;&#58;&quot;https&#58;//askubuntu.com/questions/60672/how-do-i-use-oom-score-adj&quot;,&quot;favicon&quot;&#58;&quot;https&#58;//imgs.search.brave.com/bgJnc5WjJoSzuO8tk20YOzhsdxXTr5QmxX_JyU2UxiQ/rs&#58;fit&#58;32&#58;32&#58;1&#58;0/g&#58;ce/aHR0cDovL2Zhdmlj/b25zLnNlYXJjaC5i/cmF2ZS5jb20vaWNv/bnMvNzFkNmY1ODg4/NmIzNWViN2QyYzg0/MzU0MGZhMGIxODY2/YTE4MjVmM2Y4NjUw/Y2FjZTFmYTk4ZmZm/NTk1YWUxMC9hc2t1/YnVudHUuY29tLw&quot;,&quot;title&quot;&#58;&quot;process&#32;-&#32;How&#32;do&#32;I&#32;use&#32;oom_score_adj?&#32;-&#32;Ask&#32;Ubuntu&quot;,&quot;snippet&quot;&#58;&quot;…value&#32;of&#32;-1000&#32;for&#32;oom_score&#32;is&#32;special&#32;because&#32;it&#32;cannot&#32;be&#32;selected&#32;by&#32;OOM&#32;Killer&#32;no&#32;matter&#32;the&#32;computed&#32;value&#32;of&#32;the&#32;above&#32;computation.&#32;In&#32;most&#32;cases&#32;the&#32;resulting&#32;value&#32;would&#32;be&#32;negative&#32;enough&#32;to&#32;not&#32;be&#32;selected&#32;in&#32;any&#32;case&quot;&#125;,&#123;&quot;url&quot;&#58;&quot;https&#58;//poweradm.com/out-of-memory-killer-linux/&quot;,&quot;favicon&quot;&#58;&quot;https&#58;//imgs.search.brave.com/UweyfPeIMD-OG66gb0vboHIgkh5vuoAgYCa2NkBnhsk/rs&#58;fit&#58;32&#58;32&#58;1&#58;0/g&#58;ce/aHR0cDovL2Zhdmlj/b25zLnNlYXJjaC5i/cmF2ZS5jb20vaWNv/bnMvMDBiZGYxODVh/YTM3MDViMTM2YzYw/YjEwMTAzY2RhODUx/ZDNiNzQ2MGI3MzU3/NWM4YjAyMmZiZDc5/YTZhODlmNS9wb3dl/cmFkbS5jb20v&quot;,&quot;title&quot;&#58;&quot;Using&#32;Out&#32;Of&#32;Memory&#32;Killer&#32;on&#32;Linux&#32;-&#32;Power&#32;Sysadmin&#32;Blog&quot;,&quot;snippet&quot;&#58;&quot;…the&#32;oom_adj&#32;file.&#32;For&#32;example&#58;&#32;&#36;&#32;sudo&#32;echo&#32;-5&#32;&gt;&#32;/proc/1764/oom_adj&#32;&#36;&#32;cat&#32;/proc/1764/oom_score&#32;·&#32;If&#32;you&#32;want&#32;to&#32;completely&#32;disable&#32;OOM&#32;Killer&#32;for&#32;a&#32;process,&#32;you&#32;need&#32;to&#32;set&#32;oom_adj&#32;to&#32;-17…&quot;&#125;,&#123;&quot;url&quot;&#58;&quot;http&#58;//www.dbasquare.com/kb/how-to-adjust-oom-score-for-a-process/&quot;,&quot;favicon&quot;&#58;&quot;https&#58;//imgs.search.brave.com/NRmqDJfuxlaPumxBscsbfmKwQloEPn2003XwqWGNfZ0/rs&#58;fit&#58;32&#58;32&#58;1&#58;0/g&#58;ce/aHR0cDovL2Zhdmlj/b25zLnNlYXJjaC5i/cmF2ZS5jb20vaWNv/bnMvNjBkZTEwMDIw/ZmUyMmM2MjIzN2Fm/YjY2MThkYjg3YmRj/MjdiYTkwMWMzMjJi/YzBmNzE0MDk0MTlj/ZTIxMTk5OS93d3cu/ZGJhc3F1YXJlLmNv/bS8&quot;,&quot;title&quot;&#58;&quot;How&#32;to&#32;adjust&#32;OOM&#32;score&#32;for&#32;a&#32;process?&quot;,&quot;snippet&quot;&#58;&quot;…oom_adj.&#92;nFor&#32;modern&#32;kernels&#32;(&gt;=&#32;2.6.29)&#58;&#32;The&#32;file&#32;/proc/&#91;pid&#93;/oom_score_adj&#32;accepts&#32;values&#32;ranging&#32;from&#32;-1000&#32;to&#32;1000.&#92;nFor&#32;older&#32;kernels&#32;(&lt;&#32;2.6.29)&#58;&#32;The&#32;file&#32;/proc/&#91;pid&#93;/oom_adj&#32;accepts&#32;values&#32;from…&quot;&#125;,&#123;&quot;url&quot;&#58;&quot;https&#58;//docs.redhat.com/en/documentation/red_hat_enterprise_linux_for_real_time/8/html/optimizing_rhel_8_for_real_time_for_low_latency_operation/assembly_managing-out-of-memory-states_optimizing-rhel8-for-real-time-for-low-latency-operation&quot;,&quot;favicon&quot;&#58;&quot;https&#58;//imgs.search.brave.com/PoxwbWncSF6uwl-K4SD-0FfkmP68_hzcBTmOAvf_NsE/rs&#58;fit&#58;32&#58;32&#58;1&#58;0/g&#58;ce/aHR0cDovL2Zhdmlj/b25zLnNlYXJjaC5i/cmF2ZS5jb20vaWNv/bnMvNGVmNzg2ZmI3/MzNhZDY3MWVkOWJk/ZGMyOTU3MmQ5MmIz/ZjAxNzIxNjAxNGM0/ODQwMDMxNmMxNTM0/MjlkYzQ1Ni9kb2Nz/LnJlZGhhdC5jb20v&quot;,&quot;title&quot;&#58;&quot;Chapter&#32;15.&#32;Managing&#32;Out&#32;of&#32;Memory&#32;states&quot;,&quot;snippet&quot;&#58;&quot;…oom_score_adj.&#92;n&#92;t&#92;t&#92;t&#92;t&#92;t&#92;nVerification&#92;n&#92;n&#92;t&#92;t&#92;t&#92;t&#92;t&#92;tDisplay&#32;the&#32;current&#32;oom_score&#32;for&#32;the&#32;process.&#92;n&#92;t&#92;t&#92;t&#92;t&#92;t&#92;n&#92;n#&#32;cat&#32;/proc/12465/oom_score&#92;n78&#92;n&#92;n&#92;n&#92;t&#92;t&#92;t&#92;tYou&#32;can&#32;disable&#32;the&#32;oom_killer()&#32;function&#32;for&#32;a&#32;process&#32;by&#32;setting&#32;oom_score_adj&#96;&#32;to&#32;the&#32;reserved&#32;value&#32;of&#32;-17…&quot;&#125;,&#123;&quot;url&quot;&#58;&quot;https&#58;//serverfault.com/questions/442364/disable-linux-kernel-from-killing-postgresql-process&quot;,&quot;favicon&quot;&#58;&quot;https&#58;//imgs.search.brave.com/E98Ig0Ri8bFnVZyxYu-ysBMYaZ-cyEV2x4R-99yU11Q/rs&#58;fit&#58;32&#58;32&#58;1&#58;0/g&#58;ce/aHR0cDovL2Zhdmlj/b25zLnNlYXJjaC5i/cmF2ZS5jb20vaWNv/bnMvNTQxZTY3Yzkz/OWMwNzNmN2FjMjA2/ZDQxZGUzYTBjNzcx/OWY0ZjhkMzkxNmVj/ZDIxM2E4MjYyMGU5/NGVhNzFlOS9zZXJ2/ZXJmYXVsdC5jb20v&quot;,&quot;title&quot;&#58;&quot;oom&#32;-&#32;Disable&#32;Linux&#32;kernel&#32;from&#32;killing&#32;postgresql&#32;process&#32;-&#32;Server&#32;...&quot;,&quot;snippet&quot;&#58;&quot;-17&#32;is&#32;default&#32;in&#32;postgres-9.0&#32;(and&#32;9.1),&#32;i&#32;think&#32;version&#32;8&#32;is&#32;using&#32;too&#32;old&#32;init&#32;script&#32;to&#32;adjust&#32;oom-killer&#32;parameters&quot;&#125;&#93;"} The process is immune to the OOM killer. |
/// | `-16` | `~ -941` | Strongly protected. |
/// | `0` | `0` | Default behavior. |
/// | `15` | `~ 882` | Highly likely to be killed. |
/// | `16+` | `1000` (Capped) | Maximum likelihood of being killed. |
///
/// # Arguments
///
/// * `score` - The legacy `oom_adj` value. Expected range is typically `[-17, 15]`, but values outside this range are handled gracefully.
///
/// # Returns
///
/// The corresponding `oom_score_adj` value in the range `[-1000, 1000]`.
///
/// # Special Cases
///
/// - If `score` is exactly `15` (the legacy maximum), this function returns `1000` explicitly to ensure the highest possible kill priority.
/// - If `score` is `-17` (the legacy "disable" value), the formula `(score * 1000) / 17` naturally results in `-1000`, which correctly disables the OOM killer in modern kernels.
///
/// # Example
///
/// ```
/// # fn convert_oom_adj(score: i32) -> i32 {
/// #     const OOM_ADJUST_MAX: i32 = 15;
/// #     const OOM_SCORE_ADJ_MAX: i32 = 1000;
/// #     const OOM_DISABLE: i32 = -17;
/// #     if score == OOM_ADJUST_MAX {
/// #         OOM_SCORE_ADJ_MAX
/// #     } else {
/// #         (score * OOM_SCORE_ADJ_MAX) / -OOM_DISABLE
/// #     }
/// # }
/// assert_eq!(convert_oom_adj(0), 0);
/// assert_eq!(convert_oom_adj(-17), -1000); // Immune
/// assert_eq!(convert_oom_adj(15), 1000);   // Max priority
/// assert_eq!(convert_oom_adj(10), 588);    // ~588
/// ```
///
/// # References
///
/// - The kernel constant `OOM_DISABLE` is defined as `-17`.
/// - The kernel constant `OOM_SCORE_ADJ_MAX` is defined as `1000`.
/// - See `proc(5)` man page for details on `/proc/[pid]/oom_score_adj`.
pub fn convert_oom_adj(score: i32) -> i32 {
    const OOM_ADJUST_MAX: i32 = 15;
    const OOM_SCORE_ADJ_MAX: i32 = 1000;
    const OOM_DISABLE: i32 = -17;

    if score == OOM_ADJUST_MAX {
        OOM_SCORE_ADJ_MAX
    } else {
        (score * OOM_SCORE_ADJ_MAX) / -OOM_DISABLE
    }
}
