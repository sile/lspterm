use orfail::OrFail;

use crate::{
    args::RAW_FLAG,
    json::JsonObject,
    proxy_client::{PORT_OPT, ProxyClient},
    target::{TARGET_ARG, TargetLocation},
};

pub fn try_run(mut args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("completion")
        .doc("Get completion suggestions at a specific position (textDocument/completion)")
        .take(&mut args)
        .is_present()
    {
        return Ok(Some(args));
    }

    let port: u16 = PORT_OPT.take(&mut args).then(|a| a.value().parse())?;
    let raw = RAW_FLAG.take(&mut args).is_present();
    let target: TargetLocation = TARGET_ARG.take(&mut args).then(|a| a.value().parse())?;
    // TODO: --apply

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(None);
    }
    target.file.check_existence().or_fail()?;

    let mut client = ProxyClient::connect(port).or_fail()?;

    let params = nojson::object(|f| target.fmt_json_object(f));
    let result = client.call("textDocument/completion", params).or_fail()?;

    if raw {
        println!("{result}");
        return Ok(None);
    }

    if result.value().kind().is_null() {
        println!("No completions found");
        return Ok(None);
    }

    let items: Vec<_> = result
        .value()
        .to_member("items")
        .or_fail()?
        .required()
        .or_fail()?
        .to_array()
        .or_fail()?
        .collect();

    if items.is_empty() {
        println!("No completions found");
        return Ok(None);
    }

    println!("# Completions\n");

    for (i, item) in items.iter().enumerate() {
        let item_obj = JsonObject::new(*item).or_fail()?;

        let label: String = item_obj.convert_required("label").or_fail()?;
        let kind: Option<u32> = item_obj.convert_optional("kind").or_fail()?;
        let detail: Option<String> = item_obj.convert_optional("detail").or_fail()?;
        // let documentation: Option<JsonObject<'_, '_>> =
        //     item_obj.convert_optional("documentation").or_fail()?;

        println!("## {}. `{}`", i + 1, label);

        if let Some(kind_num) = kind {
            let kind_str = match kind_num {
                1 => "Text",
                2 => "Method",
                3 => "Function",
                4 => "Constructor",
                5 => "Field",
                6 => "Variable",
                7 => "Class",
                8 => "Interface",
                9 => "Module",
                10 => "Property",
                11 => "Unit",
                12 => "Value",
                13 => "Enum",
                14 => "Keyword",
                15 => "Snippet",
                16 => "Color",
                17 => "File",
                18 => "Reference",
                19 => "Folder",
                20 => "EnumMember",
                21 => "Constant",
                22 => "Struct",
                23 => "Event",
                24 => "Operator",
                25 => "TypeParameter",
                _ => "Unknown",
            };
            println!("**Kind:** {}", kind_str);
        }

        if let Some(detail_text) = detail {
            println!("**Type:** `{}`", detail_text);
        }

        // if let Some(doc_obj) = documentation {
        //     if let Ok(doc_value) = doc_obj.convert_required::<String>("value") {
        //         println!("\n{}", doc_value);
        //     }
        // }

        println!();
    }
    Ok(None)
}
