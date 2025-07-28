use std::{io::BufReader, net::TcpStream};

use orfail::OrFail;

use crate::{json::JsonObject, lsp};

#[derive(Debug)]
pub struct ProxyClient {
    stream: BufReader<TcpStream>,
    next_request_id: u32,
}

impl ProxyClient {
    pub fn connect(port: u16) -> orfail::Result<Self> {
        let stream = BufReader::new(TcpStream::connect(("127.0.0.1", port)).or_fail()?);
        Ok(Self {
            stream,
            next_request_id: 0,
        })
    }

    pub fn call<T>(&mut self, method: &str, params: T) -> orfail::Result<nojson::RawJsonOwned>
    where
        T: nojson::DisplayJson,
    {
        let request_id = self.next_request_id;
        self.next_request_id += 1;
        lsp::send_request(self.stream.get_mut(), request_id, method, params).or_fail()?;

        let response = lsp::recv_message(&mut self.stream).or_fail()?.or_fail()?;
        let response = JsonObject::new(response.value()).or_fail()?;

        let id: u32 = response.convert_required("id").or_fail()?;
        (request_id == id)
            .or_fail_with(|()| format!("expected request id {request_id}, but got {id}"))?;

        if let Some(error) = response.get_optional("error") {
            return Err(orfail::Failure::new(format!(
                "LSP server returned error: {error}"
            )));
        }

        response.convert_required("result").or_fail()
    }

    pub fn cast<T>(&mut self, method: &str, params: T) -> orfail::Result<()>
    where
        T: nojson::DisplayJson,
    {
        lsp::send_notification(self.stream.get_mut(), method, params).or_fail()?;
        Ok(())
    }
}
