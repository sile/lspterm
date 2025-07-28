use std::path::PathBuf;

use orfail::OrFail;

use crate::{
    lsp::DocumentUri,
    lsp_server::LspServerSpec,
    proxy_server::{DEFAULT_PORT, ProxyServer, ProxyServerConfig},
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

    let config = ProxyServerConfig {
        port,
        workspace_folder_uri,
        lsp_server_spec,
    };
    let proxy_server = ProxyServer::new(config);
    proxy_server.run().or_fail()?;

    Ok(None)
}
