use std::path::PathBuf;

#[derive(Debug)]
pub struct InitializeRequest {
    pub id: u64,
    pub workspace_folder: PathBuf,
}

impl nojson::DisplayJson for InitializeRequest {
    fn fmt(&self, f: &mut nojson::JsonFormatter<'_, '_>) -> std::fmt::Result {
        fmt_request(f, self.id, "initialize", |f| {
            f.member(
                "clientInfo",
                object(|f| {
                    f.member("name", env!("CARGO_PKG_NAME"))?;
                    f.member("version", env!("CARGO_PKG_VERSION"))
                }),
            )?;
            f.member(
                "workspaceFolders",
                [object(|f| {
                    f.member("uri", format!("file://{}", self.workspace_folder.display()))?;
                    f.member("name", "main")
                })],
            )?;
            f.member(
                "capabilities",
                object(|f| {
                    f.member(
                        "general",
                        object(|f| f.member("positionEncodings", ["utf-8"])),
                    )
                }),
            )?;
            Ok(())
        })
    }
}

pub trait JsonRpcRequest {
    fn method(&self) -> &str;
    fn params(&self) -> Option<&dyn nojson::DisplayJson>;
}

fn object<F>(members: F) -> impl nojson::DisplayJson
where
    F: Fn(&mut nojson::JsonObjectFormatter<'_, '_, '_>) -> std::fmt::Result,
{
    nojson::json(move |f| f.object(|f| members(f)))
}

fn fmt_request<F>(
    f: &mut nojson::JsonFormatter<'_, '_>,
    id: u64,
    method: &str,
    params: F,
) -> std::fmt::Result
where
    F: Fn(&mut nojson::JsonObjectFormatter<'_, '_, '_>) -> std::fmt::Result,
{
    f.object(|f| {
        f.member("jsonrpc", "2.0")?;
        f.member("id", id)?;
        f.member("method", method)?;
        f.member("params", nojson::json(|f| f.object(|f| params(f))))
    })
}
