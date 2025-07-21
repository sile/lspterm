use std::fs::File;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use orfail::OrFail;

#[derive(Debug)]
pub struct LspClient {
    process: Child,
}

impl LspClient {
    pub fn new(
        lsp_server_command: PathBuf,
        lsp_server_args: Vec<String>,
        stderr_file: Option<PathBuf>,
    ) -> orfail::Result<Self> {
        let mut command = Command::new(&lsp_server_command);
        command
            .args(&lsp_server_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());

        match stderr_file {
            Some(path) => {
                let file = File::create(&path).or_fail_with(|e| {
                    format!("failed to create stderr file '{}': {e}", path.display())
                })?;
                command.stderr(Stdio::from(file));
            }
            None => {
                command.stderr(Stdio::null());
            }
        }

        let process = command.spawn().or_fail_with(|e| {
            format!(
                "failed to spawn LSP server process '{}': {e}",
                lsp_server_command.display()
            )
        })?;

        Ok(Self { process })
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.process.stdin.take();
        for _ in 0..10 {
            let Ok(status) = self.process.try_wait() else {
                break;
            };
            if status.is_some() {
                return;
            }

            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let _ = self.process.kill();
    }
}
