use std::io::{Error, ErrorKind};
use std::mem::MaybeUninit;
use std::time::{Duration, Instant};
use tuinix::{Terminal, TerminalEvent};

use crate::lsp_client::LspClient;

/// Extended terminal event that includes LSP client events
#[derive(Debug)]
pub enum ExtendedTerminalEvent {
    /// Terminal event (input or resize)
    Terminal(TerminalEvent),
    /// LSP client stdout has data available
    LspStdout,
    /// LSP client stderr has data available
    LspStderr,
}

/// Extension trait for Terminal to support polling LSP client file descriptors
pub trait TerminalExt {
    /// Poll for events from both terminal and LSP client
    fn poll_event_with_lsp(
        &mut self,
        lsp_client: &LspClient,
        timeout: Option<Duration>,
    ) -> std::io::Result<Option<ExtendedTerminalEvent>>;
}

impl TerminalExt for Terminal {
    fn poll_event_with_lsp(
        &mut self,
        lsp_client: &LspClient,
        timeout: Option<Duration>,
    ) -> std::io::Result<Option<ExtendedTerminalEvent>> {
        use std::os::fd::AsRawFd;

        // First check if there's already buffered terminal input
        if let Some(input) = self.read_input()? {
            return Ok(Some(ExtendedTerminalEvent::Terminal(TerminalEvent::Input(
                input,
            ))));
        }

        let start_time = Instant::now();
        loop {
            unsafe {
                let mut readfds = MaybeUninit::<libc::fd_set>::zeroed();
                libc::FD_ZERO(readfds.as_mut_ptr());

                // Add terminal file descriptors
                let stdin_fd = self.input_fd();
                let signal_fd = self.signal_fd();
                libc::FD_SET(stdin_fd, readfds.as_mut_ptr());
                libc::FD_SET(signal_fd, readfds.as_mut_ptr());

                // Add LSP client file descriptors
                let lsp_stdout_fd = lsp_client.stdout.as_raw_fd();
                let lsp_stderr_fd = lsp_client.stderr.get_ref().as_raw_fd();
                libc::FD_SET(lsp_stdout_fd, readfds.as_mut_ptr());
                libc::FD_SET(lsp_stderr_fd, readfds.as_mut_ptr());

                let mut readfds = readfds.assume_init();

                // Find the maximum file descriptor
                let maxfd = [stdin_fd, signal_fd, lsp_stdout_fd, lsp_stderr_fd]
                    .iter()
                    .max()
                    .copied()
                    .unwrap_or(0);

                // Set up timeout
                let mut timeval = MaybeUninit::<libc::timeval>::zeroed();
                let timeval_ptr = if let Some(duration) = timeout {
                    let duration = duration.saturating_sub(start_time.elapsed());
                    if duration.is_zero() {
                        return Ok(None);
                    }
                    let tv = timeval.as_mut_ptr();
                    (*tv).tv_sec = duration.as_secs() as libc::time_t;
                    (*tv).tv_usec = duration.subsec_micros() as libc::suseconds_t;
                    tv
                } else {
                    std::ptr::null_mut()
                };

                // Call select
                let ret = libc::select(
                    maxfd + 1,
                    &mut readfds,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    timeval_ptr,
                );

                if ret == -1 {
                    let e = Error::last_os_error();
                    if e.kind() == ErrorKind::Interrupted {
                        continue;
                    }
                    return Err(e);
                } else if ret == 0 {
                    // Timeout
                    return Ok(None);
                }

                // Check which file descriptor has data available
                if libc::FD_ISSET(stdin_fd, &readfds) {
                    if let Some(input) = self.read_input()? {
                        return Ok(Some(ExtendedTerminalEvent::Terminal(TerminalEvent::Input(
                            input,
                        ))));
                    }
                }

                if libc::FD_ISSET(signal_fd, &readfds) {
                    let size = self.wait_for_resize()?;
                    return Ok(Some(ExtendedTerminalEvent::Terminal(
                        TerminalEvent::Resize(size),
                    )));
                }

                if libc::FD_ISSET(lsp_stdout_fd, &readfds) {
                    return Ok(Some(ExtendedTerminalEvent::LspStdout));
                }

                if libc::FD_ISSET(lsp_stderr_fd, &readfds) {
                    return Ok(Some(ExtendedTerminalEvent::LspStderr));
                }
            }
        }
    }
}
