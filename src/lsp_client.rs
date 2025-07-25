use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};

use orfail::OrFail;

#[derive(Debug)]
pub struct LspServerSpec {
    pub command: PathBuf,
    pub args: Vec<String>,
}

impl LspServerSpec {
    pub fn parse_args(args: &mut noargs::RawArgs) -> noargs::Result<Self> {
        noargs::opt("lsp-server")
            .short('s')
            .env("LSPTERM_LSP_SERVER")
            .example("/path/to/lsp-server")
            .take(args)
            .then(|a| a.value().parse())
    }
}

impl std::str::FromStr for LspServerSpec {
    type Err = nojson::JsonParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with('{') {
            return Ok(Self {
                command: PathBuf::from(s),
                args: Vec::new(),
            });
        }

        let json = nojson::RawJson::parse(s)?;
        let value = json.value();
        Ok(Self {
            command: value.to_member("command")?.required()?.try_into()?,
            args: value
                .to_member("args")?
                .map(|v| v.try_into())?
                .unwrap_or_default(),
        })
    }
}

#[derive(Debug)]
pub struct LspClient {
    process: Child,
    pub stdin: ChildStdin,
    pub stdout: Option<ChildStdout>,
    stderr: Option<BufReader<ChildStderr>>,
}

impl LspClient {
    pub fn new(lsp_server_spec: LspServerSpec) -> orfail::Result<Self> {
        let mut command = Command::new(&lsp_server_spec.command);
        command
            .args(&lsp_server_spec.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut process = command.spawn().or_fail_with(|e| {
            format!(
                "failed to spawn LSP server process '{}': {e}",
                lsp_server_spec.command.display()
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
        })
    }

    pub fn send<T>(&mut self, request: T) -> orfail::Result<()>
    where
        T: nojson::DisplayJson,
    {
        let content = nojson::Json(request).to_string();
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
