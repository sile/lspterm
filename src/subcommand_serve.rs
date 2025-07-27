use std::{
    io::BufReader,
    net::{TcpListener, TcpStream},
    path::PathBuf,
};

use orfail::OrFail;

use crate::{
    DEFAULT_PORT,
    lsp::{self, DocumentUri},
    lsp_server::{LspMessage, LspServer, LspServerSpec},
};

pub fn try_run(mut raw_args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("serve")
        .doc("Start LSP proxy server that forwards requests to a configured LSP server")
        .take(&mut raw_args)
        .is_present()
    {
        return Ok(Some(raw_args));
    }

    let workspace_folder: Option<PathBuf> = noargs::opt("workspace-folder")
        .short('w')
        .doc("Path to workspace folder (defaults to current directory)")
        .take(&mut raw_args)
        .present_and_then(|a| a.value().parse())?;
    let port: u16 = noargs::opt("port")
        .short('p')
        .default(DEFAULT_PORT)
        .doc("Port number to bind the proxy server to")
        .env("LSPTERM_PORT")
        .take(&mut raw_args)
        .then(|a| a.value().parse())?;
    let lsp_server_config_file_path: PathBuf = noargs::opt("lsp-server-config-file")
        .short('c')
        .doc("Path to JSON configuration file specifying the LSP server command and options")
        .example("/path/to/config.json")
        .env("LSPTERM_LSP_SERVER_CONFIG_FILE")
        .take(&mut raw_args)
        .then(|a| a.value().parse())?;

    if let Some(help) = raw_args.finish()? {
        print!("{help}");
        return Ok(None);
    }

    let workspace_folder_uri = if let Some(path) = workspace_folder {
        DocumentUri::new_dir(path).or_fail()?
    } else {
        DocumentUri::new(std::env::current_dir().or_fail()?).or_fail()?
    };

    let lsp_server_spec = LspServerSpec::load(&lsp_server_config_file_path).or_fail()?;
    let listener = TcpListener::bind(("127.0.0.1", port)).or_fail()?;

    let lsp_server = LspServer::new(lsp_server_spec, workspace_folder_uri).or_fail()?;
    let lsp_server_msg_tx = lsp_server.message_sender();

    for incoming in listener.incoming() {
        let incoming = incoming.or_fail()?;
        let lsp_server_msg_tx = lsp_server_msg_tx.clone();
        std::thread::spawn(move || {
            if let Err(e) = run_proxy_client(incoming, lsp_server_msg_tx) {
                eprintln!("[WARN] failed to run proxy client: {e}");
            }
        });
    }

    lsp_server.shutdown().or_fail()?;

    Ok(None)
}

fn run_proxy_client(
    stream: TcpStream,
    msg_tx: std::sync::mpsc::Sender<LspMessage>,
) -> orfail::Result<()> {
    let mut stream = BufReader::new(stream);
    while let Some(json) = lsp::recv_message(&mut stream).or_fail()? {
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
                LspMessage::Request {
                    method,
                    params,
                    reply_tx,
                },
                Some((id, reply_rx)),
            )
        } else {
            // TODO: handle didOpen to automatically close when disconnected
            (LspMessage::Notification { method, params }, None)
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
