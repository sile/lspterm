use std::{
    io::{BufRead, Write},
    path::{Path, PathBuf},
};

use orfail::OrFail;

use crate::json::JsonObject;

pub fn send_request<W, T>(
    mut writer: W,
    request_id: u32,
    method: &str,
    params: T,
) -> orfail::Result<String>
where
    W: Write,
    T: nojson::DisplayJson,
{
    let content = nojson::object(|f| {
        f.member("jsonrpc", "2.0")?;
        f.member("id", request_id)?;
        f.member("method", method)?;
        f.member("params", &params)
    })
    .to_string();

    write!(writer, "Content-Length: {}\r\n", content.len()).or_fail()?;
    write!(writer, "\r\n").or_fail()?;
    write!(writer, "{content}").or_fail()?;
    writer.flush().or_fail()?;

    Ok(content)
}

pub fn send_notification<W, T>(mut writer: W, method: &str, params: T) -> orfail::Result<String>
where
    W: Write,
    T: nojson::DisplayJson,
{
    let content = nojson::object(|f| {
        f.member("jsonrpc", "2.0")?;
        f.member("method", method)?;
        f.member("params", &params)
    })
    .to_string();

    write!(writer, "Content-Length: {}\r\n", content.len()).or_fail()?;
    write!(writer, "\r\n").or_fail()?;
    write!(writer, "{content}").or_fail()?;
    writer.flush().or_fail()?;

    Ok(content)
}

pub fn send_response<W, I, T, E>(
    mut writer: W,
    request_id: I,
    result: Result<T, E>,
) -> orfail::Result<String>
where
    W: Write,
    I: nojson::DisplayJson,
    T: nojson::DisplayJson,
    E: nojson::DisplayJson,
{
    let content = nojson::object(|f| {
        f.member("jsonrpc", "2.0")?;
        f.member("id", &request_id)?;
        match &result {
            Ok(v) => f.member("result", v),
            Err(v) => f.member("error", v),
        }
    })
    .to_string();

    write!(writer, "Content-Length: {}\r\n", content.len()).or_fail()?;
    write!(writer, "\r\n").or_fail()?;
    write!(writer, "{content}").or_fail()?;
    writer.flush().or_fail()?;

    Ok(content)
}

pub fn recv_message<R>(mut reader: R) -> orfail::Result<Option<nojson::RawJsonOwned>>
where
    R: BufRead,
{
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let size = reader.read_line(&mut line).or_fail()?;
        if size == 0 {
            return Ok(None);
        }
        if line == "\r\n" {
            break;
        }

        let (k, v) = line.split_once(':').or_fail()?;
        if k.eq_ignore_ascii_case("Content-Length") {
            content_length = Some(v.trim().parse::<usize>().or_fail()?);
        }
    }

    let content_length = content_length.or_fail()?;
    let mut content = vec![0; content_length];
    reader.read_exact(&mut content).or_fail()?;

    let content = String::from_utf8(content).or_fail()?;

    let json = nojson::RawJsonOwned::parse(&content).or_fail()?;
    check_jsonrpc_version(json.value()).or_fail()?;

    Ok(Some(json))
}

fn check_jsonrpc_version(
    value: nojson::RawJsonValue<'_, '_>,
) -> Result<(), nojson::JsonParseError> {
    value.to_member("jsonrpc")?.required()?.map(|v| {
        let version = v.to_unquoted_string_str()?;
        if version == "2.0" {
            Ok(())
        } else {
            Err(v.invalid("unsupported JSON-RPC version"))
        }
    })?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentUri(PathBuf);

impl DocumentUri {
    pub fn new<P: AsRef<Path>>(path: P) -> orfail::Result<Self> {
        let path = std::path::absolute(path).or_fail()?;
        Ok(Self(path))
    }

    pub fn new_dir<P: AsRef<Path>>(path: P) -> orfail::Result<Self> {
        let path = path.as_ref().canonicalize().or_fail()?;
        path.is_dir()
            .or_fail_with(|()| format!("path '{}' is not a directory", path.display()))?;
        Ok(Self(path))
    }

    pub fn read_to_string(&self) -> orfail::Result<String> {
        std::fs::read_to_string(&self.0)
            .or_fail_with(|e| format!("failed to read file '{}': {e}", self.0.display()))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn relative_path<P: AsRef<Path>>(&self, base_dir: P) -> PathBuf {
        match self.0.strip_prefix(base_dir.as_ref()) {
            Ok(relative) => relative.to_path_buf(),
            Err(_) => self.0.clone(),
        }
    }

    pub fn check_existence(&self) -> orfail::Result<()> {
        self.0
            .exists()
            .or_fail_with(|()| format!("file '{}' does not exist", self.0.display()))?;
        Ok(())
    }
}

impl nojson::DisplayJson for DocumentUri {
    fn fmt(&self, f: &mut nojson::JsonFormatter<'_, '_>) -> std::fmt::Result {
        f.string(format!("file://{}", self.0.display()))
    }
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for DocumentUri {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        let uri_string = value.to_unquoted_string_str()?;
        if let Some(path_str) = uri_string.strip_prefix("file://") {
            Ok(Self(PathBuf::from(path_str)))
        } else {
            Err(value.invalid("URI must start with 'file://'"))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub line: usize,
    pub character: usize,
}

impl nojson::DisplayJson for Position {
    fn fmt(&self, f: &mut nojson::JsonFormatter<'_, '_>) -> std::fmt::Result {
        let indent = f.get_indent_size();
        f.set_indent_size(0);
        f.object(|f| {
            f.member("line", self.line)?;
            f.member("character", self.character)
        })?;
        f.set_indent_size(indent);
        Ok(())
    }
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for Position {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        let object = JsonObject::new(value)?;
        Ok(Self {
            line: object.convert_required("line")?,
            character: object.convert_required("character")?,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PositionRange {
    pub start: Position,
    pub end: Position,
}

impl PositionRange {
    pub fn is_multiline(self) -> bool {
        self.end.line > self.start.line
    }

    pub fn get_start_line(self, text: &str) -> Option<&str> {
        let line = text.lines().nth(self.start.line)?.trim_end();
        Some(line)
    }

    pub fn get_range_text(self, text: &str) -> Option<&str> {
        if self.is_multiline() {
            todo!();
        }
        let line = self.get_start_line(text)?;
        line.get(self.start.character..self.end.character)
    }
}

impl nojson::DisplayJson for PositionRange {
    fn fmt(&self, f: &mut nojson::JsonFormatter<'_, '_>) -> std::fmt::Result {
        let indent = f.get_indent_size();
        f.set_indent_size(0);
        f.object(|f| {
            f.member("start", &self.start)?;
            f.member("end", &self.end)
        })?;
        f.set_indent_size(indent);
        Ok(())
    }
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for PositionRange {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        let object = JsonObject::new(value)?;
        Ok(Self {
            start: object.convert_required("start")?,
            end: object.convert_required("end")?,
        })
    }
}
