#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

/// Disposable SSH agent for authority-bound integration fixtures.
///
/// Tests never inspect or invoke the user's SSH agent or repository-authority
/// key. The private key lives only inside the owning temporary directory.
pub struct EphemeralAgent {
    child: Child,
    socket: PathBuf,
}

impl EphemeralAgent {
    pub fn start(root: &Path, comment: &str) -> Self {
        let socket = root.join("agent.sock");
        let mut child = Command::new("ssh-agent")
            .args(["-D", "-a"])
            .arg(&socket)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start disposable ssh-agent");
        for _ in 0..100 {
            if socket.exists() {
                let agent = Self { child, socket };
                agent.add_key(root, comment);
                return agent;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let _ = child.kill();
        let _ = child.wait();
        panic!("disposable ssh-agent did not create its socket");
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    fn add_key(&self, root: &Path, comment: &str) {
        let key = root.join("repository_authority");
        let generated = Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-C", comment, "-f"])
            .arg(&key)
            .output()
            .expect("generate disposable Ed25519 key");
        assert!(
            generated.status.success(),
            "ssh-keygen: {}",
            String::from_utf8_lossy(&generated.stderr)
        );
        let added = Command::new("ssh-add")
            .arg(&key)
            .env("SSH_AUTH_SOCK", &self.socket)
            .output()
            .expect("load disposable Ed25519 key");
        assert!(
            added.status.success(),
            "ssh-add: {}",
            String::from_utf8_lossy(&added.stderr)
        );
    }
}

impl Drop for EphemeralAgent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
