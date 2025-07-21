use orfail::OrFail;
use tuinix::{KeyCode, Terminal, TerminalEvent, TerminalInput};

use crate::{lsp_client::LspClient, mame::TerminalFrame};

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

    pub fn run(mut self) -> orfail::Result<()> {
        // Draw initial frame
        self.render().or_fail()?;

        // Event loop
        loop {
            match self.terminal.poll_event(None).or_fail()? {
                Some(TerminalEvent::Input(input)) => {
                    let TerminalInput::Key(key_input) = input;

                    // Handle quit command
                    if let KeyCode::Char('q') = key_input.code {
                        break;
                    }

                    // TODO: Add more input handling here
                    // For now, just redraw the UI on any key press
                    self.render().or_fail()?;
                }
                Some(TerminalEvent::Resize(_size)) => {
                    // Terminal was resized, redraw UI
                    self.render().or_fail()?;
                }
                None => {
                    // Timeout elapsed, no events to process
                    // TODO: Add periodic updates here if needed
                }
            }
        }

        Ok(())
    }

    fn render(&mut self) -> orfail::Result<()> {
        use std::fmt::Write;

        let size = self.terminal.size();
        let mut frame = TerminalFrame::new(size);

        // Simple UI for now
        writeln!(frame, "LSP Editor").or_fail()?;
        writeln!(frame, "Press 'q' to quit").or_fail()?;
        writeln!(frame, "Terminal size: {}x{}", size.cols, size.rows).or_fail()?;

        self.terminal.draw(frame).or_fail()?;
        Ok(())
    }
}
