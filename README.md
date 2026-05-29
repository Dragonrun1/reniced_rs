# reniced

`reniced` is a small Rust utility that automatically adjusts process
priorities using regex-based rules.

It can:

- Change CPU scheduling priority (`nice`)
- Adjust Linux OOM killer scores (`oom_score_adj`)
- Set Linux I/O priorities (`ionice`-style classes)
- Set Windows I/O priority hints via `NtSetInformationProcess`
- Match against process names, command lines, or executable paths
- Run safely in dry-run mode before making changes
- Generate shell completions

The tool is intended for systems where certain workloads should always
run with predictable scheduling behavior.

Examples:

- Lower the priority of background encoders
- Raise the priority of interactive audio processes
- Reduce I/O impact from backup jobs
- Make important daemons less likely to be killed under memory pressure

---

## Platform Support

`reniced` supports multiple operating systems, but available features
depend on the platform.

| Feature                      | Linux | BSD | macOS | Windows         |
|------------------------------|-------|-----|-------|-----------------|
| Process nice adjustment      | Yes   | Yes | Yes   | Yes (mapped)    |
| I/O priority control         | Yes   | No  | No    | Yes (no levels) |
| OOM score adjustment         | Yes   | No  | No    | No              |
| Thread/task matching         | Yes   | No  | No    | No              |
| Syslog/Event Log integration | Yes   | Yes | Yes   | Yes             |
| Regex process matching       | Yes   | Yes | Yes   | Yes             |

---

# Cross-Platform Features

The following functionality works consistently across Linux, BSD,
macOS, and Windows unless otherwise noted.

## Process Matching

Rules use Rust regular expressions to match processes.

Supported match targets:

- Process names
- Command lines
- Executable paths
- Executable base names

---

## Nice Priority Adjustment

`reniced` can change CPU scheduling priority.

Typical Unix-style nice ranges:

```text
-20 (highest priority)
  0 (default)
 19 (lowest priority)
```

Notes:

- Lower values receive more CPU time
- Negative priorities usually require elevated privileges
- Windows internally maps priorities to discrete priority classes

---

## Logging

All supported platforms provide:

- Terminal logging (`stderr`)
- System logging integration

Platform-specific backends:

| Platform | System Logger                   |
|----------|---------------------------------|
| Linux    | syslog/journald                 |
| BSD      | syslog                          |
| macOS    | syslog / unified logging bridge |
| Windows  | Event Log                       |

---

# Linux Features

Linux has the most complete feature support.

## I/O Priority Control

Linux supports `ionice`-style I/O scheduling classes.

Available classes:

- Realtime (`r`)
- Best-effort (`b`)
- Idle (`i`)

These settings affect disk scheduling behavior.

---

## OOM (Out Of Memory) Killer Adjustment

Linux supports adjusting `oom_score_adj`.

This controls how likely a process is to be terminated during memory
pressure.

Example:

```text
o-17 important_service
```

Lower values make a process less likely to be killed.

The tool converts legacy Linux `oom_adj` style values into modern
`oom_score_adj` values automatically using the formula
`oom_score_adj = (oom_adj × 1000) / 17`, with the special case that
the legacy maximum of `15` maps exactly to `1000`.

For more information about OOM see:

```bash
man proc_pid_oom_score_adj
```

or [proc_pid_oom_score_adj(5)][oom_score_adj]

---

## Thread/Task Matching

Linux can optionally process threads/tasks individually:

```bash
reniced --threads
```

Useful for:

- Audio workloads
- Worker thread pools
- Applications with named helper threads

Requires elevated privileges.

---

## Linux Permissions

Some operations require:

- Root access
- `CAP_SYS_NICE`
- `CAP_SYS_RESOURCE`
- PAM limits configuration
- Appropriate cgroup/systemd policy

Without permission, operations fail gracefully with warnings.

---

# BSD and macOS Notes

BSD systems and macOS support nice adjustment and regex matching, but
do not expose Linux-specific APIs such as:

- `ionice`
- `oom_score_adj`
- Linux task/thread enumeration

Configuration files remain compatible across platforms, but unsupported
operations are ignored with warnings. A warning is emitted once per run
when any IO priority rule is skipped.

Example:

```text
n-5r2 myprocess
```

On macOS/BSD:

- `n-5` is applied
- `r2` is ignored with a warning

---

# Windows Notes

## Nice Priority

Windows uses priority classes internally instead of Unix nice values.

`reniced` maps configured nice values into Windows process priority
classes via `SetPriorityClass`. Input values are clamped to the valid
nice range `[-20, 19]` before mapping.

