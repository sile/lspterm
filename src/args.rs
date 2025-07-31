pub const APPLY_FLAG: noargs::FlagSpec =
    noargs::flag("apply").short('a').doc("Apply the operation");

pub const RAW_FLAG: noargs::FlagSpec = noargs::flag("raw")
    .short('r')
    .doc("Output raw JSON response from LSP server");
