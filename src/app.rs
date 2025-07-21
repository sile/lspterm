#[derive(Debug)]
pub struct App {}

impl App {
    pub fn new() -> orfail::Result<Self> {
        Ok(Self {})
    }

    pub fn run(self) -> orfail::Result<()> {
        Ok(())
    }
}
