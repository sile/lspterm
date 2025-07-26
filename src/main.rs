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
    let Some(args) = lspterm::subcommand_initialize::try_run(args)? else {
        return Ok(());
    };
    let Some(args) = lspterm::subcommand_find_def::try_run(args)? else {
        return Ok(());
    };

    if let Some(help) = args.finish()? {
        print!("{help}");
    }

    Ok(())
}
