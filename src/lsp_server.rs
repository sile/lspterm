use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::mpsc::{Receiver, Sender},
};

use nojson::RawJsonOwned;
use orfail::OrFail;

use crate::{
    json::JsonObject,
    lsp::{self, DocumentUri},
};

const INITIALIZE_REQUEST_ID: u32 = 0;

#[derive(Debug)]
pub struct LspServerSpec {
    pub command: PathBuf,
    pub args: Vec<String>,
    pub initialize_options: Option<RawJsonOwned>,
}

impl LspServerSpec {
    pub fn load(path: &Path) -> orfail::Result<Self> {
        crate::json::parse_file(path, |value| {
            let object = JsonObject::new(value)?;
            Ok(Self {
                command: object.convert_required("command")?,
                args: object.convert_optional_or_default("args")?,
                initialize_options: object
                    .get_optional("initialize_options")?
                    .map(|v| v.extract().into_owned()),
            })
        })
        .or_fail()
    }

    pub fn spawn_process(&self) -> orfail::Result<Child> {
        Command::new(&self.command)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .or_fail_with(|e| {
                format!(
                    "failed to spawn LSP server process '{}': {e}",
                    self.command.display()
                )
            })
    }
}

#[derive(Debug)]
pub enum LspMessage {
    Request {
        method: String,
        params: Option<RawJsonOwned>,
        reply_tx: Sender<Result<RawJsonOwned, RawJsonOwned>>,
    },
    Notification {
        method: String,
        params: Option<RawJsonOwned>,
    },
    ResponseFromLspServer {
        request_id: u32,
        result: Result<RawJsonOwned, RawJsonOwned>,
    },
    ResponseToLspServer {
        request_id: RawJsonOwned,
        result: Result<RawJsonOwned, RawJsonOwned>,
    },
    LspServerStdoutError,
}

pub struct LspServer {
    process: Child,
    message_tx: Sender<LspMessage>,
}

impl LspServer {
    pub fn new(spec: &LspServerSpec) -> orfail::Result<Self> {
        let mut process = spec.spawn_process().or_fail()?;
        let mut stdin = process.stdin.take().or_fail()?;
        let mut stdout = BufReader::new(process.stdout.take().or_fail()?);

        // Initialize the LSP server
        Self::initialize_lsp_server(spec, &mut stdout, &mut stdin).or_fail()?;

        let (message_tx, message_rx) = std::sync::mpsc::channel();
        let message_tx_for_stdout = message_tx.clone();

        // Spawn thread to handle stdin (sending messages to LSP server)
        std::thread::spawn(move || {
            if let Err(e) = Self::run_stdin_loop(stdin, message_rx) {
                eprintln!("[ERROR] LSP server stdin thread error: {e}");
            }
        });

        // Spawn thread to handle stdout (receiving messages from LSP server)
        std::thread::spawn(move || {
            if let Err(e) = Self::run_stdout_loop(stdout, message_tx_for_stdout.clone()) {
                eprintln!("[ERROR] LSP server stdout thread error: {e}");
                let _ = message_tx_for_stdout.send(LspMessage::LspServerStdoutError);
            }
        });

        Ok(Self {
            process,
            message_tx,
        })
    }

    pub fn send_request(
        &self,
        method: String,
        params: Option<RawJsonOwned>,
    ) -> orfail::Result<Receiver<Result<RawJsonOwned, RawJsonOwned>>> {
        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        self.message_tx
            .send(LspMessage::Request {
                method,
                params,
                reply_tx,
            })
            .or_fail()?;
        Ok(reply_rx)
    }

    pub fn send_notification(
        &self,
        method: String,
        params: Option<RawJsonOwned>,
    ) -> orfail::Result<()> {
        self.message_tx
            .send(LspMessage::Notification { method, params })
            .or_fail()
    }

    pub fn message_sender(&self) -> Sender<LspMessage> {
        self.message_tx.clone()
    }

    pub fn shutdown(mut self) -> orfail::Result<()> {
        // Send shutdown request
        let shutdown_reply = self.send_request("shutdown".to_string(), None).or_fail()?;
        let _ = shutdown_reply.recv().or_fail()?;

        // Send exit notification
        self.send_notification("exit".to_string(), None).or_fail()?;

        // Wait for process to terminate
        self.process.wait().or_fail()?;
        Ok(())
    }

