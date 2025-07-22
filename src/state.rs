#[derive(Debug)]
pub struct State {
    console_log: Vec<ConsoleLogEntry>,
}

impl State {
    pub fn new() -> Self {
        Self {
            console_log: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub enum ConsoleLogEntry {
    LspServerStderr(String),
}
