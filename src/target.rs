use std::num::NonZeroUsize;

use crate::lsp::DocumentUri;

#[derive(Debug, Clone)]
pub struct TargetLocation {
    pub file: DocumentUri,
    pub line: NonZeroUsize,
    pub character: NonZeroUsize,
}

impl TargetLocation {
    pub fn fmt_json_object(
        &self,
        f: &mut nojson::JsonObjectFormatter<'_, '_, '_>,
    ) -> std::fmt::Result {
        f.member(
            "textDocument",
            nojson::object(|f| f.member("uri", &self.file)),
        )?;
        f.member(
            "position",
            nojson::object(|f| {
                f.member("line", self.line.get() - 1)?;
                f.member("character", self.character.get() - 1)
            }),
        )?;
        Ok(())
    }
}

impl std::str::FromStr for TargetLocation {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut tokens = s.splitn(3, ':');

        let file = tokens.next().expect("infallible");
        let file = DocumentUri::new(file)
            .map_err(|e| format!("invalid file path '{file}': {}", e.message))?;

        let line_str = tokens.next().unwrap_or("1");
        let line = line_str
            .parse::<NonZeroUsize>()
            .map_err(|_| format!("invalid line number '{line_str}': must be a positive integer"))?;

        let character_str = tokens.next().unwrap_or("1");
        let character = character_str.parse::<NonZeroUsize>().map_err(|_| {
            format!("invalid column number '{character_str}': must be a positive integer")
        })?;

        Ok(Self {
            file,
            line,
            character,
        })
    }
}
