use std::io::Write;

use orfail::OrFail;

use crate::json::json_object;

pub fn send_request<W, T>(
    mut writer: W,
    request_id: u64,
    method: &str,
    params: T,
) -> orfail::Result<String>
where
    W: Write,
    T: nojson::DisplayJson,
{
    let content = nojson::Json(json_object(|f| {
        f.member("jsonrpc", "2.0")?;
        f.member("id", request_id)?;
        f.member("method", method)?;
        f.member("params", &params)
    }))
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
    let content = nojson::Json(json_object(|f| {
        f.member("jsonrpc", "2.0")?;
        f.member("method", method)?;
        f.member("params", &params)
    }))
    .to_string();

    write!(writer, "Content-Length: {}\r\n", content.len()).or_fail()?;
    write!(writer, "\r\n").or_fail()?;
    write!(writer, "{content}").or_fail()?;
    writer.flush().or_fail()?;

    Ok(content)
}
