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
            nojson::object(|f| {
                f.member("diagnostics", nojson::array(|_| Ok(())))?;
                // Support filtering code actions by kind
                // You could add this as a command-line option if needed:
                // f.member("only", nojson::array(|f| {
                //     f.element("quickfix")?;
                //     f.element("refactor")?;
                //     f.element("source")
                // }))?;
                Ok(())
            }),
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
    let response_json = lsp::recv_message(&mut stream).or_fail()?;
    let response_value = response_json.value();

    // Check if there's an error in the response
    if let Some(error) = response_value.to_member("error").or_fail()?.get() {
        eprintln!("LSP server returned error: {}", error);
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
                    eprintln!("Failed to execute code action: {}", e);
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
        println!("{}", result);
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
        action.clone(),
    )
    .map_err(|e| format!("Failed to send resolve request: {}", e))?;

    let response_json = lsp::recv_message(stream)
        .map_err(|e| format!("Failed to receive resolve response: {}", e))?;
    let response_value = response_json.value();

    if let Some(error) = response_value
        .to_member("error")
        .map_err(|e| format!("Invalid response format: {}", e))?
        .get()
    {
        return Err(format!("Failed to resolve code action: {}", error).into());
    }

    let result = response_value
        .to_member("result")
        .map_err(|e| format!("Invalid response format: {}", e))?
        .required()
        .map_err(|e| format!("Missing result in response: {}", e))?;

    Ok(result.extract().into_owned())
}

fn apply_workspace_edit(
    stream: &mut BufReader<TcpStream>,
    request_id: u32,
    edit: &nojson::RawJsonValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let params = nojson::object(|f| f.member("edit", edit.clone()));

    lsp::send_request(stream.get_mut(), request_id, "workspace/applyEdit", params)
        .map_err(|e| format!("Failed to send apply edit request: {}", e))?;

    let response_json = lsp::recv_message(stream)
        .map_err(|e| format!("Failed to receive apply edit response: {}", e))?;
    let response_value = response_json.value();

    if let Some(error) = response_value
        .to_member("error")
        .map_err(|e| format!("Invalid response format: {}", e))?
        .get()
    {
        return Err(format!("Failed to apply workspace edit: {}", error).into());
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
        command.clone(),
    )
    .map_err(|e| format!("Failed to send execute command request: {}", e))?;

    let response_json = lsp::recv_message(stream)
        .map_err(|e| format!("Failed to receive execute command response: {}", e))?;
    let response_value = response_json.value();

    if let Some(error) = response_value
        .to_member("error")
        .map_err(|e| format!("Invalid response format: {}", e))?
        .get()
    {
        return Err(format!("Failed to execute command: {}", error).into());
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
        .map_err(|e| format!("Invalid action format: {}", e))?
        .get()
        .and_then(|t| t.to_unquoted_string_str().ok())
        .unwrap_or(Cow::Borrowed("Unknown"));

    println!("Executing code action: {}", title);

    // Check if the action needs to be resolved first
    let resolved_action = if action
        .to_member("data")
        .map_err(|e| format!("Invalid action format: {}", e))?
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
        .map_err(|e| format!("Invalid resolved action format: {}", e))?
        .get()
    {
        request_id += 1;
        apply_workspace_edit(stream, request_id, &edit)?;
    }

    // Execute the command if present
    if let Some(command) = resolved_value
        .to_member("command")
        .map_err(|e| format!("Invalid resolved action format: {}", e))?
        .get()
    {
        request_id += 1;
        execute_command(stream, request_id, &command)?;
    }

    Ok(())
}
