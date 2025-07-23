#[derive(Debug)]
pub struct State {
    pub console_log: Vec<String>,
}

impl State {
    pub fn new() -> Self {
        Self {
            console_log: Vec::new(),
        }
    }
}
