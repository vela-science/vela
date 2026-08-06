#![cfg(unix)]
/* Each test binary compiles this module separately, so a helper used by some of
them is dead code in the rest. */
#![allow(dead_code)]

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

/// Deletes one installed trust anchor when the test that created it ends.
///
/// `vela init` writes a local authority trust anchor under the OS ACCOUNT home,
/// resolved through `geteuid`/`getpwuid_r`. That deliberately ignores `$HOME` —
/// there is a test asserting a hostile `HOME` cannot redirect it — so a test
/// cannot sandbox it by setting an environment variable. Every init-based test
/// therefore writes into the developer's real trust store and, without this,
/// leaves the file behind: one run of this suite added dozens.
///
/// The anchor path comes back in `init`'s own JSON. Arm the guard from that and
/// nothing else gets deleted.
pub struct RemoveAnchorOnDrop(pub PathBuf);

impl RemoveAnchorOnDrop {
    /// Build from the stdout of `vela init --json`. Returns `None` when the
    /// init did not report an anchor, which is the case a test asserting a
    /// FAILED init wants.
    pub fn from_init_json(stdout: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(stdout.trim()).ok()?;
        value["authority"]["local_trust"]["anchor_path"]
            .as_str()
            .map(|path| Self(PathBuf::from(path)))
    }
}

impl Drop for RemoveAnchorOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
