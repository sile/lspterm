use std::path::PathBuf;

use orfail::OrFail;

use crate::{
    json::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, JsonValue, json_object},
    lsp_client::{LspClient, LspClientOptions},
    subcommand_initialize::initialize,
};

pub fn try_run(mut args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("find-def").take(&mut args).is_present() {
        return Ok(Some(args));
    }

    let options = LspClientOptions::parse_args(&mut args)?;
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

    let mut lsp_client = LspClient::new(options).or_fail()?;
    initialize(&mut lsp_client).or_fail()?;
    // TODO: capability check

    let file = file.canonicalize().or_fail()?;
    let did_open = DidOpenNotification::new(&file).or_fail()?;
    lsp_client.cast(did_open).or_fail()?;

    for i in 0..10 {
        let req = DefinitionRequest::new(file.clone(), line, character).or_fail()?;
        let Ok(res) = lsp_client.call(req).or_fail() else {
            continue; // TODO
        };
        println!("[{i}] {}", nojson::Json(res.value));
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    Ok(None)
}

#[derive(Debug)]
pub struct DidOpenNotification {
    file: PathBuf,
    content: String,
}

impl DidOpenNotification {
    pub fn new(file: &PathBuf) -> orfail::Result<Self> {
        let content = std::fs::read_to_string(file)
            .or_fail_with(|e| format!("failed to read file '{}': {e}", file.display()))?;
        Ok(Self {
            file: file.clone(),
            content,
        })
    }
}

impl JsonRpcNotification for DidOpenNotification {
    fn method(&self) -> &str {
        "textDocument/didOpen"
    }

    fn params(&self, f: &mut nojson::JsonObjectFormatter<'_, '_, '_>) -> std::fmt::Result {
        f.member(
            "textDocument",
            json_object(|f| {
                f.member("uri", format!("file://{}", self.file.display()))?;
                f.member("languageId", "rust")?; // TODO
                f.member("version", 1)?;
                f.member("text", &self.content)
            }),
        )?;
        Ok(())
    }
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
            json_object(|f| f.member("uri", format!("file://{}", self.file.display()))),
        )?;
        f.member(
            "position",
            json_object(|f| {
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
