use crate::lsp_client::LspClient;

#[derive(Debug)]
pub struct App {
    lsp_client: LspClient,
}

impl App {
    pub fn new(lsp_client: LspClient) -> orfail::Result<Self> {
        Ok(Self { lsp_client })
    }

    pub fn run(self) -> orfail::Result<()> {
        Ok(())
    }
}
