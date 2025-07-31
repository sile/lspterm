use orfail::OrFail;

use crate::{
    args::RAW_FLAG,
    proxy_client::{PORT_OPT, ProxyClient},
    target::{TARGET_OPT, TargetLocation},
};

pub fn try_run(mut args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("definition")
        .doc("Get definition location for a symbol (textDocument/definition)")
        .take(&mut args)
        .is_present()
    {
        return Ok(Some(args));
    }

    let target: TargetLocation = TARGET_OPT.take(&mut args).then(|a| a.value().parse())?;
    let port: u16 = PORT_OPT.take(&mut args).then(|a| a.value().parse())?;
    let raw = RAW_FLAG.take(&mut args).is_present();

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(None);
    }
    target.file.check_existence().or_fail()?;

    let mut client = ProxyClient::connect(port).or_fail()?;

    let params = nojson::object(|f| target.fmt_json_object(f));
    let result = client.call("textDocument/definition", params).or_fail()?;

    if raw {
        println!("{result}");
        return Ok(None);
    }

    if result.value().kind().is_null() {
        println!("Not found");
        return Ok(None);
    }

    // TODO: Add formatted output similar to hover subcommand
    // For now, just print the raw result when not in raw mode
    println!("{result}");

    Ok(None)
}
