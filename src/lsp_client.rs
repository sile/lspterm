use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use orfail::OrFail;

use crate::json::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, json_object};

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
    pub stdout: BufReader<ChildStdout>,
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
            stdout: BufReader::new(stdout),
            process,
            next_request_id: 0,
        })
    }

    pub fn call<T>(&mut self, request: T) -> orfail::Result<T::Response>
    where
        T: JsonRpcRequest,
    {
        let id = self.send_request(request).or_fail()?;
        self.recv_response(id).or_fail()
    }

    pub fn cast<T>(&mut self, notification: T) -> orfail::Result<()>
    where
        T: JsonRpcNotification,
    {
        self.send_notification(notification).or_fail()?;
        Ok(())
    }

    fn send_request<T>(&mut self, request: T) -> orfail::Result<u64>
    where
        T: JsonRpcRequest,
    {
        let id = self.next_request_id;
        let content = nojson::Json(json_object(|f| {
            f.member("jsonrpc", "2.0")?;
            f.member("id", id)?;
            f.member("method", request.method())?;
            f.member("params", json_object(|f| request.params(f)))
        }))
        .to_string();
        self.next_request_id += 1;

        if self.options.verbose {
            eprintln!("{content}");
        }

        write!(self.stdin, "Content-Length: {}\r\n", content.len()).or_fail()?;
        write!(self.stdin, "\r\n").or_fail()?;
        write!(self.stdin, "{content}").or_fail()?;
        self.stdin.flush().or_fail()?;

        Ok(id)
    }

    fn send_notification<T>(&mut self, request: T) -> orfail::Result<()>
    where
        T: JsonRpcNotification,
    {
        let content = nojson::Json(json_object(|f| {
            f.member("jsonrpc", "2.0")?;
            f.member("method", request.method())?;
            f.member("params", json_object(|f| request.params(f)))
        }))
        .to_string();

        if self.options.verbose {
            eprintln!("{content}");
        }

        write!(self.stdin, "Content-Length: {}\r\n", content.len()).or_fail()?;
        write!(self.stdin, "\r\n").or_fail()?;
        write!(self.stdin, "{content}").or_fail()?;
        self.stdin.flush().or_fail()?;

        Ok(())
    }

    fn recv_response<T>(&mut self, request_id: u64) -> orfail::Result<T>
    where
        T: JsonRpcResponse,
    {
        let mut content_length = None;
        loop {
            let mut line = String::new();
            let size = self.stdout.read_line(&mut line).or_fail()?;
            (size > 0).or_fail()?;
            if line == "\r\n" {
                break;
            }

            let (k, v) = line.split_once(':').or_fail()?;
            if k.eq_ignore_ascii_case("Content-Length") {
                content_length = Some(v.trim().parse::<usize>().or_fail()?);
            }
        }

        let content_length = content_length.or_fail()?;
        let mut content = vec![0; content_length];
        self.stdout.read_exact(&mut content).or_fail()?;

        let content = String::from_utf8(content).or_fail()?;
        if self.options.verbose {
            eprintln!("{content}");
        }

        let json = nojson::RawJson::parse(&content).or_fail()?;
        self.parse_response(request_id, json.value()).or_fail()
    }

    fn parse_response<T>(
        &self,
        request_id: u64,
        value: nojson::RawJsonValue<'_, '_>,
    ) -> Result<T, nojson::JsonParseError>
    where
        T: JsonRpcResponse,
    {
        value.to_member("jsonrpc")?.required()?.map(|v| {
            let version = v.to_unquoted_string_str()?;
            if version == "2.0" {
                Ok(())
            } else {
                Err(v.invalid("unsupported JSON-RPC version"))
            }
        })?;
        value.to_member("id")?.required()?.map(|v| {
            let id = v.as_integer_str()?;
            if id == request_id.to_string() {
                Ok(())
            } else {
                Err(v.invalid("expected ID {request_id} but got {id}"))
            }
        })?;

        if let Some(e) = value.to_member("error")?.get() {
            return Err(e.invalid("unexpected error response"));
        }

        let result = value.to_member("result")?.required()?;
        T::from_result_value(result)
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