    fn initialize_lsp_server<R, W>(
        spec: &LspServerSpec,
        mut reader: R,
        mut writer: W,
    ) -> orfail::Result<()>
    where
        R: BufRead,
        W: Write,
    {
        let workspace_folder_uri =
            DocumentUri::new(std::env::current_dir().or_fail()?).or_fail()?;

        let params = nojson::object(|f| {
            f.member("clientInfo", Self::client_info())?;
            f.member(
                "workspaceFolders",
                [Self::workspace_folder(&workspace_folder_uri)],
            )?;
            f.member(
                "capabilities",
                nojson::RawJson::parse(include_str!("capabilities.json")).expect("bug"),
            )?;
            if let Some(options) = &spec.initialize_options {
                f.member("initializationOptions", options)?;
            }
            Ok(())
        });
        let json = lsp::send_request(&mut writer, INITIALIZE_REQUEST_ID, "initialize", params)
            .or_fail()?;
        println!("{json}");

        let json = lsp::recv_message(&mut reader).or_fail()?.or_fail()?;
        println!("{json}");

        let json = lsp::send_notification(&mut writer, "initialized", ()).or_fail()?;
        println!("{json}");

        Ok(())
    }

    fn run_stdin_loop(
        mut stdin: ChildStdin,
        message_rx: Receiver<LspMessage>,
    ) -> orfail::Result<()> {
        let mut ongoing_requests = HashMap::new();
        let mut next_request_id = INITIALIZE_REQUEST_ID + 1;

        while let Ok(msg) = message_rx.recv() {
            match msg {
                LspMessage::Request {
                    method,
                    params,
                    reply_tx,
                } => {
                    let json = lsp::send_request(&mut stdin, next_request_id, &method, params)
                        .or_fail()?;
                    println!("{json}");
                    ongoing_requests.insert(next_request_id, reply_tx);
                    next_request_id += 1;
                }
                LspMessage::Notification { method, params } => {
                    let json = lsp::send_notification(&mut stdin, &method, params).or_fail()?;
                    println!("{json}");
                }
                LspMessage::ResponseFromLspServer { request_id, result } => {
                    if let Some(reply_tx) = ongoing_requests.remove(&request_id) {
                        let _ = reply_tx.send(result);
                    }
                }
                LspMessage::ResponseToLspServer { request_id, result } => {
                    let json = lsp::send_response(&mut stdin, request_id, result).or_fail()?;
                    println!("{json}");
                }
                LspMessage::LspServerStdoutError => break,
            }
        }
        Ok(())
    }

    fn run_stdout_loop(
        mut stdout: BufReader<ChildStdout>,
        message_tx: Sender<LspMessage>,
    ) -> orfail::Result<()> {
        while let Some(json) = lsp::recv_message(&mut stdout).or_fail()? {
            println!("{json}");

            let value = json.value();
            let Some(request_id) = value.to_member("id").or_fail()?.get() else {
                continue;
            };

            let msg = if let Some(method) = value.to_member("method").or_fail()?.get() {
                let request_id = request_id.extract().into_owned();
                let method = method.to_unquoted_string_str().or_fail()?;
                match method.as_ref() {
                    "window/workDoneProgress/create" => {
                        let result = Ok(RawJsonOwned::parse("null").expect("bug"));
                        LspMessage::ResponseToLspServer { request_id, result }
                    }
                    _ => {
                        let result = Err(RawJsonOwned::parse(
                            r#"{"code":-32601, "message":"method not found"}"#,
                        )
                        .expect("bug"));
                        LspMessage::ResponseToLspServer { request_id, result }
                    }
                }
            } else {
                let request_id = u32::try_from(request_id).or_fail()?;
                let result = if let Some(e) = value.to_member("error").or_fail()?.get() {
                    Err(e.extract().into_owned())
                } else {
                    let result = value.to_member("result").or_fail()?.required().or_fail()?;
                    Ok(result.extract().into_owned())
                };
                LspMessage::ResponseFromLspServer { request_id, result }
            };

            if message_tx.send(msg).is_err() {
                break;
            }
        }
        Ok(())
    }

    fn client_info() -> impl nojson::DisplayJson {
        nojson::object(|f| {
            f.member("name", env!("CARGO_PKG_NAME"))?;
            f.member("version", env!("CARGO_PKG_VERSION"))
        })
    }

    fn workspace_folder(workspace_folder_uri: &DocumentUri) -> impl nojson::DisplayJson + '_ {
        nojson::object(move |f| {
            f.member("uri", workspace_folder_uri)?;
            f.member("name", "main")
        })
    }
}
