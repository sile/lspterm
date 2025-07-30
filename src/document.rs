use std::collections::HashMap;

use orfail::OrFail;

use crate::{
    json::JsonObject,
    lsp::{DocumentUri, PositionRange},
};

#[derive(Debug, Clone)]
pub struct DocumentChanges {
    pub changes: Vec<DocumentChange>,
}

impl nojson::DisplayJson for DocumentChanges {
    fn fmt(&self, f: &mut nojson::JsonFormatter<'_, '_>) -> std::fmt::Result {
        f.object(|f| f.member("documentChanges", &self.changes))
    }
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for DocumentChanges {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        let object = JsonObject::new(value)?;
        let mut changes = Vec::new();
        for item in object.get_required("documentChanges")?.to_array()? {
            if let Ok(text_change) = TextDocumentChange::try_from(item) {
                changes.push(DocumentChange::TextDocument(text_change));
            } else if let Ok(rename_change) = RenameFileChange::try_from(item) {
                changes.push(DocumentChange::RenameFile(rename_change));
            } else {
                return Err(item.invalid("unknown `documentChanges` entry"));
            }
        }
        Ok(Self { changes })
    }
}

#[derive(Debug, Clone)]
pub enum DocumentChange {
    TextDocument(TextDocumentChange),
    RenameFile(RenameFileChange),
}

