use std::{borrow::Cow, io::BufReader, net::TcpStream, path::PathBuf};

use orfail::OrFail;

use crate::lsp::{self, DocumentUri};

pub fn try_run(mut args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("act").take(&mut args).is_present() {
        return Ok(Some(args));
    }

    let port: u16 = noargs::opt("port")
        .short('p')
        .default("9257")
        .env("LSPTERM_PORT")
        .take(&mut args)
        .then(|a| a.value().parse())?;
    let execute_index: Option<usize> = noargs::opt("execute")
        .short('e')
        .doc("Execute the code action at the specified index (1-based)")
        .take(&mut args)
        .present_and_then(|a| {
            a.value()
                .parse::<usize>()
                .map(|i| if i > 0 { i - 1 } else { 0 })
        })?;
    let file = noargs::arg("FILE")
        .example("/path/to/file")
        .take(&mut args)
        .then(|a| a.value().parse::<PathBuf>())?;
    let start_line = noargs::arg("START_LINE")
        .take(&mut args)
        .then(|a| a.value().parse::<u32>())?;
    let start_character = noargs::arg("START_CHARACTER")
        .take(&mut args)
        .then(|a| a.value().parse::<u32>())?;
    let end_line = noargs::arg("END_LINE")
        .take(&mut args)
        .then(|a| a.value().parse::<u32>())?;
    let end_character = noargs::arg("END_CHARACTER")
        .take(&mut args)
        .then(|a| a.value().parse::<u32>())?;

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(None);
    }

    let file = DocumentUri::new(file).or_fail()?;

    let mut stream = BufReader::new(TcpStream::connect(("127.0.0.1", port)).or_fail()?);

    // Send code action request
    let request_id = 1u32;
    let params = nojson::object(|f| {
        f.member("textDocument", nojson::object(|f| f.member("uri", &file)))?;
        f.member(
            "range",
            nojson::object(|f| {
                f.member(
                    "start",
                    nojson::object(|f| {
                        f.member("line", start_line)?;
                        f.member("character", start_character)
                    }),
                )?;
                f.member(
                    "end",
                    nojson::object(|f| {
                        f.member("line", end_line)?;
                        f.member("character", end_character)
                    }),
                )
            }),
        )?;
        f.member(
            "context",
            nojson::object(|f| f.member("diagnostics", nojson::array(|_| Ok(())))),
        )
    });

    lsp::send_request(
        stream.get_mut(),
        request_id,
        "textDocument/codeAction",
        params,
    )
    .or_fail()?;

    // Receive code action response
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

    if let Ok(actions) = result.to_array().map(|a| a.collect::<Vec<_>>()) {
        if actions.is_empty() {
            println!("No code actions available for the specified range.");
            return Ok(None);
        }

        // Display available code actions
        println!("Available code actions:");
        for (i, action) in actions.iter().enumerate() {
            if let Some(title) = action.to_member("title").or_fail()?.get() {
                println!(
                    "  {}: {}",
                    i + 1,
                    title.to_unquoted_string_str().unwrap_or_default()
                );

                if let Some(kind) = action.to_member("kind").or_fail()?.get() {
                    println!(
                        "     Kind: {}",
                        kind.to_unquoted_string_str().unwrap_or_default()
                    );
                }

                if let Some(disabled) = action.to_member("disabled").or_fail()?.get() {
                    if let Some(reason) = disabled.to_member("reason").or_fail()?.get() {
                        println!(
                            "     Disabled: {}",
                            reason.to_unquoted_string_str().unwrap_or_default()
                        );
                    }
                }
            }
            println!();
        }

        // Execute specific code action if requested
        if let Some(index) = execute_index {
            if index < actions.len() {
                let selected_action = &actions[index];

                // Check if the action is disabled
                if let Some(disabled) = selected_action.to_member("disabled").or_fail()?.get() {
                    if let Some(reason) = disabled.to_member("reason").or_fail()?.get() {
                        eprintln!(
                            "Cannot execute disabled code action: {}",
                            reason.to_unquoted_string_str().unwrap_or_default()
                        );
                        return Ok(None);
                    }
                }

                if let Err(e) = execute_code_action(&mut stream, request_id + 1, selected_action) {
                    eprintln!("Failed to execute code action: {e}");
                }
            } else {
                eprintln!(
                    "Invalid code action index: {}. Available actions: 1-{}",
                    index + 1,
                    actions.len()
                );
            }
        }
    } else {
        println!("{result}");
    }

    Ok(None)
}

