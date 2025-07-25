pub trait JsonRpcRequest {
    fn method(&self) -> &str;
    fn params(&self, f: &mut nojson::JsonObjectFormatter<'_, '_, '_>) -> std::fmt::Result;
}

pub fn json_object<F>(members: F) -> impl nojson::DisplayJson
where
    F: Fn(&mut nojson::JsonObjectFormatter<'_, '_, '_>) -> std::fmt::Result,
{
    nojson::json(move |f| f.object(|f| members(f)))
}
