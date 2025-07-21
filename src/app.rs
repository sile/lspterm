use orfail::OrFail;
use tuinix::Terminal;

use crate::lsp_client::LspClient;

#[derive(Debug)]
pub struct App {
    terminal: Terminal,
    lsp_client: LspClient,
}

impl App {
    pub fn new(lsp_client: LspClient) -> orfail::Result<Self> {
        let terminal = Terminal::new().or_fail()?;
        Ok(Self {
            terminal,
            lsp_client,
        })
    }

    pub fn run(self) -> orfail::Result<()> {
        Ok(())
    }
}