fn resolve_code_action(
    stream: &mut BufReader<TcpStream>,
    request_id: u32,
    action: &nojson::RawJsonValue,
) -> Result<nojson::RawJsonOwned, Box<dyn std::error::Error>> {
    lsp::send_request(
        stream.get_mut(),
        request_id,
        "codeAction/resolve",
        *action,
    )
    .map_err(|e| format!("Failed to send resolve request: {e}"))?;

    let response_json = lsp::recv_message(stream)
        .map_err(|e| format!("Failed to receive resolve response: {e}"))?
        .ok_or("unexpected EOS")?;
    let response_value = response_json.value();

    if let Some(error) = response_value
        .to_member("error")
        .map_err(|e| format!("Invalid response format: {e}"))?
        .get()
    {
        return Err(format!("Failed to resolve code action: {error}").into());
    }

    let result = response_value
        .to_member("result")
        .map_err(|e| format!("Invalid response format: {e}"))?
        .required()
        .map_err(|e| format!("Missing result in response: {e}"))?;

    Ok(result.extract().into_owned())
}

fn apply_workspace_edit(edit: &nojson::RawJsonValue) -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::HashMap;
    use std::fs;

    // Parse documentChanges array
    let document_changes = edit
        .to_member("documentChanges")
        .map_err(|e| format!("Invalid edit format: {e}"))?
        .required()
        .map_err(|e| format!("Missing documentChanges: {e}"))?
        .to_array()
        .map_err(|e| format!("Invalid documentChanges format: {e}"))?;

    // Group edits by file
    let mut files_to_edit: HashMap<String, Vec<_>> = HashMap::new();

    for change in document_changes {
        let text_document = change
            .to_member("textDocument")
            .map_err(|e| format!("Invalid change format: {e}"))?
            .required()
            .map_err(|e| format!("Missing textDocument: {e}"))?;

        let uri = text_document
            .to_member("uri")
            .map_err(|e| format!("Invalid textDocument format: {e}"))?
            .required()
            .map_err(|e| format!("Missing uri: {e}"))?
            .to_unquoted_string_str()
            .map_err(|e| format!("Invalid uri format: {e}"))?;

        let edits = change
            .to_member("edits")
            .map_err(|e| format!("Invalid change format: {e}"))?
            .required()
            .map_err(|e| format!("Missing edits: {e}"))?
            .to_array()
            .map_err(|e| format!("Invalid edits format: {e}"))?;

        for edit in edits {
            let range = edit
                .to_member("range")
                .map_err(|e| format!("Invalid edit format: {e}"))?
                .required()
                .map_err(|e| format!("Missing range: {e}"))?;

            let start = range
                .to_member("start")
                .map_err(|e| format!("Invalid range format: {e}"))?
                .required()
                .map_err(|e| format!("Missing start: {e}"))?;
            let start_line = usize::try_from(
                start
                    .to_member("line")
                    .map_err(|e| format!("Invalid start format: {e}"))?
                    .required()
                    .map_err(|e| format!("Missing line: {e}"))?,
            )
            .map_err(|e| format!("Invalid line number: {e}"))?;
            let start_char = usize::try_from(
                start
                    .to_member("character")
                    .map_err(|e| format!("Invalid start format: {e}"))?
                    .required()
                    .map_err(|e| format!("Missing character: {e}"))?,
            )
            .map_err(|e| format!("Invalid character number: {e}"))?;

            let end = range
                .to_member("end")
                .map_err(|e| format!("Invalid range format: {e}"))?
                .required()
                .map_err(|e| format!("Missing end: {e}"))?;
            let end_line = usize::try_from(
                end.to_member("line")
                    .map_err(|e| format!("Invalid end format: {e}"))?
                    .required()
                    .map_err(|e| format!("Missing line: {e}"))?,
            )
            .map_err(|e| format!("Invalid line number: {e}"))?;
            let end_char = usize::try_from(
                end.to_member("character")
                    .map_err(|e| format!("Invalid end format: {e}"))?
                    .required()
                    .map_err(|e| format!("Missing character: {e}"))?,
            )
            .map_err(|e| format!("Invalid character number: {e}"))?;

            let new_text = edit
                .to_member("newText")
                .map_err(|e| format!("Invalid edit format: {e}"))?
                .required()
                .map_err(|e| format!("Missing newText: {e}"))?
                .to_unquoted_string_str()
                .map_err(|e| format!("Invalid newText format: {e}"))?;

            files_to_edit.entry(uri.to_string()).or_default().push((
                start_line,
                start_char,
                end_line,
                end_char,
                new_text.to_string(),
            ));
        }
    }

    // Apply edits to each file (reuse logic from subcommand_rename.rs)
    for (uri, mut edits) in files_to_edit {
        // Convert file:// URI to path
        let file_path = if let Some(path) = uri.strip_prefix("file://") {
            std::path::PathBuf::from(path)
        } else {
            return Err(format!("Invalid URI format: {uri}").into());
        };

        // Read file content
        let content = fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read file '{}': {}", file_path.display(), e))?;

        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        // Sort edits by position (end to start) to avoid offset issues
        edits.sort_by(|a, b| match b.0.cmp(&a.0) {
            std::cmp::Ordering::Equal => b.1.cmp(&a.1),
            other => other,
        });

        // Apply edits (reuse the logic from subcommand_rename.rs)
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
                // Multi-line edit
                if start_line < lines.len() && end_line < lines.len() {
                    let start_line_content = &lines[start_line];
                    let end_line_content = &lines[end_line];

                    let start_chars: Vec<char> = start_line_content.chars().collect();
                    let end_chars: Vec<char> = end_line_content.chars().collect();

                    if start_char <= start_chars.len() && end_char <= end_chars.len() {
                        let mut new_line = String::new();
                        new_line.push_str(&start_chars[..start_char].iter().collect::<String>());
                        new_line.push_str(&new_text);
                        new_line.push_str(&end_chars[end_char..].iter().collect::<String>());

                        lines.splice(start_line..=end_line, std::iter::once(new_line));
                    }
                }
            }
        }

        // Write back to file
        let new_content = lines.join("\n");
        let final_content = if content.ends_with('\n') && !new_content.ends_with('\n') {
            new_content + "\n"
        } else {
            new_content
        };

        fs::write(&file_path, final_content)
            .map_err(|e| format!("Failed to write file '{}': {}", file_path.display(), e))?;

        println!("Applied changes to: {}", file_path.display());
    }

    println!("Workspace edit applied successfully");
    Ok(())
}

