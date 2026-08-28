use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "state", content = "details")]
pub enum ProcessStatus {
    #[serde(rename = "stopped")]
    Stopped,
    #[serde(rename = "running")]
    Running {
        pid: u32,
        endpoint: String,
        lock_file: String,
    },
    #[serde(rename = "stale")]
    Stale {
        stale_pid: u32,
        lock_file: String,
        message: String,
    },
    #[serde(rename = "unhealthy")]
    Unhealthy {
        pid: u32,
        endpoint: String,
        reason: String,
    },
}

pub struct DaemonLock {
    #[allow(dead_code)]
    file: File,
    path: PathBuf,
}

impl DaemonLock {
    /// Acquire an exclusive advisory OS lock for the daemon process.
    pub fn acquire(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                let _ = std::fs::create_dir_all(parent);
            }
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;

        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();
            let flock_res = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
            if flock_res != 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::EWOULDBLOCK)
                    || err.raw_os_error() == Some(libc::EAGAIN)
                {
                    anyhow::bail!(
                        "AeroFS daemon is already running (exclusive lock held at: {})",
                        path.display()
                    );
                } else {
                    return Err(err.into());
                }
            }
        }

        // Set file permissions to 0600 on unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = file.set_permissions(perms);
        }

        let mut f_mut = &file;
        let _ = f_mut.set_len(0);
        let _ = f_mut.seek(SeekFrom::Start(0));
        let _ = write!(f_mut, "{}", std::process::id());
        let _ = f_mut.flush();

        Ok(Self {
            file,
            path: path.to_path_buf(),
        })
    }

    /// Cleanly remove lock file on shutdown
    pub fn release(self) {
        if self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    /// Inspect daemon status without blocking or mutating state
    pub fn inspect_status(lock_path: &Path, host: &str, port: u16) -> ProcessStatus {
        let endpoint = format!("{}:{}", host, port);

        if !lock_path.exists() {
            return ProcessStatus::Stopped;
        }

        let file_res = OpenOptions::new().read(true).write(false).open(lock_path);

        let file = match file_res {
            Ok(f) => f,
            Err(_) => return ProcessStatus::Stopped,
        };

        // On Unix, test flock non-blockingly
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();
            // Try non-blocking shared or exclusive lock
            let flock_res = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
            if flock_res == 0 {
                // We obtained lock! That means NO active daemon is holding the lock.
                // Unlock immediately
                unsafe { libc::flock(fd, libc::LOCK_UN) };

                // Read stale PID if present
                let mut content = String::new();
                let mut f_read = file;
                let _ = f_read.read_to_string(&mut content);
                let stale_pid = content.trim().parse::<u32>().unwrap_or(0);

                return ProcessStatus::Stale {
                    stale_pid,
                    lock_file: lock_path.display().to_string(),
                    message: "Lock file exists but OS lock is not held (daemon crashed or terminated ungracefully)".to_string(),
                };
            }
        }

        // Lock is actively held! Read PID from file
        let mut content = String::new();
        let mut f_read = file;
        let _ = f_read.read_to_string(&mut content);
        let pid = content.trim().parse::<u32>().unwrap_or(0);

        #[cfg(unix)]
        {
            if pid > 0 {
                // Verify process existence in OS via signal 0
                let kill_res = unsafe { libc::kill(pid as i32, 0) };
                if kill_res != 0 {
                    let err = std::io::Error::last_os_error();
                    if err.raw_os_error() == Some(libc::ESRCH) {
                        return ProcessStatus::Stale {
                            stale_pid: pid,
                            lock_file: lock_path.display().to_string(),
                            message: format!("Process with PID {} no longer exists in OS", pid),
                        };
                    }
                }
            }
        }

        ProcessStatus::Running {
            pid,
            endpoint,
            lock_file: lock_path.display().to_string(),
        }
    }
}
