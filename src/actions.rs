use std::fs;

use anyhow::Result;

use crate::cli::{Cli, MatchTarget};
use crate::model::{IoClass, ProcessEntry, Rule};
use crate::platform::set_process_priority;

const IOPRIO_WHO_PROCESS: libc::c_int = 1;

fn ioprio_value(class: u16, data: u16) -> libc::c_int {
    ((class << 13) | data) as libc::c_int
}

pub fn apply_rules(process: &ProcessEntry, rules: &[Rule], cli: &Cli) -> Result<()> {
    for rule in rules {
        let target = match cli.match_target {
            MatchTarget::Name => &process.name,
            MatchTarget::Cmdline => &process.cmd,
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

fn set_priority(process: &ProcessEntry, nice: i32, cli: &Cli) -> Result<()> {
    if cli.noop {
        println!("would set priority of {} to {}", process.pid, nice);
        return Ok(());
    }

    set_process_priority(process.pid, nice)?;

    if cli.verbose {
        println!("nice set to {}: {}/{}", nice, process.pid, process.cmd);
    }

    Ok(())
}

fn adjust_oom(process: &ProcessEntry, score: i32, cli: &Cli) -> Result<()> {
    let converted = convert_oom_adj(score);

    if cli.noop {
        println!(
            "would adjust OOM setting of pid {} to {}",
            process.pid, converted
        );

        return Ok(());
    }

    let path = format!("/proc/{}/oom_score_adj", process.pid);

    fs::write(&path, format!("{converted}\n"))
        .map_err(|error| anyhow::anyhow!("failed writing {}: {}", path, error,))?;

    if cli.verbose {
        println!(
            "OOM adjust set to {}: {}/{}",
            converted, process.pid, process.cmd
        );
    }

    Ok(())
}

fn set_io_priority(
    process: &ProcessEntry,
    class: IoClass,
    level: Option<u8>,
    cli: &Cli,
) -> Result<()> {
    let class_num = match class {
        IoClass::Realtime => 1,
        IoClass::BestEffort => 2,
        IoClass::Idle => 3,
    };

    if cli.noop {
        match level {
            Some(level) => {
                println!(
                    "would set IO priority for pid {} to class {} level {}",
                    process.pid, class_num, level,
                );
            }
            None => {
                println!(
                    "would set IO priority for pid {} to class {}",
                    process.pid, class_num,
                );
            }
        }
        return Ok(());
    }

    let data = u16::from(level.unwrap_or(0));

    let prio = ioprio_value(class_num as u16, data);

    let result =
        unsafe { libc::syscall(libc::SYS_ioprio_set, IOPRIO_WHO_PROCESS, process.pid, prio) };

    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    if cli.verbose {
        println!("IO priority updated: {}/{}", process.pid, process.cmd);
    }

    Ok(())
}

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
