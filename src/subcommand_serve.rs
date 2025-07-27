use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

use nojson::RawJsonOwned;
use orfail::OrFail;

use crate::{
    DEFAULT_PORT,
    lsp::{self, DocumentUri},
};

const INITIALIZE_REQUEST_ID: u32 = 1;
const SHUTDOWN_REQUEST_ID: u32 = 0;

pub fn try_run(mut raw_args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("serve")
        .doc("TODO")
        .take(&mut raw_args)
        .is_present()
    {
        return Ok(Some(raw_args));
    }

    let port: u16 = noargs::opt("port")
        .short('p')
        .default(DEFAULT_PORT)
        .doc("TODO")
        .env("LSPTERM_PORT")
        .take(&mut raw_args)
        .then(|a| a.value().parse())?;
    let lsp_server_config_file_path: PathBuf = noargs::opt("lsp-server-config-file")
        .short('c')
        .example("/path/to/config.json")
        .env("LSPTERM_LSP_SERVER_CONFIG_FILE")
        .take(&mut raw_args)
        .then(|a| a.value().parse())?;

    if let Some(help) = raw_args.finish()? {
        print!("{help}");
        return Ok(None);
    }

    let lsp_server_spec = LspServerSpec::load(&lsp_server_config_file_path).or_fail()?;
    let mut lsp_server = lsp_server_spec.spawn_process().or_fail()?;
    let mut lsp_server_stdin = lsp_server.stdin.take().or_fail()?;
    let mut lsp_server_stdout = BufReader::new(lsp_server.stdout.take().or_fail()?);
    initialize_lsp_server(
        &lsp_server_spec,
        &mut lsp_server_stdout,
        &mut lsp_server_stdin,
    )
    .or_fail()?;

    let listener = TcpListener::bind(("127.0.0.1", port)).or_fail()?;

    let (lsp_server_msg_tx, lsp_server_msg_rx) = std::sync::mpsc::channel();
    let lsp_server_msg_tx_for_ls_server_thread = lsp_server_msg_tx.clone();
    let lsp_server_thread_handle = std::thread::spawn(move || {
        if let Err(e) = run_lsp_server(
            &mut lsp_server_stdin,
            &mut lsp_server_stdout,
            lsp_server_msg_tx_for_ls_server_thread,
            lsp_server_msg_rx,
        )
        .or_fail()
        {
            eprintln!("[ERROR] failed to run lsp server: {e}");
        }
        (lsp_server_stdin, lsp_server_stdout)
    });

    for incoming in listener.incoming() {
        if lsp_server_thread_handle.is_finished() {
            eprintln!("[WARN] LSP server thread has finished");
            break;
        }

        let incoming = incoming.or_fail()?;
        let lsp_server_msg_tx = lsp_server_msg_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = run_proxy_client(incoming, lsp_server_msg_tx) {
                eprintln!("[WARN] failed to run proxy client: {e}");
            }
        });
    }

    let (lsp_server_stdin, lsp_server_stdout) = lsp_server_thread_handle
        .join()
        .unwrap_or_else(|e| std::panic::resume_unwind(e));
    shutdown_lsp_server(lsp_server_stdout, lsp_server_stdin).or_fail()?;
    lsp_server.wait().or_fail()?;

    Ok(None)
}

