use std::{io::BufReader, net::TcpStream, path::PathBuf};

use orfail::OrFail;

use crate::lsp::{self, DocumentUri};

pub fn try_run(mut args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("completion").take(&mut args).is_present() {
        return Ok(Some(args));
    }

    let port: u16 = noargs::opt("port")
        .short('p')
        .default("9257")
        .env("LSPTERM_PORT")
        .take(&mut args)
        .then(|a| a.value().parse())?;
    let file = noargs::arg("FILE")
        .take(&mut args)
        .then(|a| a.value().parse::<PathBuf>())?;
    let line = noargs::arg("LINE")
        .take(&mut args)
        .then(|a| a.value().parse::<u32>())?;
    let character = noargs::arg("CHARACTER")
        .take(&mut args)
        .then(|a| a.value().parse::<u32>())?;

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(None);
    }

    let file = DocumentUri::new(file).or_fail()?;

    let mut stream = BufReader::new(TcpStream::connect(("127.0.0.1", port)).or_fail()?);

    // Send completion request
    let request_id = 1;
    let params = nojson::object(|f| {
        f.member("textDocument", nojson::object(|f| f.member("uri", &file)))?;
        f.member(
            "position",
            nojson::object(|f| {
                f.member("line", line)?;
                f.member("character", character)
            }),
        )
    });

    lsp::send_request(
        stream.get_mut(),
        request_id,
        "textDocument/completion",
        params,
    )
    .or_fail()?;

    // Receive completion response
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

    Ok(None)
}
