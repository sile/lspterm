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
                "client_info",
                json_object(|f| {
                    f.member("name", env!("CARGO_PKG_NAME"))?;
                    f.member("version", env!("CARGO_PKG_VERSION"))
                }),
            )?;
            f.member(
                "workspace_folders",
                [json_object(|f| {
                    f.member("uri", &self.workspace_folder)?;
                    f.member("name", "main")
                })],
            )?;
            f.member("capabilities", ())?;
            Ok(())
        })
    }
}

fn json_object<F>(members: F) -> impl nojson::DisplayJson
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
        f.member("jsonrpc", "2")?;
        f.member("id", id)?;
        f.member("method", method)?;
        f.member("params", nojson::json(|f| f.object(|f| params(f))))
    })
}

// #[derive(Debug, Clone, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct ClientCapabilities {
//     #[serde(default)]
//     pub workspace: WorkspaceCapabilitylies,
//     pub general: GeneralClientCapabilities,
// }

// #[derive(Debug, Default, Clone, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct WorkspaceCapabilitylies {
//     #[serde(default)]
//     pub workspace_edit: WorkspaceEditClientCapabilities,
// }

// #[derive(Debug, Default, Clone, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct GeneralClientCapabilities {
//     #[serde(default)]
//     pub position_encodings: Vec<PositionEncodingKind>,
// }

// #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
// pub enum PositionEncodingKind {
//     #[serde(rename = "utf-8")]
//     Utf8,
//     #[default]
//     #[serde(rename = "utf-16")]
//     Utf16,
//     #[serde(rename = "utf-32")]
//     Utf32,
// }