#[derive(Debug)]
enum Message {
    Request {
        method: String,
        params: Option<RawJsonOwned>,
        reply_tx: std::sync::mpsc::Sender<Result<RawJsonOwned, RawJsonOwned>>,
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

#[derive(Debug)]
struct LspServerSpec {
    command: PathBuf,
    args: Vec<String>,
    initialize_options: Option<RawJsonOwned>,
}

impl LspServerSpec {
    fn load(path: &Path) -> orfail::Result<Self> {
        crate::json::parse_file(path, |value| {
            Ok(Self {
                command: value.to_member("command")?.required()?.try_into()?,
                args: Option::try_from(value.to_member("args")?)?.unwrap_or_default(),
                initialize_options: value
                    .to_member("initializeOptions")?
                    .get()
                    .map(|v| v.extract().into_owned()),
            })
        })
        .or_fail()
    }

    fn spawn_process(&self) -> orfail::Result<Child> {
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

fn run_proxy_client(
    stream: TcpStream,
    msg_tx: std::sync::mpsc::Sender<Message>,
) -> orfail::Result<()> {
    let mut stream = BufReader::new(stream);
    loop {
        // TODO: handle eos
        let json = lsp::recv_message(&mut stream).or_fail()?;
        let value = json.value();
        let method = value
            .to_member("method")
            .or_fail()?
            .required()
            .or_fail()?
            .to_unquoted_string_str()
            .or_fail()?
            .into_owned();
        let params = value
            .to_member("params")
            .or_fail()?
            .map(|v| Ok(v.extract().into_owned()))
            .or_fail()?;
        let id = value.to_member("id").or_fail()?.get();
        let (msg, id_and_reply_rx) = if let Some(id) = id {
            let (reply_tx, reply_rx) = std::sync::mpsc::channel();
            (
                Message::Request {
                    method,
                    params,
                    reply_tx,
                },
                Some((id, reply_rx)),
            )
        } else {
            // TODO: handle didOpen to automatically close when disconnected
            (Message::Notification { method, params }, None)
        };
        if msg_tx.send(msg).is_err() {
            break;
        }

        if let Some((id, reply_rx)) = id_and_reply_rx {
            let Ok(result) = reply_rx.recv() else {
                break;
            };
            lsp::send_response(stream.get_mut(), id, result).or_fail()?;
        }
    }
    Ok(())
}

fn run_lsp_server(
    stdin: &mut ChildStdin,
    stdout: &mut BufReader<ChildStdout>,
    msg_tx: std::sync::mpsc::Sender<Message>,
    msg_rx: std::sync::mpsc::Receiver<Message>,
) -> orfail::Result<()> {
    std::thread::scope(|s| {
        s.spawn(|| {
            if let Err(e) = run_lsp_server_stdout_loop(stdout, msg_tx.clone()).or_fail() {
                eprintln!("[ERROR] failed to read from LSP server stdout: {e}");
                let _ = msg_tx.send(Message::LspServerStdoutError);
            }
        });
        s.spawn(move || {
            if let Err(e) = run_lsp_server_stdin_loop(stdin, msg_rx).or_fail() {
                eprintln!("[ERROR] failed to write to LSP server stdout: {e}");
            }
        });
    });
    Ok(())
}

fn run_lsp_server_stdin_loop(
    mut stdin: &mut ChildStdin,
    msg_rx: std::sync::mpsc::Receiver<Message>,
) -> orfail::Result<()> {
    let mut ongoing_requests = HashMap::new();
    let mut next_request_id = INITIALIZE_REQUEST_ID + 1;
    while let Ok(msg) = msg_rx.recv() {
        match msg {
            Message::Request {
                method,
                params,
                reply_tx,
            } => {
                let json =
                    lsp::send_request(&mut stdin, next_request_id, &method, params).or_fail()?;
                println!("{json}");
                ongoing_requests.insert(next_request_id, reply_tx);
                next_request_id += 1;
            }
            Message::Notification { method, params } => {
                let json = lsp::send_notification(&mut stdin, &method, params).or_fail()?;
                println!("{json}");
            }
            Message::ResponseFromLspServer { request_id, result } => {
                let reply_tx = ongoing_requests.remove(&request_id).or_fail()?;
                let _ = reply_tx.send(result);
            }
            Message::ResponseToLspServer { request_id, result } => {
                let json = lsp::send_response(&mut stdin, request_id, result).or_fail()?;
                println!("{json}");
            }
            Message::LspServerStdoutError => break,
        }
    }
    Ok(())
}

fn run_lsp_server_stdout_loop(
    mut stdout: &mut BufReader<ChildStdout>,
    msg_tx: std::sync::mpsc::Sender<Message>,
) -> orfail::Result<()> {
    loop {
        let json = lsp::recv_message(&mut stdout).or_fail()?;
        println!("{json}");

        let value = json.value();
        let Some(request_id) = value.to_member("id").or_fail()?.get() else {
            continue;
        };

        let res = if let Some(method) = value.to_member("method").or_fail()?.get() {
            let request_id = request_id.extract().into_owned();
            let method = method.to_unquoted_string_str().or_fail()?;
            match method.as_ref() {
                "window/workDoneProgress/create" => {
                    let result = Ok(RawJsonOwned::parse("null").expect("bug"));
                    Message::ResponseToLspServer { request_id, result }
                }
                _ => {
                    let result = Err(RawJsonOwned::parse(
                        r#"{"code":-32601, "message":"method not found"}"#,
                    )
                    .expect("bug"));
                    Message::ResponseToLspServer { request_id, result }
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
            Message::ResponseFromLspServer { request_id, result }
        };
        if msg_tx.send(res).is_err() {
            break;
        }
    }
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
    let workspace_folder_uri = DocumentUri::new(std::env::current_dir().or_fail()?).or_fail()?;

    let initialize_params = nojson::object(|f| {
        f.member("clientInfo", create_client_info())?;
        f.member(
            "workspaceFolders",
            [create_workspace_folder(&workspace_folder_uri)],
        )?;
        f.member(
            "capabilities",
            nojson::object(|f| {
                f.member("textDocument", create_text_document_capabilities())?;
                f.member("window", create_window_capabilities())?;
                f.member("general", create_general_capabilities())
            }),
        )?;
        if let Some(options) = &spec.initialize_options {
            f.member("initializationOptions", options)?;
        }
        Ok(())
    });

    let json = lsp::send_request(
        &mut writer,
        INITIALIZE_REQUEST_ID,
        "initialize",
        initialize_params,
    )
    .or_fail()?;
    println!("{json}");

    let json = lsp::recv_message(&mut reader).or_fail()?;
    println!("{json}");

    let json = lsp::send_notification(&mut writer, "initialized", ()).or_fail()?;
    println!("{json}");

    Ok(())
}

fn shutdown_lsp_server<R, W>(mut reader: R, mut writer: W) -> orfail::Result<()>
where
    R: BufRead,
    W: Write,
{
    let json = lsp::send_request(&mut writer, SHUTDOWN_REQUEST_ID, "shutdown", ()).or_fail()?;
    println!("{json}");

    let json = lsp::recv_message(&mut reader).or_fail()?;
    println!("{json}");

    let json = lsp::send_notification(&mut writer, "exit", ()).or_fail()?;
    println!("{json}");

    Ok(())
}

fn create_text_document_capabilities() -> impl nojson::DisplayJson {
    nojson::object(|f| {
        f.member("definition", create_definition_capabilities())?;
        f.member("codeAction", create_code_action_capabilities())
    })
}

fn create_definition_capabilities() -> impl nojson::DisplayJson {
    nojson::object(|f| f.member("linkSupport", true))
}

fn create_code_action_capabilities() -> impl nojson::DisplayJson {
    nojson::object(|f| {
        f.member("dynamicRegistration", false)?;
        f.member(
            "codeActionLiteralSupport",
            create_code_action_literal_support(),
        )?;
        f.member("isPreferredSupport", true)?;
        f.member("disabledSupport", true)?;
        f.member("dataSupport", true)?;
        f.member("resolveSupport", create_resolve_support())
    })
}

fn create_code_action_literal_support() -> impl nojson::DisplayJson {
    nojson::object(|f| f.member("codeActionKind", create_code_action_kind_support()))
}

fn create_code_action_kind_support() -> impl nojson::DisplayJson {
    nojson::object(|f| {
        f.member(
            "valueSet",
            [
                "quickfix",
                "refactor",
                "refactor.extract",
                "refactor.inline",
                "refactor.rewrite",
                "source",
                "source.organizeImports",
                "source.fixAll",
            ],
        )
    })
}

fn create_resolve_support() -> impl nojson::DisplayJson {
    nojson::object(|f| f.member("properties", ["edit"]))
}

fn create_window_capabilities() -> impl nojson::DisplayJson {
    nojson::object(|f| f.member("workDoneProgress", true))
}

fn create_general_capabilities() -> impl nojson::DisplayJson {
    nojson::object(|f| f.member("positionEncodings", ["utf-8"]))
}

fn create_client_info() -> impl nojson::DisplayJson {
    nojson::object(|f| {
        f.member("name", env!("CARGO_PKG_NAME"))?;
        f.member("version", env!("CARGO_PKG_VERSION"))
    })
}

fn create_workspace_folder(workspace_folder_uri: &DocumentUri) -> impl nojson::DisplayJson + '_ {
    nojson::object(move |f| {
        f.member("uri", workspace_folder_uri)?;
        f.member("name", "main")
    })
}