impl nojson::DisplayJson for DocumentChange {
    fn fmt(&self, f: &mut nojson::JsonFormatter<'_, '_>) -> std::fmt::Result {
        match self {
            DocumentChange::TextDocument(change) => change.fmt(f),
            DocumentChange::RenameFile(change) => change.fmt(f),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextDocumentChange {
    pub text_document: TextDocument,
    pub edits: Vec<TextEdit>,
}

impl nojson::DisplayJson for TextDocumentChange {
    fn fmt(&self, f: &mut nojson::JsonFormatter<'_, '_>) -> std::fmt::Result {
        f.object(|f| {
            f.member("textDocument", &self.text_document)?;
            f.member("edits", &self.edits)
        })
    }
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for TextDocumentChange {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        let object = JsonObject::new(value)?;
        let text_document = object.convert_required("textDocument")?;
        let edits = object.convert_required("edits")?;
        Ok(Self {
            text_document,
            edits,
        })
    }
}

#[derive(Debug, Clone)]
pub struct RenameFileChange {
    pub old_uri: DocumentUri,
    pub new_uri: DocumentUri,
}

impl RenameFileChange {
    fn apply(&self) -> orfail::Result<()> {
        let old_path = self.old_uri.path();
        let new_path = self.new_uri.path();

        if let Some(parent) = new_path.parent() {
            std::fs::create_dir_all(parent).or_fail_with(|e| {
                format!("Failed to create directory '{}': {e}", parent.display())
            })?;
        }

        std::fs::rename(old_path, new_path).or_fail_with(|e| {
            format!(
                "Failed to rename '{}' to '{}': {e}",
                old_path.display(),
                new_path.display(),
            )
        })?;

        Ok(())
    }
}

impl nojson::DisplayJson for RenameFileChange {
    fn fmt(&self, f: &mut nojson::JsonFormatter<'_, '_>) -> std::fmt::Result {
        f.object(|f| {
            f.member("kind", "rename")?;
            f.member("oldUri", &self.old_uri)?;
            f.member("newUri", &self.new_uri)
        })
    }
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for RenameFileChange {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        let object = JsonObject::new(value)?;
        let kind = object.get_required("kind")?;
        if kind.to_unquoted_string_str()? != "rename" {
            return Err(kind.invalid("unsupported kind"));
        }
        Ok(Self {
            old_uri: object.convert_required("oldUri")?,
            new_uri: object.convert_required("newUri")?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct TextDocument {
    pub uri: DocumentUri,
    pub version: Option<u32>,
}

impl nojson::DisplayJson for TextDocument {
    fn fmt(&self, f: &mut nojson::JsonFormatter<'_, '_>) -> std::fmt::Result {
        f.object(|f| {
            f.member("uri", &self.uri)?;
            f.member("version", self.version)
        })
    }
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for TextDocument {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        let object = JsonObject::new(value)?;
        let uri = object.convert_required("uri")?;
        let version = object.convert_required("version")?;
        Ok(Self { uri, version })
    }
}

#[derive(Debug, Clone)]
pub struct TextEdit {
    pub range: PositionRange,
    pub new_text: String,
}

impl nojson::DisplayJson for TextEdit {
    fn fmt(&self, f: &mut nojson::JsonFormatter<'_, '_>) -> std::fmt::Result {
        f.object(|f| {
            f.member("range", &self.range)?;
            f.member("newText", &self.new_text)
        })
    }
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for TextEdit {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        let object = JsonObject::new(value)?;
        let range = object.convert_required("range")?;
        let new_text = object.convert_required("newText")?;
        Ok(TextEdit { range, new_text })
    }
}

impl DocumentChanges {
    pub fn apply(&self) -> orfail::Result<()> {
        // Group edits by file
        let mut files_to_edit: HashMap<DocumentUri, Vec<&TextEdit>> = HashMap::new();

        for change in &self.changes {
            match change {
                DocumentChange::TextDocument(text_change) => {
                    for edit in &text_change.edits {
                        files_to_edit
                            .entry(text_change.text_document.uri.clone())
                            .or_default()
                            .push(edit);
                    }
                }
                DocumentChange::RenameFile(rename_change) => rename_change.apply().or_fail()?,
            }
        }

        // Apply edits to each file
        for (uri, mut edits) in files_to_edit {
            // Convert file:// URI to path
            let file_path = uri.path();

            // Read file content
            let content = std::fs::read_to_string(&file_path).or_fail_with(|e| {
                format!("Failed to read file '{}': {}", file_path.display(), e)
            })?;

            let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

            // Sort edits by position (end to start) to avoid offset issues
            edits.sort_by(|a, b| match b.range.start.line.cmp(&a.range.start.line) {
                std::cmp::Ordering::Equal => b.range.start.character.cmp(&a.range.start.character),
                other => other,
            });

            // Apply edits
            for edit in edits {
                let start_line = edit.range.start.line;
                let start_char = edit.range.start.character;
                let end_line = edit.range.end.line;
                let end_char = edit.range.end.character;

                if start_line == end_line {
                    // Single line edit
                    if start_line < lines.len() {
                        let line = &lines[start_line];
                        let mut chars: Vec<char> = line.chars().collect();
                        if start_char <= chars.len() && end_char <= chars.len() {
                            chars.splice(start_char..end_char, edit.new_text.chars());
                            lines[start_line] = chars.into_iter().collect::<String>();
                        }
                    }
                } else {
                    // Multi-line edit
                    if start_line < lines.len() && end_line < lines.len() {
                        let start_line_content = &lines[start_line];
                        let end_line_content = &lines[end_line];

                        let start_chars: Vec<char> = start_line_content.chars().collect();
                        let end_chars: Vec<char> = end_line_content.chars().collect();

                        if start_char <= start_chars.len() && end_char <= end_chars.len() {
                            let mut new_line = String::new();
                            new_line
                                .push_str(&start_chars[..start_char].iter().collect::<String>());
                            new_line.push_str(&edit.new_text);
                            new_line.push_str(&end_chars[end_char..].iter().collect::<String>());

                            lines.splice(start_line..=end_line, std::iter::once(new_line));
                        }
                    }
                }
            }

            // Write back to file
            let new_content = lines.join("\n");
            let final_content = if content.ends_with('\n') && !new_content.ends_with('\n') {
                new_content + "\n"
            } else {
                new_content
            };

            std::fs::write(&file_path, final_content).or_fail_with(|e| {
                format!("Failed to write file '{}': {}", file_path.display(), e)
            })?;
        }

        Ok(())
    }
}
