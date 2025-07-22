use orfail::OrFail;
use tuinix::{KeyCode, Terminal, TerminalEvent, TerminalInput};

use crate::{lsp_client::LspClient, mame::TerminalFrame, state::State};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    Main,
    Console,
    Trace,
}

impl Tab {
    fn name(&self) -> &'static str {
        match self {
            Tab::Main => "Main",
            Tab::Console => "Console",
            Tab::Trace => "Trace",
        }
    }

    fn all() -> [Tab; 3] {
        [Tab::Main, Tab::Console, Tab::Trace]
    }
}

#[derive(Debug)]
pub struct App {
    terminal: Terminal,
    state: State,
    lsp_client: LspClient,
    active_tab: Tab,
}

impl App {
    pub fn new(lsp_client: LspClient) -> orfail::Result<Self> {
        let terminal = Terminal::new().or_fail()?;
        Ok(Self {
            terminal,
            state: State::new(),
            lsp_client,
            active_tab: Tab::Main, // Default tab
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

                    match key_input.code {
                        // Handle quit command
                        KeyCode::Char('q') => break,

                        // Tab navigation
                        KeyCode::Char('1') => self.active_tab = Tab::Main,
                        KeyCode::Char('2') => self.active_tab = Tab::Console,
                        KeyCode::Char('3') => self.active_tab = Tab::Trace,

                        // Tab switching with Tab key
                        KeyCode::Tab => {
                            self.active_tab = match self.active_tab {
                                Tab::Main => Tab::Console,
                                Tab::Console => Tab::Trace,
                                Tab::Trace => Tab::Main,
                            };
                        }

                        _ => {
                            // TODO: Add more input handling here based on active tab
                        }
                    }

                    // Redraw the UI after input
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

        // Render tabs
        self.render_tabs(&mut frame).or_fail()?;

        // Add separator line
        writeln!(frame, "{}", "─".repeat(size.cols as usize)).or_fail()?;

        // Render content based on active tab
        match self.active_tab {
            Tab::Main => self.render_main_tab(&mut frame).or_fail()?,
            Tab::Console => self.render_console_tab(&mut frame).or_fail()?,
            Tab::Trace => self.render_trace_tab(&mut frame).or_fail()?,
        }

        // Status line
        writeln!(frame, "{}", "─".repeat(size.cols as usize)).or_fail()?;
        writeln!(
            frame,
            "Press 'q' to quit | 1/2/3 or Tab to switch tabs | Size: {}x{}",
            size.cols, size.rows
        )
        .or_fail()?;

        self.terminal.draw(frame).or_fail()?;
        Ok(())
    }

    fn render_tabs(&self, frame: &mut TerminalFrame) -> orfail::Result<()> {
        use std::fmt::Write;

        let mut tab_line = String::new();

        for (i, tab) in Tab::all().iter().enumerate() {
            if i > 0 {
                tab_line.push(' ');
            }

            if *tab == self.active_tab {
                write!(tab_line, "[{}]", tab.name()).or_fail()?;
            } else {
                write!(tab_line, " {} ", tab.name()).or_fail()?;
            }
        }

        writeln!(frame, "{}", tab_line).or_fail()?;
        Ok(())
    }

    fn render_main_tab(&self, frame: &mut TerminalFrame) -> orfail::Result<()> {
        use std::fmt::Write;

        writeln!(frame, "LSP Editor - Main View").or_fail()?;
        writeln!(frame, "").or_fail()?;
        writeln!(frame, "This is the main editor view.").or_fail()?;
        writeln!(frame, "TODO: Implement file editor functionality").or_fail()?;

        Ok(())
    }

    fn render_console_tab(&self, frame: &mut TerminalFrame) -> orfail::Result<()> {
        use std::fmt::Write;

        writeln!(frame, "Console View").or_fail()?;
        writeln!(frame, "").or_fail()?;
        writeln!(frame, "LSP Console output will appear here.").or_fail()?;
        writeln!(frame, "TODO: Display LSP server communication").or_fail()?;

        Ok(())
    }

    fn render_trace_tab(&self, frame: &mut TerminalFrame) -> orfail::Result<()> {
        use std::fmt::Write;

        writeln!(frame, "Trace View").or_fail()?;
        writeln!(frame, "").or_fail()?;
        writeln!(frame, "LSP trace information will appear here.").or_fail()?;
        writeln!(frame, "TODO: Display detailed LSP protocol traces").or_fail()?;

        Ok(())
    }
}