| Nice Range     | Windows Priority Class          | Constant Value | Description                                                            |
|:---------------|:--------------------------------|:---------------|:-----------------------------------------------------------------------|
| `-20` to `-16` | [`REALTIME_PRIORITY_CLASS`]     | `0x00000100`   | Highest priority. Can starve system threads; use with extreme caution. |
| `-15` to `-9`  | [`HIGH_PRIORITY_CLASS`]         | `0x00000080`   | For time-critical tasks. Preempts normal/idle processes.               |
| `-8` to `-1`   | [`ABOVE_NORMAL_PRIORITY_CLASS`] | `0x00008000`   | Higher than normal, but below high.                                    |
| `0` to `4`     | [`NORMAL_PRIORITY_CLASS`]       | `0x00000020`   | Default priority for most processes.                                   |
| `5` to `10`    | [`BELOW_NORMAL_PRIORITY_CLASS`] | `0x00004000`   | Lower than normal, suitable for background tasks.                      |
| `11` to `19`   | [`IDLE_PRIORITY_CLASS`]         | `0x00000040`   | Lowest priority. Runs only when system is idle.                        |

Because Windows uses discrete classes, multiple nice values map to the
same class (e.g., both `-10` and `-15` map to `HIGH_PRIORITY_CLASS`).

Note: mapping to `REALTIME_PRIORITY_CLASS` (nice ≤ -16) can make the
system unresponsive if the process consumes significant CPU, as it
preempts critical OS threads. Use only for brief, critical operations.

---

## I/O Priority

Windows I/O priority is supported via the undocumented
`NtSetInformationProcess` API from `ntdll.dll` with the
`ProcessIoPriority` information class. The three ionice classes are
mapped to Windows `IoPriorityHint` values as follows:

| ionice class      | Windows I/O priority hint | Notes                                         |
|:------------------|:--------------------------|:----------------------------------------------|
| Realtime (`r`)    | `High`                    | Best available equivalent; requires elevation |
| Best-effort (`b`) | `Normal`                  | Windows default; level value is ignored       |
| Idle (`i`)        | `VeryLow`                 | Background I/O only                           |

The numeric level sub-class (e.g., `r4` or `b2`) is accepted in the
config file but has no effect on Windows — there are no sub-levels in
the Windows I/O hint model.

Windows does not support:

- `oom_score_adj`
- Linux thread enumeration APIs

System logging uses the Windows Event Log.

---

# Features

- Regex-driven matching
- Multiple actions per rule
- Linux thread/task support
- Syslog/Event Log integration
- Minimal dependencies and fast startup
- Cross-platform build support

---

## Installation

### From source

Requires Rust 1.81+ (edition 2021).

```bash
cargo build --release
```

Binary output:

```text
target/release/reniced
```

Install locally:

```bash
cargo install --path .
```

---

## Basic Usage

Run using the default config file:

```bash
reniced
```

Run in dry-run mode:

```bash
reniced -n
```

Use a custom config file:

```bash
reniced --config ./reniced.conf
```

Enable verbose logging:

```bash
reniced -v
```

---

## Rule File Locations

`reniced` chooses the config file based on privilege level.

### Running as root

```text
/etc/reniced.conf
```

### Running as a normal user

```text
~/.reniced
```

You can always override the location with `--config <PATH>`.

---

# Command Line Options

```text
Usage: reniced [OPTIONS] [COMMAND]
       reniced completions <SHELL>
```

## General Options

| Option                          | Description                                                                              |
|---------------------------------|------------------------------------------------------------------------------------------|
| `-n`, `--dry-run`               | Dry-run mode. Show what would happen without changing priorities.                        |
| `-v`, `--verbose`               | Verbose logging. Logs successful adjustments in addition to warnings/errors.             |
| `-t`, `--threads`               | Include Linux threads/tasks in addition to processes. Requires root privileges on Linux. |
| `-o`, `--match-target <TARGET>` | Select which process field regex rules match against.                                    |
| `-c`, `--config <PATH>`         | Path to an alternate config file.                                                        |
| `--log <TARGET>`                | Select log destination.                                                                  |

---

## Match Targets

The `--match-target` option controls which process field your regexes
are evaluated against.

### `name` (default)

Matches the process base name.

```bash
reniced --match-target name
```

On Linux this may be truncated to 15 characters because of kernel task
name limits.

Example:

```text
firefox
python3
ffmpeg
```

---

### `cmdline`

Matches the full command line with arguments.

```bash
reniced --match-target cmdline
```

Useful when multiple programs share the same executable.

Example:

```text
/usr/bin/python3 worker.py --queue video
```

---

### `exe`

Matches the full executable path.

```bash
reniced --match-target exe
```

Example:

```text
/usr/bin/ffmpeg
/opt/myapp/bin/worker
```

---

### `exe-basename`

Matches only the filename portion of the executable path.

```bash
reniced --match-target exe-basename
```

Unlike `name`, this is not truncated on Linux, supporting full-length
names.

Example:

```text
python3
ffmpeg
worker
```

---

## Logging Targets

### `stderr` (default)

Recommended for:

