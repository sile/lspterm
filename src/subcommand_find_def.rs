use std::{net::TcpStream, path::PathBuf};

use orfail::OrFail;

use crate::{
    json::{JsonRpcRequest, JsonRpcResponse, JsonValue},
    lsp::{self, DocumentUri},
};

pub fn try_run(mut args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("find-def").take(&mut args).is_present() {
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
    let content = file.read_to_string().or_fail()?;

    let mut stream = TcpStream::connect(("127.0.0.1", port)).or_fail()?;

    let params = nojson::object(|f| {
        f.member(
            "textDocument",
            nojson::object(|f| {
                f.member("uri", &file)?;
                f.member("languageId", "rust")?; // TODO
                f.member("version", 1)?;
                f.member("text", &content)
            }),
        )
    });
    lsp::send_notification(&mut stream, "textDocument/didOpen", params).or_fail()?;

    Ok(None)
}

#[derive(Debug)]
pub struct DefinitionRequest {
    pub file: PathBuf,
    pub line: u32,
    pub character: u32,
}

impl DefinitionRequest {
    pub fn new(file: PathBuf, line: u32, character: u32) -> orfail::Result<Self> {
        Ok(Self {
            file,
            line,
            character,
        })
    }
}

impl JsonRpcRequest for DefinitionRequest {
    type Response = DefinitionResponse;

    fn method(&self) -> &str {
        "textDocument/definition"
    }

    fn params(&self, f: &mut nojson::JsonObjectFormatter<'_, '_, '_>) -> std::fmt::Result {
        f.member(
            "textDocument",
            nojson::object(|f| f.member("uri", format!("file://{}", self.file.display()))),
        )?;
        f.member(
            "position",
            nojson::object(|f| {
                f.member("line", self.line)?;
                f.member("character", self.character)
            }),
        )?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct DefinitionResponse {
    value: JsonValue,
}

impl JsonRpcResponse for DefinitionResponse {
    fn from_result_value(
        value: nojson::RawJsonValue<'_, '_>,
    ) -> Result<Self, nojson::JsonParseError> {
        Ok(Self {
            value: value.try_into()?,
        })
    }
}
