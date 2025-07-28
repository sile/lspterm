use std::{
    io::BufReader,
    net::{TcpListener, TcpStream},
    sync::mpsc::Sender,
};

use orfail::OrFail;

use crate::{
    json::JsonObject,
    lsp::{self, DocumentUri},
    lsp_server::{LspMessage, LspServer, LspServerSpec},
};

pub const DEFAULT_PORT: &str = "9257";

#[derive(Debug)]
pub struct ProxyServerConfig {
    pub port: u16,
    pub workspace_folder_uri: DocumentUri,
    pub lsp_server_spec: LspServerSpec,
}

/// LSP proxy server that forwards requests to a configured LSP server
#[derive(Debug)]
pub struct ProxyServer {
    config: ProxyServerConfig,
}

impl ProxyServer {
    pub fn new(config: ProxyServerConfig) -> Self {
        Self { config }
    }

    pub fn run(self) -> orfail::Result<()> {
        let listener = TcpListener::bind(("127.0.0.1", self.config.port)).or_fail()?;

        let lsp_server = LspServer::new(
            self.config.lsp_server_spec,
            self.config.workspace_folder_uri,
        )
        .or_fail()?;

        for incoming in listener.incoming() {
            let incoming = incoming.or_fail()?;
            let lsp_server_msg_tx = lsp_server.message_sender();
            std::thread::spawn(move || {
                if let Err(e) = run_proxy_client(incoming, lsp_server_msg_tx) {
                    eprintln!("[WARN] failed to run proxy client: {e}");
                }
            });
        }

        lsp_server.shutdown().or_fail()?;
        Ok(())
    }
}

/// Handle a single client connection to the proxy server
fn run_proxy_client(stream: TcpStream, msg_tx: Sender<LspMessage>) -> orfail::Result<()> {
    let mut stream = BufReader::new(stream);
    while let Some(json) = lsp::recv_message(&mut stream).or_fail()? {
        let object = JsonObject::new(json.value()).or_fail()?;
        let method = object.convert_required("method").or_fail()?;
        let params = object.convert_optional("params").or_fail()?;
        let id = object.get_optional("id");
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
