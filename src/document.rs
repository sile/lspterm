use orfail::OrFail;

use crate::{
    json::JsonObject,
    lsp::{DocumentUri, PositionRange},
};

#[derive(Debug, Clone)]
pub struct DocumentChanges {
    pub changes: Vec<DocumentChange>,
}

#[derive(Debug, Clone)]
pub struct DocumentChange {
    pub text_document: TextDocument,
    pub edits: Vec<TextEdit>,
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
    pub fn from_json(value: nojson::RawJsonValue) -> Result<Self, nojson::JsonParseError> {
        let mut changes = Vec::new();
        let document_changes = value.to_member("documentChanges")?.required()?.to_array()?;
        for change in document_changes {
            let text_document = change.to_member("textDocument")?.required()?.try_into()?;
            changes.push(DocumentChange {
                text_document,
                edits: change.to_member("edits")?.required()?.try_into()?,
            });
        }

        Ok(DocumentChanges { changes })
    }

    pub fn apply(&self) -> orfail::Result<()> {
        use std::collections::HashMap;
        use std::fs;

        // Group edits by file
        let mut files_to_edit: HashMap<DocumentUri, Vec<&TextEdit>> = HashMap::new();

        for change in &self.changes {
            for edit in &change.edits {
                files_to_edit
                    .entry(change.text_document.uri.clone())
                    .or_default()
                    .push(edit);
            }
        }

        // Apply edits to each file
        for (uri, mut edits) in files_to_edit {
            // Convert file:// URI to path
            let file_path = uri.path();

            // Read file content
            let content = fs::read_to_string(&file_path).or_fail_with(|e| {
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

            fs::write(&file_path, final_content).or_fail_with(|e| {
                format!("Failed to write file '{}': {}", file_path.display(), e)
            })?;

            eprintln!("Applied changes to: {}", file_path.display());
        }

        Ok(())
    }
}
