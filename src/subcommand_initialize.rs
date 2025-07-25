use std::path::PathBuf;

use orfail::OrFail;

use crate::{
    json::{JsonRpcRequest, JsonRpcResponse, JsonValue, json_object},
    lsp_client::{LspClient, LspClientOptions},
};

pub fn try_run(mut args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("initialize").take(&mut args).is_present() {
        return Ok(Some(args));
    }

    let options = LspClientOptions::parse_args(&mut args)?;

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(None);
    }

    let mut lsp_client = LspClient::new(options).or_fail()?;
    let res = initialize(&mut lsp_client).or_fail()?;
    println!("{}", nojson::Json(res.value));

    Ok(None)
}

pub fn initialize(client: &mut LspClient) -> orfail::Result<InitializeResponse> {
    let req = InitializeRequest::new().or_fail()?;
    let res = client.call(req).or_fail()?;
    client.cast("initialized").or_fail()?;
    Ok(res)
}

#[derive(Debug)]
pub struct InitializeRequest {
    pub workspace_folder: PathBuf,
}

impl InitializeRequest {
    pub fn new() -> orfail::Result<Self> {
        Ok(Self {
            workspace_folder: std::env::current_dir().or_fail()?,
        })
    }
}

impl JsonRpcRequest for InitializeRequest {
    type Response = InitializeResponse;

    fn method(&self) -> &str {
        "initialize"
    }

    fn params(&self, f: &mut nojson::JsonObjectFormatter<'_, '_, '_>) -> std::fmt::Result {
        f.member(
            "clientInfo",
            json_object(|f| {
                f.member("name", env!("CARGO_PKG_NAME"))?;
                f.member("version", env!("CARGO_PKG_VERSION"))
            }),
        )?;
        f.member(
            "workspaceFolders",
            [json_object(|f| {
                f.member("uri", format!("file://{}", self.workspace_folder.display()))?;
                f.member("name", "main")
            })],
        )?;
        f.member(
            "capabilities",
            json_object(|f| {
                f.member(
                    "textDocument",
                    json_object(|f| {
                        f.member("definition", json_object(|f| f.member("linkSupport", true)))
                    }),
                )?;
                f.member(
                    "general",
                    json_object(|f| f.member("positionEncodings", ["utf-8"])),
                )
            }),
        )?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct InitializeResponse {
    value: JsonValue,
}

impl JsonRpcResponse for InitializeResponse {
    fn from_result_value(
        value: nojson::RawJsonValue<'_, '_>,
    ) -> Result<Self, nojson::JsonParseError> {
        Ok(Self {
            value: value.try_into()?,
        })
    }
}
