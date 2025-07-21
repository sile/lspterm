use std::path::PathBuf;

#[derive(Debug)]
pub struct LspClient {}

impl LspClient {
    pub fn new(lsp_server_command: PathBuf, lsp_server_args: Vec<String>) -> orfail::Result<Self> {
        Ok(Self {})
    }
}
