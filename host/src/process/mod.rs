//! Starting CLI processes: piped stdio, no console window, inside a Job Object.
mod job;

pub use job::Job;

use anyhow::{Context, Result};
use tokio::process::{Child, Command};

/// A running process and the job that holds its whole tree. Dropping it kills the tree.
pub struct Contained {
    pub child: Child,
    pub job: Job,
}

/// Starts `cmd` with piped stdin, stdout and stderr and puts it in a fresh Job Object.
/// If the job can't hold it, the process is killed and an error returned: a CLI never
/// runs uncontained.
pub fn spawn_contained(mut cmd: Command, what: &str) -> Result<Contained> {
    use std::process::Stdio;
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut child = cmd.spawn().with_context(|| format!("could not start {what}"))?;
    let job = Job::new()?;
    #[cfg(windows)]
    {
        let handle = child.raw_handle().context("the process ended before it could be contained")?;
        if let Err(e) = job.assign(handle) {
            let _ = child.start_kill();
            return Err(e).with_context(|| format!("{what} was stopped"));
        }
    }
    Ok(Contained { child, job })
}
