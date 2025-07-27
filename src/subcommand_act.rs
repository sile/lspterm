use std::{io::BufReader, net::TcpStream, path::PathBuf};

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
    // let context_only: Option<String> = noargs::opt("only")
    //     .doc("Filter code actions by kind (e.g., 'quickfix', 'refactor')")
    //     .take(&mut args)
    //     .present_and_then(|a| a.value().parse())?;
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
    let request_id = 1;
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
                // TODO: if let Some(only_value) = &context_only {
                //f.member("only", ["quickfix", "refactor", "source"])?;
                //}
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

    // Pretty print the code actions
    if let Ok(actions) = result.to_array().map(|a| a.collect::<Vec<_>>()) {
        if actions.is_empty() {
            println!("No code actions available for the specified range.");
        } else {
            println!("Available code actions:");
            for (i, action) in actions.into_iter().enumerate() {
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
                    println!();
                }
            }
        }
    } else {
        println!("{}", result);
    }

    Ok(None)
}
