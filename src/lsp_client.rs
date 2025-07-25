use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use orfail::OrFail;

use crate::json::{JsonRpcRequest, json_object};

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
pub struct LspClientOptions {
    pub server_spec: LspServerSpec,
    pub verbose: bool,
}

impl LspClientOptions {
    pub fn parse_args(args: &mut noargs::RawArgs) -> noargs::Result<Self> {
        Ok(Self {
            server_spec: LspServerSpec::parse_args(args)?,
            verbose: noargs::flag("verbose").short('v').take(args).is_present(),
        })
    }
}

#[derive(Debug)]
pub struct LspClient {
    options: LspClientOptions,
    process: Child,
    pub stdin: ChildStdin,
    pub stdout: Option<ChildStdout>,
    next_request_id: u64,
}

impl LspClient {
    pub fn new(options: LspClientOptions) -> orfail::Result<Self> {
        let mut command = Command::new(&options.server_spec.command);
        command
            .args(&options.server_spec.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut process = command.spawn().or_fail_with(|e| {
            format!(
                "failed to spawn LSP server process '{}': {e}",
                options.server_spec.command.display()
            )
        })?;

        let stdin = process.stdin.take().or_fail()?;
        let stdout = process.stdout.take().or_fail()?;
        Ok(Self {
            options,
            stdin,
            stdout: Some(stdout),
            process,
            next_request_id: 0,
        })
    }

    pub fn send_request<T>(&mut self, request: T) -> orfail::Result<()>
    where
        T: JsonRpcRequest,
    {
        let content = nojson::Json(json_object(|f| {
            f.member("jsonrpc", "2.0")?;
            f.member("id", self.next_request_id)?;
            f.member("method", request.method())?;
            f.member("params", json_object(|f| request.params(f)))
        }))
        .to_string();
        self.next_request_id += 1;

        if self.options.verbose {
            eprintln!("{content}");
        }

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
