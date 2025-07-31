use orfail::OrFail;

use crate::{
    args::RAW_FLAG,
    json::JsonObject,
    lsp::{DocumentUri, PositionRange},
    proxy_client::{PORT_OPT, ProxyClient},
    target::{TARGET_ARG, TargetLocation},
};

pub fn try_run(mut args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("definition")
        .doc("Get definition location for a symbol (textDocument/definition)")
        .take(&mut args)
        .is_present()
    {
        return Ok(Some(args));
    }

    let port: u16 = PORT_OPT.take(&mut args).then(|a| a.value().parse())?;
    let raw = RAW_FLAG.take(&mut args).is_present();
    let target: TargetLocation = TARGET_ARG.take(&mut args).then(|a| a.value().parse())?;

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

    let definitions: Vec<_> = result.value().try_into()?;
    if definitions.is_empty() {
        println!("Not found");
        return Ok(None);
    }

    let base_dir = std::env::current_dir().or_fail()?;
    for (i, def) in definitions.into_iter().enumerate() {
        let def_obj = JsonObject::new(def).or_fail()?;

        let target_uri: DocumentUri = def_obj.convert_required("targetUri").or_fail()?;
        let target_selection_range: PositionRange =
            def_obj.convert_required("targetSelectionRange").or_fail()?;

        let target_text = target_uri.read_to_string().or_fail()?;
        let selection_text = target_selection_range
            .get_range_text(&target_text)
            .or_fail()?;

        println!("## Definition {}: `{}`", i + 1, selection_text);
        println!();
        println!(
            "{}:{}:{}:",
            target_uri.relative_path(&base_dir).display(),
            target_selection_range.start.line + 1,
            target_selection_range.start.character + 1
        );

        let context_lines = 5;
        let mut context_range = target_selection_range;
        context_range.start.line = context_range.start.line.saturating_sub(context_lines);
        context_range.start.character = 0;
        context_range.end.line += context_lines;
        context_range.end.character = usize::MAX;

        dbg!(context_range);
        let context_text = context_range.get_range_text(&target_text).or_fail()?;
        println!("```");
        println!("{context_text}");
        println!("```");
        println!();
    }

    Ok(None)
}
