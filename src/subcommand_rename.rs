use orfail::OrFail;

use crate::{
    document::{DocumentChange, DocumentChanges, TextEdit},
    proxy_client::ProxyClient,
    proxy_server::DEFAULT_PORT,
    target::TargetLocation,
};

pub fn try_run(mut args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("rename")
        .doc("Rename symbol using LSP")
        .take(&mut args)
        .is_present()
    {
        return Ok(Some(args));
    }

    let target: TargetLocation = noargs::opt("target-location")
        .short('t')
        .ty("FILE[:LINE[:CHARACTER]]")
        .env("LSPTERM_TARGET_LOCATION")
        .example("/path/to/file:0:5")
        .doc("Target location for rename")
        .take(&mut args)
        .then(|a| a.value().parse())?;
    let apply = noargs::flag("apply")
        .short('a')
        .take(&mut args)
        .is_present();
    let raw = noargs::flag("raw").short('r').take(&mut args).is_present();
    let port: u16 = noargs::opt("port")
        .short('p')
        .ty("INTEGER")
        .default(DEFAULT_PORT)
        .env("LSPTERM_PORT")
        .doc("Port number of the LSP proxy server to connect to")
        .take(&mut args)
        .then(|a| a.value().parse())?;
    let new_name: String = noargs::arg("NEW_NAME")
        .example("new-name")
        .take(&mut args)
        .then(|a| a.value().parse())?;

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(None);
    }
    target.file.check_existence().or_fail()?;

    let mut client = ProxyClient::connect(port).or_fail()?;

    let params = nojson::object(|f| {
        target.fmt_json_object(f)?;
        f.member("newName", &new_name)
    });
    let result = client.call("textDocument/rename", params).or_fail()?;
    if raw {
        println!("{result}");
    }

    let document_changes = DocumentChanges::try_from(result.value())
        .or_fail_with(|e| format!("Failed to parse document changes: {}", e))?;
    if !raw {
        println!(
            "{}",
            nojson::json(|f| {
                f.set_indent_size(2);
                f.set_spacing(true);
                fmt_document_changes(f, &document_changes)
            })
        );
    }

    if apply {
        document_changes.apply().or_fail()?;
    }

    Ok(None)
}

fn fmt_document_changes(
    f: &mut nojson::JsonFormatter<'_, '_>,
    document_changes: &DocumentChanges,
) -> std::fmt::Result {
    let base_dir = std::env::current_dir().unwrap_or_default();
    f.object(|f| {
        for change in &document_changes.changes {
            match change {
                DocumentChange::TextDocument(text_change) => {
                    let Ok(text) = text_change.text_document.uri.read_to_string() else {
                        continue;
                    };
                    let path = text_change.text_document.uri.relative_path(&base_dir);
                    for edit in &text_change.edits {
                        f.member(
                            format!(
                                "{}:{}:{}",
                                path.display(),
                                edit.range.start.line + 1,
                                edit.range.start.character + 1
                            ),
                            nojson::json(|f| fmt_text_edit(f, &text, edit)),
                        )?;
                    }
                }
                DocumentChange::RenameFile(rename_change) => {
                    let old_path = rename_change.old_uri.relative_path(&base_dir);
                    let new_path = rename_change.new_uri.relative_path(&base_dir);
                    f.member(
                        format!("{}", old_path.display()),
                        nojson::json(|f| f.object(|f| f.member("new_path", &new_path))),
                    )?;
                }
            }
        }
        Ok(())
    })
}

fn fmt_text_edit(
    f: &mut nojson::JsonFormatter<'_, '_>,
    text: &str,
    edit: &TextEdit,
) -> std::fmt::Result {
    if edit.range.is_multiline() {
        todo!();
    }

    let old_line = edit.range.get_start_line(text).unwrap_or_default();
    let new_line = old_line
        .chars()
        .take(edit.range.start.character)
        .chain(edit.new_text.chars())
        .chain(old_line.chars().skip(edit.range.end.character))
        .collect::<String>();

    f.object(|f| {
        f.member("old_line", old_line)?;
        f.member("new_line", &new_line)?;
        Ok(())
    })
}
