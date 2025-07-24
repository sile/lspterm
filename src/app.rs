use orfail::OrFail;
use tuinix::{KeyCode, Terminal, TerminalEvent, TerminalInput};

use crate::{
    lsp_client::LspClient, lsp_messages::InitializeRequest, mame::TerminalFrame, state::State,
};

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
    next_request_id: u64,
}

impl App {
    pub fn new(lsp_client: LspClient) -> orfail::Result<Self> {
        let terminal = Terminal::new().or_fail()?;
        Ok(Self {
            terminal,
            state: State::new(),
            lsp_client,
            active_tab: Tab::Main, // Default tab
            next_request_id: 0,
        })
    }

    fn next_request_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        id
    }

    pub fn run(mut self) -> orfail::Result<()> {
        // Draw initial frame
        self.render().or_fail()?;

        let req = InitializeRequest {
            id: self.next_request_id(),
            workspace_folder: std::env::current_dir().or_fail()?,
        };
        self.lsp_client.send(req).or_fail()?;

        // Event loop
        loop {
            let readfds = [self.lsp_client.stdout_fd(), self.lsp_client.stderr_fd()]
                .into_iter()
                .filter_map(|v| v)
                .collect::<Vec<_>>();
            match self.terminal.poll_event(&readfds, &[], None).or_fail()? {
                Some(TerminalEvent::Input(input)) => {
                    let TerminalInput::Key(key_input) = input else {
                        unreachable!("mouse input has not been enabled");
                    };

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
                Some(TerminalEvent::FdReady { fd, .. }) => {
                    if self.lsp_client.stderr_fd() == Some(fd)
                        && let Some(line) = self.lsp_client.read_stderr_line().or_fail()?
                    {
                        self.state.console_log.push(line);
                    } else if self.lsp_client.stdout_fd() == Some(fd) {
                        let response_json = self.lsp_client.recv_response_json().or_fail()?;
                        if !response_json.is_empty() {
                            return Err(orfail::Failure::new(response_json));
                        }
                    }
                }
                None => {}
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
        for line in self.state.console_log.iter().rev().take(10) {
            let line = line.trim();
            writeln!(frame, "LSP-SERVER-STDERR> {line}").or_fail()?;
        }

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
