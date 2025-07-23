use std::io::{BufRead, BufReader};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};

use orfail::OrFail;

#[derive(Debug)]
pub struct LspClient {
    process: Child,
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
    pub stderr: BufReader<ChildStderr>,
}

impl LspClient {
    pub fn new(lsp_server_command: PathBuf, lsp_server_args: Vec<String>) -> orfail::Result<Self> {
        let mut command = Command::new(&lsp_server_command);
        command
            .args(&lsp_server_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut process = command.spawn().or_fail_with(|e| {
            format!(
                "failed to spawn LSP server process '{}': {e}",
                lsp_server_command.display()
            )
        })?;

        let stdin = process.stdin.take().or_fail()?;
        tuinix::set_nonblocking(stdin.as_raw_fd()).or_fail()?;

        let stdout = process.stdout.take().or_fail()?;
        tuinix::set_nonblocking(stdout.as_raw_fd()).or_fail()?;

        let stderr = process.stderr.take().or_fail()?;
        tuinix::set_nonblocking(stderr.as_raw_fd()).or_fail()?;

        Ok(Self {
            stdin,
            stdout,
            stderr: BufReader::new(stderr),
            process,
        })
    }

    pub fn read_stderr_line(&mut self) -> orfail::Result<Option<String>> {
        let mut line = String::new();
        match tuinix::try_nonblocking(self.stderr.read_line(&mut line)) {
            Ok(Some(_)) => Ok(Some(line)),
            Ok(None) => Ok(None),
            Err(e) => Err(e).or_fail(),
        }
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.process.stdin.take();
        for _ in 0..10 {
            let Ok(status) = self.process.try_wait() else {
                break;
            };
            if status.is_some() {
                return;
            }

            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let _ = self.process.kill();
    }
}
