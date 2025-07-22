use std::path::PathBuf;

use orfail::OrFail;

use lspterm::{app::App, lsp_client::LspClient};

fn main() -> noargs::Result<()> {
    let mut args = noargs::raw_args();
    args.metadata_mut().app_name = env!("CARGO_PKG_NAME");
    args.metadata_mut().app_description = env!("CARGO_PKG_DESCRIPTION");

    if noargs::VERSION_FLAG.take(&mut args).is_present() {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    noargs::HELP_FLAG.take_help(&mut args);

    let lsp_server_command: PathBuf = noargs::arg("LSP_SERVER_COMMAND")
        .example("/path/to/lsp-server")
        .take(&mut args)
        .then(|a| a.value().parse())?;

    let mut lsp_server_args: Vec<String> = Vec::new();
    while let Some(arg) = noargs::arg("[LSP_SERVER_ARG]...")
        .take(&mut args)
        .present_and_then(|a| a.value().parse())?
    {
        lsp_server_args.push(arg);
    }

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(());
    }

    let lsp_client = LspClient::new(lsp_server_command, lsp_server_args).or_fail()?;
    let app = App::new(lsp_client).or_fail()?;
    app.run().or_fail()?;

    Ok(())
}