- Interactive terminal use
- `systemd`
- Containers

```bash
reniced --log stderr
```

---

### `system`

Uses:

- `syslog` on Unix
- Windows Event Log on Windows

Recommended for:

- `cron`
- Background scheduled tasks
- Detached execution

```bash
reniced --log system
```

---

# Configuration File Format

Each non-empty, non-comment line contains:

```text
<ACTIONS> <REGEX>
```

Example:

```text
5 ^firefox$
```

This means:

- Set matching processes to `nice 5`
- Match processes whose target field matches `^firefox$`

Lines beginning with `#` are comments.

Blank lines are ignored.

Invalid rules are skipped with a warning.

---

## Rule Syntax

A rule is made of:

```text
COMMAND REGEX
```

Where:

- `COMMAND` defines one or more actions
- `REGEX` is a Rust regular expression

Example:

```text
n-5r2 ^jackd$
```

This:

- Sets `nice` to `-5`
- Sets realtime I/O class with level `2`
- Applies to processes matching `^jackd$`

---

# Actions

Multiple actions can be combined.

Example:

```text
n-10r4o-10 ^pipewire$
```

This rule:

- Raises CPU priority (`nice -10`)
- Sets realtime I/O class level `4`
- Reduces OOM kill likelihood

---

## Nice Priority

### Implicit nice syntax

If the command starts with a number, it is treated as a `nice` value.

```text
5 firefox
-10 jackd
0 myprocess
```

Equivalent explicit form:

```text
n5 firefox
n-10 jackd
n0 myprocess
```

Typical Linux nice range:

```text
-20 (highest priority)
  0 (default)
 19 (lowest priority)
```

Negative priorities usually require elevated privileges.

---

## OOM Adjustment

Use `o<number>`.

Examples:

```text
o-17 important_daemon
o5 background_job
```

Lower values are less likely to be killed under memory pressure.

The tool converts legacy Linux `oom_adj` style values into modern
`oom_score_adj` values automatically.

Approximate mapping:

| Rule Value | Effective `oom_score_adj` |
|------------|---------------------------|
| `-17`      | `-1000`                   |
| `0`        | `0`                       |
| `15`       | `1000`                    |

Linux-only.

---

## I/O Priority Classes

### Realtime

```text
r<level>
```

Example:

```text
r4 audio-engine
```

---

### Best-effort

```text
b<level>
```

Example:

```text
b7 backup
```

---

### Idle

```text
i
```

Example:

```text
i updatedb
```

The idle class runs only when the system is otherwise idle.

Linux-only for full class/level support. On Windows, the class is
mapped to a Windows I/O priority hint and the level is ignored.

---

# Example Configurations

## Lower priority for browsers

```text
5 ^firefox$
5 ^chrome$
```

---

## Make audio workloads more responsive

```text
n-10r2 ^pipewire$
n-15r1 ^jackd$
```

---

## Reduce backup I/O impact

```text
10i rsync
15i borg
```

---

## Match against command lines

```text
5 worker\.py --queue low
```

Run using:

```bash
reniced --match-target cmdline
```

---

## Match against executable paths

```text
-5 ^/opt/render/bin/renderer$
```

Run using:

```bash
reniced --match-target exe
```

---

# Dry Run Mode

Use `-n` before enabling real changes.

```bash
reniced -n -v
```

This prints actions that would be performed without modifying any
process priorities.

Recommended when testing new regex rules.

---

# Shell Completions

Generate shell completions:

```bash
reniced completions bash
reniced completions zsh
reniced completions fish
reniced completions elvish
reniced completions powershell
```

Example installation for Bash:

```bash
reniced completions bash > ~/.local/share/bash-completion/completions/reniced
```

---

# Linux Notes

## Thread Support

Linux can optionally include threads/tasks using:

```bash
reniced --threads
```

This requires elevated privileges.

The option is useful when applications create worker threads with names
that differ from the parent process.

---

## Permissions

Raising priorities typically requires:

- Root access
- `CAP_SYS_NICE`
- Appropriate PAM limits

Without sufficient permissions, `reniced` logs warnings and continues.

---

# Exit Behavior

- Returns `0` on success
- Returns non-zero on fatal errors
- Invalid individual rules are skipped instead of aborting the run
- Individual process failures are logged and processing continues

---

# Example Workflow

1. Create a config file
2. Test with dry-run mode
3. Enable verbose logging
4. Run from:
- `systemd`
- `cron`
- login scripts
- service wrappers

Example:

```bash
reniced -n -v --config ~/.reniced
```

Then:

```bash
reniced -v --config ~/.reniced
```

---

# License

[GPL-2.0 or later][license]

[license]: https://spdx.org/licenses/GPL-2.0-or-later.html
[oom_score_adj]: https://man7.org/linux/man-pages/man5/proc_pid_oom_score_adj.5.html "proc_pid_oom_score_adj(5)"
