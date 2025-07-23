#[derive(Debug)]
pub struct Initialization {}

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct RequestMessage {
//     jsonrpc: JsonrpcVersion,
//     pub id: RequestId,
//     pub method: String,
//     #[serde(default)]
//     pub params: serde_json::Value,
// }

// #[derive(Debug, Clone, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct InitializeParams {
//     #[serde(default)]
//     pub root_uri: Option<DocumentUri>,
//     pub client_info: Option<ClientInfo>,
//     pub capabilities: ClientCapabilities,
//     #[serde(default)]
//     pub workspace_folders: Vec<WorkspaceFolder>,
// }

// impl InitializeParams {
//     pub fn root_uri(&self) -> orfail::Result<&DocumentUri> {
//         self.root_uri
//             .as_ref()
//             .or_else(|| self.workspace_folders.first().map(|f| &f.uri))
//             .or_fail_with(|()| "rootUri or workspaceFoldersa is required".to_owned())
//     }
// }

// #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
// pub struct DocumentUri(url::Url);

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct ClientInfo {
//     pub name: String,
//     pub version: String,
// }

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

// #[derive(Debug, Clone, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct WorkspaceFolder {
//     pub uri: DocumentUri,
//     pub name: String,
// }
