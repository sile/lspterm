use orfail::OrFail;

use crate::{
    args::RAW_FLAG,
    json::JsonObject,
    lsp::PositionRange,
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

    // Display each definition location
    for (i, def) in definitions.into_iter().enumerate() {
        let def_obj = JsonObject::new(def).or_fail()?;

        // Get target URI and range
        let target_uri: String = def_obj.convert_required("targetUri").or_fail()?;
        let target_range: PositionRange =
            def_obj.convert_required("targetSelectionRange").or_fail()?;

        // Extract file path from URI (remove file:// prefix)
        let file_path = target_uri.strip_prefix("file://").unwrap_or(&target_uri);

        if i > 0 {
            println!(); // Add blank line between multiple definitions
        }

        println!("## Definition {}:", i + 1);
        println!("- **File:** `{}`", file_path);
        println!(
            "- **Location:** Line {}, Column {}",
            target_range.start.line + 1,
            target_range.start.character + 1
        );

        // Try to read and display the target code if it's a local file
        if let Ok(target_text) = std::fs::read_to_string(file_path) {
            if let Some(symbol_text) = target_range.get_range_text(&target_text) {
                if !symbol_text.trim().is_empty() {
                    println!("- **Code:**");
                    println!("```");
                    println!("{}", symbol_text.trim());
                    println!("```");
                }
            }
        }
    }

    Ok(None)
}
