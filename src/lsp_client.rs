use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};

use orfail::OrFail;

#[derive(Debug)]
enum Logger {
    Null,
    File { messages: File, server_stderr: File },
}

impl Logger {
    fn new(log_dir: Option<PathBuf>) -> orfail::Result<Self> {
        let Some(log_dir) = log_dir else {
            return Ok(Logger::Null);
        };

        std::fs::create_dir_all(&log_dir).or_fail_with(|e| {
            format!(
                "failed to create log directory '{}': {e}",
                log_dir.display()
            )
        })?;

        let messages = File::create(log_dir.join("messages.jsonl"))
            .or_fail_with(|e| format!("failed to create messages log file: {e}"))?;

        let server_stderr = File::create(log_dir.join("server_stderr.log"))
            .or_fail_with(|e| format!("failed to create server stderr log file: {e}"))?;

        Ok(Logger::File {
            messages,
            server_stderr,
        })
    }

    fn log_message(&mut self, message: &str) -> orfail::Result<()> {
        if let Logger::File { messages, .. } = self {
            writeln!(messages, "{message}").or_fail()?;
        }
        Ok(())
    }

    fn log_server_stderr(&mut self, line: &str) -> orfail::Result<()> {
        if let Logger::File { server_stderr, .. } = self {
            writeln!(server_stderr, "{line}").or_fail()?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct LspClient {
    process: Child,
    pub stdin: ChildStdin,
    pub stdout: Option<ChildStdout>,
    stderr: Option<BufReader<ChildStderr>>,
    logger: Logger,
}

impl LspClient {
    pub fn new(
        lsp_server_command: PathBuf,
        lsp_server_args: Vec<String>,
        log_dir: Option<PathBuf>,
    ) -> orfail::Result<Self> {
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

        // TODO: Make stdin non-blocking
        let stdin = process.stdin.take().or_fail()?;

        // TODO: Make stdout non-blocking
        let stdout = process.stdout.take().or_fail()?;

        let stderr = process.stderr.take().or_fail()?;
        tuinix::set_nonblocking(stderr.as_raw_fd()).or_fail()?;

        Ok(Self {
            stdin,
            stdout: Some(stdout),
            stderr: Some(BufReader::new(stderr)),
            process,
            logger: Logger::new(log_dir).or_fail()?,
        })
    }

    pub fn send<T>(&mut self, request: T) -> orfail::Result<()>
    where
        T: nojson::DisplayJson,
    {
        let content = nojson::Json(request).to_string();
        self.logger.log_message(&content).or_fail()?;
        write!(self.stdin, "Content-length: {}\r\n", content.len()).or_fail()?;
        write!(self.stdin, "\r\n").or_fail()?;
        write!(self.stdin, "{content}").or_fail()?;
        self.stdin.flush().or_fail()?;
        Ok(())
    }

    pub fn recv_response_json(&mut self) -> orfail::Result<String> {
        let Some(reader) = &mut self.stdout else {
            todo!();
        };

        let mut buf = vec![0; 4096];
        let size = reader.read(&mut buf).or_fail()?;
        Ok(String::from_utf8_lossy(&buf[..size]).into_owned())
    }

    pub fn stdout_fd(&self) -> Option<RawFd> {
        self.stdout.as_ref().map(|x| x.as_raw_fd())
    }

    pub fn stderr_fd(&self) -> Option<RawFd> {
        self.stderr.as_ref().map(|x| x.get_ref().as_raw_fd())
    }

    pub fn read_stderr_line(&mut self) -> orfail::Result<Option<String>> {
        let Some(reader) = &mut self.stderr else {
            return Ok(None);
        };

        let mut line = String::new();
        match tuinix::try_nonblocking(reader.read_line(&mut line)) {
            Ok(Some(0)) => {
                self.stderr = None;
                Ok(None)
            }
            Ok(Some(_)) => {
                let line = line.trim_end().to_owned();
                self.logger.log_server_stderr(&line).or_fail()?;
                Ok(Some(line))
            }
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
