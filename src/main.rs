fn main() -> noargs::Result<()> {
    let mut args = noargs::raw_args();
    args.metadata_mut().app_name = env!("CARGO_PKG_NAME");
    args.metadata_mut().app_description = env!("CARGO_PKG_DESCRIPTION");

    if noargs::VERSION_FLAG.take(&mut args).is_present() {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    noargs::HELP_FLAG.take_help(&mut args);

    let Some(args) = lspterm::subcommand_serve::try_run(args)? else {
        return Ok(());
    };
    let Some(args) = lspterm::subcommand_find_def::try_run(args)? else {
        // textDocument/definition
        return Ok(());
    };
    let Some(args) = lspterm::subcommand_rename::try_run(args)? else {
        // textDocument/rename
        return Ok(());
    };
    let Some(args) = lspterm::subcommand_completion::try_run(args)? else {
        // textDocument/completion
        return Ok(());
    };
    let Some(args) = lspterm::subcommand_hover::try_run(args)? else {
        // textDocument/hover
        return Ok(());
    };
    let Some(args) = lspterm::subcommand_act::try_run(args)? else {
        // textDocument/codeAction
        return Ok(());
    };

    // TODO: process

    // MEMO:
    // - textDocument/references
    // - textDocument/documentSymbol
    // - textDocument/formatting
    // - workspace/symbol
    // - textDocument/implementation
    // - textDocument/typeDefinition
    // - textDocument/declaration
    // - textDocument/documentHighlight
    // - workspace/willRenameFiles

    if let Some(help) = args.finish()? {
        print!("{help}");
    }

    Ok(())
}
