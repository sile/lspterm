use orfail::OrFail;

use crate::{
    document::DocumentChanges, proxy_client::ProxyClient, proxy_server::DEFAULT_PORT,
    target::TargetLocation,
};

pub fn try_run(mut args: noargs::RawArgs) -> noargs::Result<Option<noargs::RawArgs>> {
    if !noargs::cmd("rename")
        .doc("TODO")
        .take(&mut args)
        .is_present()
    {
        return Ok(Some(args));
    }

    let target: TargetLocation = noargs::opt("target-location")
        .short('t')
        .ty("FILE[:LINE[:CHARACTER]]")
        .env("LSPTERM_TARGET_LOCATION")
        .example("/path/to/file:0:5")
        .doc("Target location for rename")
        .take(&mut args)
        .then(|a| a.value().parse())?;
    let diff = noargs::flag("diff")
        .short('d')
        .doc("Print the output in diff format")
        .take(&mut args)
        .is_present();
    let apply = noargs::flag("apply")
        .short('a')
        .take(&mut args)
        .is_present();
    let port: u16 = noargs::opt("port")
        .short('p')
        .ty("INTEGER")
        .default(DEFAULT_PORT)
        .env("LSPTERM_PORT")
        .doc("Port number of the LSP proxy server to connect to")
        .take(&mut args)
        .then(|a| a.value().parse())?;
    let new_name: String = noargs::arg("NEW_NAME")
        .example("new-name")
        .take(&mut args)
        .then(|a| a.value().parse())?;

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(None);
    }
    target.file.check_existence().or_fail()?;

    let mut client = ProxyClient::connect(port).or_fail()?;

    let params = nojson::object(|f| {
        target.fmt_json_object(f)?;
        f.member("newName", &new_name)
    });
    let result = client.call("textDocument/rename", params).or_fail()?;
    if diff {
        todo!()
    } else {
        println!("{result}");
    }

    if apply {
        let document_changes = DocumentChanges::from_json(result.value())
            .or_fail_with(|e| format!("Failed to parse document changes: {}", e))?;
        document_changes.apply().or_fail()?;
    }

    Ok(None)
}
