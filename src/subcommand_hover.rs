use orfail::OrFail;

use crate::{
    args::RAW_FLAG,
    json::JsonObject,
    lsp::PositionRange,
    proxy_client::{PORT_OPT, ProxyClient},
    target::{TARGET_OPT, TargetLocation},
};

pub fn try_run(mut args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("hover")
        .doc("Get hover information for a symbol at the specified location")
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
    let result = client.call("textDocument/hover", params).or_fail()?;

    if raw {
        println!("{result}");
        return Ok(None);
    }

    if result.value().kind().is_null() {
        println!("Not found");
        return Ok(None);
    }

    let object = JsonObject::new(result.value()).or_fail()?;
    let range: PositionRange = object.convert_required("range").or_fail()?;
    let contents: JsonObject<'_, '_> = object.convert_required("contents").or_fail()?;
    let description: String = contents.convert_required("value").or_fail()?;

    let text = target.file.read_to_string().or_fail()?;
    let symbol = range.get_range_text(&text).or_fail()?;

    println!("# `{symbol}`\n");
    println!("{description}");

    Ok(None)
}
