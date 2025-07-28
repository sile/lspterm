use crate::lsp::DocumentUri;

#[derive(Debug, Clone)]
pub struct TargetLocation {
    pub file: DocumentUri,
    pub line: usize,
    pub character: usize,
}

impl std::str::FromStr for TargetLocation {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut tokens = s.splitn(3, ':');

        let file = tokens.next().expect("infallible");
        let file = DocumentUri::new(file)
            .map_err(|e| format!("invalid file path '{file}': {}", e.message))?;

        let line_str = tokens.next().unwrap_or("0");
        let line = line_str.parse::<usize>().map_err(|_| {
            format!("invalid line number '{line_str}': must be a non-negative integer")
        })?;

        let character_str = tokens.next().unwrap_or("0");
        let character = character_str.parse::<usize>().map_err(|_| {
            format!("invalid column number '{character_str}': must be a non-negative integer")
        })?;

        Ok(Self {
            file,
            line,
            character,
        })
    }
}
