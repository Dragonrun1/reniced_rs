use anyhow::Result;
use nix::unistd::Uid;
use procfs::process::{
    Process as ProcfsProcess,
    Task,
};

use crate::model::ProcessEntry;

pub fn read_processes(
    include_threads: bool,
) -> Result<Vec<ProcessEntry>> {
    let mut entries = Vec::new();

    for process in procfs::process::all_processes()?
        .flatten()
    {
        collect_process(
            &process,
            include_threads,
            &mut entries,
        )?;
    }

    Ok(entries)
}

fn collect_process(
    process: &ProcfsProcess,
    include_threads: bool,
    entries: &mut Vec<ProcessEntry>,
) -> Result<()> {
    let owner_uid = process.status()?.ruid;

    let current_uid = Uid::effective();

    if !current_uid.is_root()
        && owner_uid != current_uid.into()
    {
        return Ok(());
    }

    let cmd = process_name(process)?;

    entries.push(ProcessEntry {
        pid: process.pid,
        cmd: cmd.clone(),
    });

    if include_threads {
        collect_threads(process, &cmd, entries)?;
    }

    Ok(())
}
fn collect_threads(
    process: &ProcfsProcess,
    fallback_cmd: &str,
    entries: &mut Vec<ProcessEntry>,
) -> Result<()> {
    let tasks = process.tasks()?;

    for task in tasks.flatten() {
        let tid = task.tid;

        if tid == process.pid {
            continue;
        }

        let cmd = thread_name(&task)
            .unwrap_or_else(|_| fallback_cmd.to_string());

        entries.push(ProcessEntry {
            pid: tid,
            cmd,
        });
    }

    Ok(())
}

fn process_name(
    process: &ProcfsProcess,
) -> Result<String> {
    Ok(process.stat()?.comm)
}

fn thread_name(task: &Task) -> Result<String> {
    Ok(task.stat()?.comm)
}
