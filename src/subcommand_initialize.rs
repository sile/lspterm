use crate::lsp_client::LspServerSpec;

pub fn try_run(mut args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("initialize").take(&mut args).is_present() {
        return Ok(Some(args));
    }

    let lsp_server_spec = LspServerSpec::parse_args(&mut args)?;

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(None);
    }

    Ok(None)
}
