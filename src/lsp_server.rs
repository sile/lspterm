use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

use nojson::RawJsonOwned;
use orfail::OrFail;

use crate::json::JsonObject;

#[derive(Debug)]
pub struct LspServerSpec {
    pub command: PathBuf,
    pub args: Vec<String>,
    pub initialize_options: Option<RawJsonOwned>,
}

impl LspServerSpec {
    pub fn load(path: &Path) -> orfail::Result<Self> {
        crate::json::parse_file(path, |value| {
            let object = JsonObject::new(value)?;
            Ok(Self {
                command: object.convert_required("command")?,
                args: object.convert_optional_or_default("args")?,
                initialize_options: object
                    .get_optional("initialize_options")?
                    .map(|v| v.extract().into_owned()),
            })
        })
        .or_fail()
    }

    pub fn spawn_process(&self) -> orfail::Result<Child> {
        Command::new(&self.command)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .or_fail_with(|e| {
                format!(
                    "failed to spawn LSP server process '{}': {e}",
                    self.command.display()
                )
            })
    }
}
