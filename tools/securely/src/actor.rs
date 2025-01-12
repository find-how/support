use std::process::Command;
use std::path::PathBuf;

pub struct PhpExecutor;

impl PhpExecutor {
    pub fn new() -> Self {
        PhpExecutor
    }

    pub async fn execute_php(&self, script_path: PathBuf, params: Vec<(String, String)>) -> Result<String, String> {
        let mut cmd = Command::new("php");
        cmd.arg(&script_path);

        for (key, value) in params {
            cmd.env(key, value);
        }

        match cmd.output() {
            Ok(output) => {
                if output.status.success() {
                    Ok(String::from_utf8_lossy(&output.stdout).to_string())
                } else {
                    Err(String::from_utf8_lossy(&output.stderr).to_string())
                }
            }
            Err(e) => Err(e.to_string())
        }
    }
}
