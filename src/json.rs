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

#[derive(Debug)]
pub struct JsonObject<'text, 'raw>(nojson::RawJsonValue<'text, 'raw>);

impl<'text, 'raw> JsonObject<'text, 'raw> {
    pub fn new(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, nojson::JsonParseError> {
        let _ = value.to_object()?; // Check `value` is an object
        Ok(Self(value))
    }

    pub fn get_required(
        &self,
        name: &str,
    ) -> Result<nojson::RawJsonValue<'text, 'raw>, nojson::JsonParseError> {
        self.0.to_member(name)?.required()
    }

    pub fn convert_required<T>(&self, name: &str) -> Result<T, nojson::JsonParseError>
    where
        T: TryFrom<nojson::RawJsonValue<'text, 'raw>, Error = nojson::JsonParseError>,
    {
        self.get_required(name)?.try_into()
    }

    pub fn get_optional(&self, name: &str) -> Option<nojson::RawJsonValue<'text, 'raw>> {
        self.0.to_member(name).expect("bug").get()
    }

    pub fn convert_optional<T>(&self, name: &str) -> Result<Option<T>, nojson::JsonParseError>
    where
        T: TryFrom<nojson::RawJsonValue<'text, 'raw>, Error = nojson::JsonParseError>,
    {
        self.0.to_member(name)?.map(T::try_from)
    }

    pub fn convert_optional_or_default<T>(&self, name: &str) -> Result<T, nojson::JsonParseError>
    where
        T: TryFrom<nojson::RawJsonValue<'text, 'raw>, Error = nojson::JsonParseError> + Default,
    {
        Ok(self.convert_optional(name)?.unwrap_or_default())
    }
}
