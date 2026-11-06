//! A Windows Job Object with kill-on-close: when the `Job` is dropped (or the host exits),
//! every process in it dies, including grandchildren such as shells and MCP servers.
//! `Child::kill` alone leaves those running.
//! Adapted from Agent Hub's parked Rust app (`apps/hub/src-tauri/src/job.rs`).

#[cfg(windows)]
mod imp {
    use anyhow::{Result, bail};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
    };

    pub struct Job(HANDLE);
    // A job handle is a kernel handle; it can move between threads.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    impl Job {
        pub fn new() -> Result<Job> {
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    bail!("could not create a Job Object: {}", std::io::Error::last_os_error());
                }
                let job = Job(handle);
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 {
                    bail!("could not set kill-on-close on the Job Object: {}", std::io::Error::last_os_error());
                }
                Ok(job)
            }
        }

        /// Puts the process with this raw handle into the job.
        pub fn assign(&self, process: std::os::windows::io::RawHandle) -> Result<()> {
            if unsafe { AssignProcessToJobObject(self.0, process as HANDLE) } == 0 {
                bail!("could not put the process in its Job Object: {}", std::io::Error::last_os_error());
            }
            Ok(())
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }
}

#[cfg(not(windows))]
mod imp {
    /// Linux and macOS will use a process group here (not built yet).
    pub struct Job;
    impl Job {
        pub fn new() -> anyhow::Result<Job> {
            Ok(Job)
        }
    }
}

pub use imp::Job;

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    use std::time::{Duration, Instant};

    #[test]
    fn dropping_the_job_kills_the_whole_tree() {
        // cmd starts a grandchild `ping` that waits about 3 s, then writes a marker.
        // Dropping the job must stop that write from ever happening.
        let marker = std::env::temp_dir().join(format!("curia-job-test-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let script = format!("ping -n 4 127.0.0.1 >NUL & echo done > \"{}\"", marker.display());
        let mut child = Command::new("cmd").args(["/C", &script]).creation_flags(0x0800_0000).spawn().unwrap();
        let job = Job::new().unwrap();
        job.assign(child.as_raw_handle()).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let started = Instant::now();
        drop(job);
        child.wait().unwrap();
        assert!(started.elapsed() < Duration::from_secs(2), "the tree should die at once");
        std::thread::sleep(Duration::from_secs(4));
        assert!(!marker.exists(), "the grandchild survived the job");
    }
}
