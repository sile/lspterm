use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
};

use orfail::OrFail;

use crate::{
    json::{JsonRpcResponse, JsonValue, json_object},
    lsp,
};

#[derive(Debug, Default, Clone)]
struct Args {
    port: u16,
    lsp_server_command: PathBuf,
    lsp_server_args: Vec<PathBuf>,
}

pub fn try_run(mut raw_args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("serve").take(&mut raw_args).is_present() {
        return Ok(Some(raw_args));
    }

    // TODO: Add '--' handling

    let mut args = Args::default();
    args.port = noargs::opt("port")
        .short('p')
        .default("9257")
        .take(&mut raw_args)
        .then(|a| a.value().parse())?;
    args.lsp_server_command = noargs::arg("LSP_SERVER_COMMAND")
        .example("/path/to/lsp-server")
        .take(&mut raw_args)
        .then(|a| a.value().parse())?;
    while let Some(a) = noargs::arg("[LSP_SERVER_ARG]...")
        .take(&mut raw_args)
        .present()
    {
        args.lsp_server_args.push(a.value().parse()?);
    }

    if let Some(help) = raw_args.finish()? {
        print!("{help}");
        return Ok(None);
    }

    let mut lsp_server = spawn_lsp_server(&args).or_fail()?;
    let mut lsp_server_stdin = lsp_server.stdin.take().or_fail()?;
    let mut lsp_server_stdout = BufReader::new(lsp_server.stdout.take().or_fail()?);
    initialize_lsp_server(&mut lsp_server_stdout, &mut lsp_server_stdin).or_fail()?;

    Ok(None)
}

fn spawn_lsp_server(args: &Args) -> orfail::Result<Child> {
    let mut command = Command::new(&args.lsp_server_command);
    command
        .args(&args.lsp_server_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    command.spawn().or_fail_with(|e| {
        format!(
            "failed to spawn LSP server process '{}': {e}",
            args.lsp_server_command.display()
        )
    })
}

fn initialize_lsp_server<R, W>(mut reader: R, mut writer: W) -> orfail::Result<()>
where
    R: BufRead,
    W: Write,
{
    let workspace_folder_path = std::env::current_dir().or_fail()?;
    let client_info = |f: &mut nojson::JsonObjectFormatter<'_, '_, '_>| {
        f.member("name", env!("CARGO_PKG_NAME"))?;
        f.member("version", env!("CARGO_PKG_VERSION"))
    };
    let workspace_folder = |f: &mut nojson::JsonObjectFormatter<'_, '_, '_>| {
        f.member("uri", format!("file://{}", workspace_folder_path.display()))?;
        f.member("name", "main")
    };
    let capabilities = |f: &mut nojson::JsonObjectFormatter<'_, '_, '_>| {
        f.member(
            "textDocument",
            json_object(|f| f.member("definition", json_object(|f| f.member("linkSupport", true)))),
        )?;
        f.member(
            "window",
            json_object(|f| f.member("workDoneProgress", true)),
        )?;
        f.member(
            "general",
            json_object(|f| f.member("positionEncodings", ["utf-8"])),
        )
    };
    let initialize_params = json_object(|f| {
        f.member("clientInfo", json_object(client_info))?;
        f.member("workspaceFolders", [json_object(workspace_folder)])?;
        f.member("capabilities", json_object(capabilities))?;
        Ok(())
    });
    let json = lsp::send_request(&mut writer, 0, "initialize", initialize_params).or_fail()?;
    println!("{json}");

    // TODO:
    let json = lsp::send_notification(&mut writer, "initialized", ()).or_fail()?;
    println!("{json}");

    Ok(())
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
