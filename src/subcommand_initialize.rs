use orfail::OrFail;

use crate::{
    json::InitializeRequest,
    lsp_client::{LspClient, LspServerSpec},
};

pub fn try_run(mut args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("initialize").take(&mut args).is_present() {
        return Ok(Some(args));
    }

    let lsp_server_spec = LspServerSpec::parse_args(&mut args)?;

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(None);
    }

    let mut lsp_client = LspClient::new(lsp_server_spec).or_fail()?;

    let req = InitializeRequest {
        id: 0,
        workspace_folder: std::env::current_dir().or_fail()?,
    };
    lsp_client.send_request(req).or_fail()?;

    Ok(None)
}
