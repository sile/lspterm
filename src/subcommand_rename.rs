use std::{io::BufReader, net::TcpStream, path::PathBuf};

use orfail::OrFail;

use crate::{
    lsp::{self, DocumentUri},
    proxy::DEFAULT_PORT,
};

pub fn try_run(mut args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("rename")
        .doc("TODO")
        .take(&mut args)
        .is_present()
    {
        return Ok(Some(args));
    }

    let port: u16 = noargs::opt("port")
        .short('p')
        .default(DEFAULT_PORT)
        .env("LSPTERM_PORT")
        .doc("Port number of the LSP proxy server to connect to")
        .take(&mut args)
        .then(|a| a.value().parse())?;
    let apply = noargs::flag("apply")
        .short('a')
        .take(&mut args)
        .is_present();
    let file = noargs::arg("FILE")
        .take(&mut args)
        .then(|a| a.value().parse::<PathBuf>())?;
    let line = noargs::arg("LINE")
        .take(&mut args)
        .then(|a| a.value().parse::<u32>())?;
    let character = noargs::arg("CHARACTER")
        .take(&mut args)
        .then(|a| a.value().parse::<u32>())?;
    let new_name: String = noargs::arg("NEW_NAME")
        .take(&mut args)
        .then(|a| a.value().parse())?;

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(None);
    }

    let file = DocumentUri::new(file).or_fail()?;

    let mut stream = BufReader::new(TcpStream::connect(("127.0.0.1", port)).or_fail()?);

    // Send rename request
    let request_id = 1;
    let params = nojson::object(|f| {
        f.member("textDocument", nojson::object(|f| f.member("uri", &file)))?;
        f.member(
            "position",
            nojson::object(|f| {
                f.member("line", line)?;
                f.member("character", character)
            }),
        )?;
        f.member("newName", &new_name)
    });

    lsp::send_request(stream.get_mut(), request_id, "textDocument/rename", params).or_fail()?;

    // Receive rename response
    let response_json = lsp::recv_message(&mut stream).or_fail()?.or_fail()?;
    let response_value = response_json.value();

    // Check if there's an error in the response
    if let Some(error) = response_value.to_member("error").or_fail()?.get() {
        eprintln!("LSP server returned error: {error}");
        return Ok(None);
    }

    // Parse and display the result
    let result = response_value
        .to_member("result")
        .or_fail()?
        .required()
        .or_fail()?;
    println!("{result}");

    if apply {
        apply_workspace_edit(result).or_fail()?;
    }

    Ok(None)
}

fn apply_workspace_edit(result: nojson::RawJsonValue) -> orfail::Result<()> {
    use std::collections::HashMap;
    use std::fs;

    // Parse documentChanges array
    let document_changes = result
        .to_member("documentChanges")
        .or_fail()?
        .required()
        .or_fail()?
        .to_array()
        .or_fail()?;

    // Group edits by file
    let mut files_to_edit: HashMap<String, Vec<_>> = HashMap::new();

    for change in document_changes {
        let text_document = change
            .to_member("textDocument")
            .or_fail()?
            .required()
            .or_fail()?;

        let uri = text_document
            .to_member("uri")
            .or_fail()?
            .required()
            .or_fail()?
            .to_unquoted_string_str()
            .or_fail()?;

        let edits = change
            .to_member("edits")
            .or_fail()?
            .required()
            .or_fail()?
            .to_array()
            .or_fail()?;

        for edit in edits {
            let range = edit.to_member("range").or_fail()?.required().or_fail()?;

            let start = range.to_member("start").or_fail()?.required().or_fail()?;
            let start_line =
                usize::try_from(start.to_member("line").or_fail()?.required().or_fail()?)
                    .or_fail()?;
            let start_char = usize::try_from(
                start
                    .to_member("character")
                    .or_fail()?
                    .required()
                    .or_fail()?,
            )
            .or_fail()?;

            let end = range.to_member("end").or_fail()?.required().or_fail()?;
            let end_line = usize::try_from(end.to_member("line").or_fail()?.required().or_fail()?)
                .or_fail()?;
            let end_char =
                usize::try_from(end.to_member("character").or_fail()?.required().or_fail()?)
                    .or_fail()?;

            let new_text = edit
                .to_member("newText")
                .or_fail()?
                .required()
                .or_fail()?
                .to_unquoted_string_str()
                .or_fail()?;

            files_to_edit.entry(uri.to_string()).or_default().push((
                start_line,
                start_char,
                end_line,
                end_char,
                new_text.to_string(),
            ));
        }
    }

    // Apply edits to each file
    for (uri, mut edits) in files_to_edit {
        // Convert file:// URI to path
        let file_path = if let Some(path) = uri.strip_prefix("file://") {
            PathBuf::from(path)
        } else {
            return Err(orfail::Failure::new(format!("Invalid URI format: {uri}")));
        };

        // Read file content
        let content = fs::read_to_string(&file_path)
            .or_fail_with(|e| format!("Failed to read file '{}': {}", file_path.display(), e))?;

        // Change this line to own the strings instead of borrowing
        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        // Sort edits by position (end to start) to avoid offset issues
        edits.sort_by(|a, b| match b.0.cmp(&a.0) {
            std::cmp::Ordering::Equal => b.1.cmp(&a.1),
            other => other,
        });

        // Apply edits
        for (start_line, start_char, end_line, end_char, new_text) in edits {
            if start_line == end_line {
                // Single line edit
                if start_line < lines.len() {
                    let line = &lines[start_line];
                    let mut chars: Vec<char> = line.chars().collect();
                    if start_char <= chars.len() && end_char <= chars.len() {
                        chars.splice(start_char..end_char, new_text.chars());
                        lines[start_line] = chars.into_iter().collect::<String>();
                    }
                }
            } else {
                // Multi-line edit (more complex, but handle basic case)
                if start_line < lines.len() && end_line < lines.len() {
                    let start_line_content = &lines[start_line];
                    let end_line_content = &lines[end_line];

                    let start_chars: Vec<char> = start_line_content.chars().collect();
                    let end_chars: Vec<char> = end_line_content.chars().collect();

                    if start_char <= start_chars.len() && end_char <= end_chars.len() {
                        // Create new content combining start of first line, new text, and end of last line
                        let mut new_line = String::new();
                        new_line.push_str(&start_chars[..start_char].iter().collect::<String>());
                        new_line.push_str(&new_text);
                        new_line.push_str(&end_chars[end_char..].iter().collect::<String>());

                        // Replace the range of lines with the new single line
                        lines.splice(start_line..=end_line, std::iter::once(new_line));
                    }
                }
            }
        }

        // Write back to file
        let new_content = lines.join("\n");
        if content.ends_with('\n') && !new_content.ends_with('\n') {
            let new_content = new_content + "\n";
            fs::write(&file_path, new_content).or_fail_with(|e| {
                format!("Failed to write file '{}': {}", file_path.display(), e)
            })?;
        } else {
            fs::write(&file_path, new_content).or_fail_with(|e| {
                format!("Failed to write file '{}': {}", file_path.display(), e)
            })?;
        }

        eprintln!("Applied changes to: {}", file_path.display());
    }

    Ok(())
}
