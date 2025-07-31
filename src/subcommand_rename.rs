use orfail::OrFail;

use crate::{
    args::{APPLY_FLAG, RAW_FLAG},
    document::{DocumentChange, DocumentChanges},
    proxy_client::{PORT_OPT, ProxyClient},
    target::{TARGET_OPT, TargetLocation},
};

pub fn try_run(mut args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("rename")
        .doc("Rename a symbol (textDocument/rename)")
        .take(&mut args)
        .is_present()
    {
        return Ok(Some(args));
    }

    let target: TargetLocation = TARGET_OPT.take(&mut args).then(|a| a.value().parse())?;
    let port: u16 = PORT_OPT.take(&mut args).then(|a| a.value().parse())?;
    let apply = APPLY_FLAG.take(&mut args).is_present();
    let raw = RAW_FLAG.take(&mut args).is_present();
    let new_name: String = noargs::arg("NEW_NAME")
        .doc("New name for the symbol being renamed")
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
        .or_fail_with(|e| format!("Failed to parse document changes: {e}"))?;
    if !raw {
        print_markdown_changes(&document_changes, !apply);
    }

    if apply {
        document_changes.apply().or_fail()?;
        eprintln!("=> Renamed");
    }

    Ok(None)
}

fn print_markdown_changes(document_changes: &DocumentChanges, dry_run: bool) {
    let base_dir = std::env::current_dir().unwrap_or_default();

    println!(
        "# Rename Changes{}\n",
        if dry_run { " (dry-run)" } else { "" }
    );

    for change in &document_changes.changes {
        match change {
            DocumentChange::TextDocument(text_change) => {
                let Ok(text) = text_change.text_document.uri.read_to_string() else {
                    continue;
                };
                let path = text_change.text_document.uri.relative_path(&base_dir);

                println!("## {}\n", path.display());

                for edit in &text_change.edits {
                    println!(
                        "### Line {}, Character {}\n",
                        edit.range.start.line + 1,
                        edit.range.start.character + 1
                    );

                    if edit.range.is_multiline() {
                        println!("```diff");
                        println!("- [multiline change]");
                        println!("+ {}", edit.new_text);
                        println!("```\n");
                    } else {
                        let old_line = edit.range.get_start_line(&text).unwrap_or_default();
                        let new_line = old_line
                            .chars()
                            .take(edit.range.start.character)
                            .chain(edit.new_text.chars())
                            .chain(old_line.chars().skip(edit.range.end.character))
                            .collect::<String>();

                        println!("```diff");
                        println!("- {old_line}");
                        println!("+ {new_line}");
                        println!("```\n");
                    }
                }
            }
            DocumentChange::RenameFile(rename_change) => {
                let old_path = rename_change.old_uri.relative_path(&base_dir);
                let new_path = rename_change.new_uri.relative_path(&base_dir);

                println!("## File Rename\n");
                println!("```diff");
                println!("- {}", old_path.display());
                println!("+ {}", new_path.display());
                println!("```\n");
            }
        }
    }
}
