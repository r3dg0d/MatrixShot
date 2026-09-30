//! Bind saved recorder state to a Linux process, rather than a reusable PID.

use anyhow::{bail, Context, Result};
use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

fn start_ticks(stat: &str) -> Option<u64> {
    // The command field may itself contain spaces and closing parentheses.
    stat.rsplit_once(')')?
        .1
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()
}

pub fn token(pid: u32) -> Result<String> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let ticks = start_ticks(&stat).context("invalid recorder process metadata")?;
    let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    Ok(format!("{}:{ticks}", boot.trim()))
}

pub struct Handle(OwnedFd);

impl Handle {
    pub fn open(pid: u32, expected: &str) -> Result<Option<Self>> {
        if pid == 0 || pid > i32::MAX as u32 {
            return Ok(None);
        }
        // SAFETY: pidfd_open takes an integer PID and flags, and returns an fd.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid as i32, 0u32) };
        if fd < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                return Ok(None);
            }
            return Err(error).context("open recorder pidfd (Linux 5.3+ required)");
        }
        // SAFETY: the new descriptor is valid and owned exclusively here.
        let handle = Self(unsafe { OwnedFd::from_raw_fd(fd as i32) });
        let current = match token(pid) {
            Ok(value) => value,
            Err(error)
                if error
                    .downcast_ref::<io::Error>()
                    .is_some_and(|e| e.kind() == io::ErrorKind::NotFound) =>
            {
                return Ok(None)
            }
            Err(error) => return Err(error),
        };
        if current != expected.trim() || handle.exited()? {
            return Ok(None);
        }
        Ok(Some(handle))
    }

    pub fn interrupt(&self) -> Result<()> {
        // SAFETY: fd stays open for this call; null requests the default siginfo.
        let result = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.0.as_raw_fd(),
                libc::SIGINT,
                std::ptr::null::<libc::siginfo_t>(),
                0u32,
            )
        };
        if result < 0 {
            let error = io::Error::last_os_error();
            // A recorder that exited between validation and signaling is done.
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error).context("interrupt recorder");
            }
        }
        Ok(())
    }

    pub fn exited(&self) -> Result<bool> {
        let mut pollfd = libc::pollfd {
            fd: self.0.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: pollfd points to one initialized pollfd for the call's lifetime.
        let result = unsafe { libc::poll(&mut pollfd, 1, 0) };
        if result < 0 {
            return Err(io::Error::last_os_error()).context("poll recorder exit");
        }
        if pollfd.revents & libc::POLLNVAL != 0 {
            bail!("invalid recorder pidfd");
        }
        Ok(pollfd.revents & (libc::POLLIN | libc::POLLHUP) != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_command_names_with_parentheses_and_spaces() {
        let fields = (4..=22)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            start_ticks(&format!("123 (a ) strange name) S {fields}")),
            Some(22)
        );
        assert_eq!(start_ticks("123 (broken) S"), None);
    }

    #[test]
    fn rejects_wrong_identity_and_invalid_pid() {
        assert!(Handle::open(std::process::id(), "different-boot:0")
            .unwrap()
            .is_none());
        assert!(Handle::open(0, "anything").unwrap().is_none());
        assert!(Handle::open(u32::MAX, "anything").unwrap().is_none());
        let token = token(std::process::id()).unwrap();
        let handle = Handle::open(std::process::id(), &token).unwrap().unwrap();
        assert!(!handle.exited().unwrap());
    }
}
