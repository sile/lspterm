use std::path::Path;

use orfail::OrFail;

pub fn parse_file<P: AsRef<Path>, F, T>(path: P, f: F) -> orfail::Result<T>
where
    F: FnOnce(nojson::RawJsonValue) -> Result<T, nojson::JsonParseError>,
{
    let text = std::fs::read_to_string(&path)
        .or_fail_with(|e| format!("failed to read file {}: {e}", path.as_ref().display()))?;

    // TODO: improve error message
    let json = nojson::RawJson::parse(&text).or_fail()?;
    f(json.value()).or_fail()
}
