//! Privileged listener acquisition from the operating-system service manager.
//!
//! launchd/systemd own ports 80 and 443 and pass the already-bound descriptors
//! to Roxy. The rest of the daemon can therefore run as the developer user.

use std::net::TcpListener;

use anyhow::Result;

#[derive(Default)]
pub struct ActivatedListeners {
    pub http: Option<TcpListener>,
    pub https: Option<TcpListener>,
}

impl ActivatedListeners {
    pub fn acquire() -> Result<Self> {
        platform::acquire()
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::env;
    use std::os::fd::{FromRawFd, RawFd};
    use std::process;

    use anyhow::{Context, Result, bail};

    use super::{ActivatedListeners, TcpListener};

    const FIRST_ACTIVATION_FD: RawFd = 3;

    pub fn acquire() -> Result<ActivatedListeners> {
        let Some(listen_pid) = env::var("LISTEN_PID").ok() else {
            return Ok(ActivatedListeners::default());
        };
        if listen_pid.parse::<u32>().ok() != Some(process::id()) {
            return Ok(ActivatedListeners::default());
        }

        let count = env::var("LISTEN_FDS")
            .context("LISTEN_PID is set but LISTEN_FDS is missing")?
            .parse::<usize>()
            .context("LISTEN_FDS is not a valid descriptor count")?;
        if count > 2 {
            bail!("Roxy expected at most two activated listeners, received {count}");
        }
        let names = env::var("LISTEN_FDNAMES").unwrap_or_default();
        let names: Vec<_> = names.split(':').collect();
        let mut listeners = ActivatedListeners::default();

        for index in 0..count {
            let fd = FIRST_ACTIVATION_FD + index as RawFd;
            let name = names.get(index).copied().unwrap_or(match index {
                0 => "http",
                1 => "https",
                _ => "unknown",
            });
            let listener = listener_from_owned_fd(fd)?;

            match name {
                "http" => listeners.http = Some(listener),
                "https" => listeners.https = Some(listener),
                _ => drop(listener),
            }
        }

        Ok(listeners)
    }

    fn listener_from_owned_fd(fd: RawFd) -> Result<TcpListener> {
        // SAFETY: F_GETFD only inspects the numeric descriptor and does not
        // dereference pointers. A failure lets us reject spoofed or stale
        // socket-activation environment variables before taking ownership.
        if unsafe { libc::fcntl(fd, libc::F_GETFD) } == -1 {
            bail!("Activated descriptor {fd} is not open");
        }

        // SAFETY: systemd promises that descriptors starting at 3 are valid,
        // uniquely owned when LISTEN_PID matches this process; the F_GETFD
        // check above verifies the descriptor exists. This function is called
        // at most once and immediately takes ownership.
        let listener = unsafe { TcpListener::from_raw_fd(fd) };
        listener
            .set_nonblocking(true)
            .context("Failed to make activated listener non-blocking")?;
        if listener.local_addr().is_err() {
            bail!("Activated descriptor {fd} is not a TCP listener");
        }
        Ok(listener)
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::CString;
    use std::os::fd::{FromRawFd, OwnedFd};
    use std::os::raw::{c_char, c_int};
    use std::ptr;

    use anyhow::{Context, Result, bail};

    use super::{ActivatedListeners, TcpListener};

    #[link(name = "System")]
    unsafe extern "C" {
        fn launch_activate_socket(
            name: *const c_char,
            fds: *mut *mut c_int,
            count: *mut usize,
        ) -> c_int;
    }

    pub fn acquire() -> Result<ActivatedListeners> {
        Ok(ActivatedListeners {
            http: activate("Http")?,
            https: activate("Https")?,
        })
    }

    fn activate(name: &str) -> Result<Option<TcpListener>> {
        let name = CString::new(name).context("launchd socket name contains NUL")?;
        let mut raw_fds: *mut c_int = ptr::null_mut();
        let mut count = 0_usize;

        // SAFETY: `name` is NUL-terminated, both output pointers are valid for
        // writes, and launchd allocates the returned descriptor array. On
        // success each descriptor is uniquely transferred to this process.
        let result = unsafe { launch_activate_socket(name.as_ptr(), &mut raw_fds, &mut count) };
        if result == libc::ENOENT || result == libc::ESRCH {
            return Ok(None);
        }
        if result != 0 {
            bail!("launch_activate_socket failed with errno {result}");
        }
        if raw_fds.is_null() || count == 0 {
            return Ok(None);
        }

        // SAFETY: launchd returned `count` initialized descriptors in an array
        // allocated with malloc. Copying the integers does not outlive the array.
        let descriptors = unsafe { std::slice::from_raw_parts(raw_fds, count) }.to_vec();
        // SAFETY: launchd documents that callers own and must free this array.
        unsafe { libc::free(raw_fds.cast()) };

        let mut owned: Vec<OwnedFd> = descriptors
            .into_iter()
            .map(|fd| {
                // SAFETY: each descriptor was transferred by launchd exactly
                // once and is now represented by one OwnedFd.
                unsafe { OwnedFd::from_raw_fd(fd) }
            })
            .collect();
        let Some(fd) = owned.pop() else {
            return Ok(None);
        };
        let listener = TcpListener::from(fd);
        listener
            .set_nonblocking(true)
            .context("Failed to make activated listener non-blocking")?;
        listener
            .local_addr()
            .context("Activated descriptor is not a TCP listener")?;
        Ok(Some(listener))
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod platform {
    use anyhow::Result;

    use super::ActivatedListeners;

    pub fn acquire() -> Result<ActivatedListeners> {
        Ok(ActivatedListeners::default())
    }
}