fn execute_command(
    stream: &mut BufReader<TcpStream>,
    request_id: u32,
    command: &nojson::RawJsonValue,
) -> Result<(), Box<dyn std::error::Error>> {
    lsp::send_request(
        stream.get_mut(),
        request_id,
        "workspace/executeCommand",
        *command,
    )
    .map_err(|e| format!("Failed to send execute command request: {e}"))?;

    let response_json = lsp::recv_message(stream)
        .map_err(|e| format!("Failed to receive execute command response: {e}"))?
        .ok_or("unexpected EOS")?;
    let response_value = response_json.value();

    if let Some(error) = response_value
        .to_member("error")
        .map_err(|e| format!("Invalid response format: {e}"))?
        .get()
    {
        return Err(format!("Failed to execute command: {error}").into());
    }

    println!("Command executed successfully");
    Ok(())
}

fn execute_code_action(
    stream: &mut BufReader<TcpStream>,
    mut request_id: u32,
    action: &nojson::RawJsonValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let title = action
        .to_member("title")
        .map_err(|e| format!("Invalid action format: {e}"))?
        .get()
        .and_then(|t| t.to_unquoted_string_str().ok())
        .unwrap_or(Cow::Borrowed("Unknown"));

    println!("Executing code action: {title}");

    // Check if the action needs to be resolved first
    let resolved_action = if action
        .to_member("data")
        .map_err(|e| format!("Invalid action format: {e}"))?
        .get()
        .is_some()
    {
        println!("Resolving code action...");
        request_id += 1;
        resolve_code_action(stream, request_id, action)?
    } else {
        action.extract().into_owned()
    };

    let resolved_value = resolved_action.value();

    // Execute the edit if present
    if let Some(edit) = resolved_value
        .to_member("edit")
        .map_err(|e| format!("Invalid resolved action format: {e}"))?
        .get()
    {
        request_id += 1;
        apply_workspace_edit(&edit)?;
    }

    // Execute the command if present
    if let Some(command) = resolved_value
        .to_member("command")
        .map_err(|e| format!("Invalid resolved action format: {e}"))?
        .get()
    {
        request_id += 1;
        execute_command(stream, request_id, &command)?;
    }

    Ok(())
}
