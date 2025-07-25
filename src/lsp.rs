use std::{
    io::{BufRead, Write},
    path::{Path, PathBuf},
};

use orfail::OrFail;

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

pub fn recv_json<R>(mut reader: R) -> orfail::Result<nojson::RawJsonOwned>
where
    R: BufRead,
{
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let size = reader.read_line(&mut line).or_fail()?;
        (size > 0).or_fail()?;
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

    Ok(json)
}

pub fn recv_ok_response<R, T>(reader: R, request_id: u32) -> orfail::Result<(T, String)>
where
    R: BufRead,
    T: for<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>, Error = nojson::JsonParseError>,
{
    let json = recv_json(reader).or_fail()?;
    let value = json.value();
    let parse = || -> Result<T, nojson::JsonParseError> {
        if let Some(method) = value.to_member("method")?.get() {
            return Err(method.invalid("expected a response, but got request"));
        }

        let id = value.to_member("id")?.required()?;
        if u32::try_from(id)? != request_id {
            return Err(id.invalid(format!("expected ID {request_id}, but got {id}")));
        }

        if let Some(error) = value.to_member("error")?.get() {
            return Err(error.invalid("expected a success response, but got a error response"));
        }

        let result = value.to_member("result")?.required()?;
        result.try_into()
    };

    parse().map(|v| (v, json.to_string())).or_fail()
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

#[derive(Debug)]
pub struct DocumentUri(PathBuf);

impl DocumentUri {
    pub fn new<P: AsRef<Path>>(path: P) -> orfail::Result<Self> {
        let path = path.as_ref().canonicalize().or_fail()?;
        Ok(Self(path))
    }
}

impl nojson::DisplayJson for DocumentUri {
    fn fmt(&self, f: &mut nojson::JsonFormatter<'_, '_>) -> std::fmt::Result {
        f.string(format!("file://{}", self.0.display()))
    }
}
