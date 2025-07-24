pub fn try_run(mut args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("initialize").take(&mut args).is_present() {
        return Ok(Some(args));
    }

    // let lsp_server_command: PathBuf = noargs::arg("LSP_SERVER_COMMAND")
    //     .example("/path/to/lsp-server")
    //     .take(&mut args)
    //     .then(|a| a.value().parse())?;

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(None);
    }

    Ok(None)
}
