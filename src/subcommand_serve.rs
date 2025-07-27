use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

use nojson::RawJsonOwned;
use orfail::OrFail;

use crate::lsp::{self, DocumentUri};

const INITIALIZE_REQUEST_ID: u32 = 1;
const SHUTDOWN_REQUEST_ID: u32 = 0;

#[derive(Debug, Default, Clone)]
struct Args {
    port: u16,
    lsp_server_command: PathBuf,
    lsp_server_args: Vec<PathBuf>,
}

pub fn try_run(mut raw_args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("serve").take(&mut raw_args).is_present() {
        return Ok(Some(raw_args));
    }

    // TODO: Add '--' handling

    let mut args = Args::default();
    args.port = noargs::opt("port")
        .short('p')
        .default("9257")
        .env("LSPTERM_PORT")
        .take(&mut raw_args)
        .then(|a| a.value().parse())?;
    args.lsp_server_command = noargs::arg("LSP_SERVER_COMMAND")
        .example("/path/to/lsp-server")
        .take(&mut raw_args)
        .then(|a| a.value().parse())?;
    while let Some(a) = noargs::arg("[LSP_SERVER_ARG]...")
        .take(&mut raw_args)
        .present()
    {
        args.lsp_server_args.push(a.value().parse()?);
    }

    if let Some(help) = raw_args.finish()? {
        print!("{help}");
        return Ok(None);
    }

    let mut lsp_server = spawn_lsp_server(&args).or_fail()?;
    let mut lsp_server_stdin = lsp_server.stdin.take().or_fail()?;
    let mut lsp_server_stdout = BufReader::new(lsp_server.stdout.take().or_fail()?);
    initialize_lsp_server(&mut lsp_server_stdout, &mut lsp_server_stdin).or_fail()?;

    let _listener = TcpListener::bind(("127.0.0.1", args.port)).or_fail()?;

    let (lsp_server_msg_tx, lsp_server_msg_rx) = std::sync::mpsc::channel();
    let lsp_server_thread_handle = std::thread::spawn(move || {
        if let Err(e) = run_lsp_server(
            &mut lsp_server_stdin,
            &mut lsp_server_stdout,
            lsp_server_msg_tx,
            lsp_server_msg_rx,
        )
        .or_fail()
        {
            eprintln!("failed to run lsp server: {e}");
        }
        (lsp_server_stdin, lsp_server_stdout)
    });
    lsp_server_thread_handle.is_finished();

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
        params: RawJsonOwned,
        reply_tx: std::sync::mpsc::Sender<Result<RawJsonOwned, RawJsonOwned>>,
    },
    Notification {
        method: String,
        params: RawJsonOwned,
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

fn spawn_lsp_server(args: &Args) -> orfail::Result<Child> {
    let mut command = Command::new(&args.lsp_server_command);
    command
        .args(&args.lsp_server_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    command.spawn().or_fail_with(|e| {
        format!(
            "failed to spawn LSP server process '{}': {e}",
            args.lsp_server_command.display()
        )
    })
}

fn initialize_lsp_server<R, W>(mut reader: R, mut writer: W) -> orfail::Result<()>
where
    R: BufRead,
    W: Write,
{
    let workspace_folder_uri = DocumentUri::new(std::env::current_dir().or_fail()?).or_fail()?;
    let client_info = |f: &mut nojson::JsonObjectFormatter<'_, '_, '_>| {
        f.member("name", env!("CARGO_PKG_NAME"))?;
        f.member("version", env!("CARGO_PKG_VERSION"))
    };
    let workspace_folder = |f: &mut nojson::JsonObjectFormatter<'_, '_, '_>| {
        f.member("uri", &workspace_folder_uri)?;
        f.member("name", "main")
    };
    let capabilities = |f: &mut nojson::JsonObjectFormatter<'_, '_, '_>| {
        f.member(
            "textDocument",
            nojson::object(|f| {
                f.member(
                    "definition",
                    nojson::object(|f| f.member("linkSupport", true)),
                )
            }),
        )?;
        f.member(
            "window",
            nojson::object(|f| f.member("workDoneProgress", true)),
        )?;
        f.member(
            "general",
            nojson::object(|f| f.member("positionEncodings", ["utf-8"])),
        )
    };
    let initialize_params = nojson::object(|f| {
        f.member("clientInfo", nojson::object(client_info))?;
        f.member("workspaceFolders", [nojson::object(workspace_folder)])?;
        f.member("capabilities", nojson::object(capabilities))?;
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
