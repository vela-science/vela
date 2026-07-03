//! Feedback for the silent seconds: a braille spinner on stderr while
//! a verifier replays or a frontier materializes. Restraint rules:
//! stderr only (never corrupts a --json stdout), TTY-gated (piped runs
//! get one plain line), NO_COLOR respected via cli_style, finishes as
//! one permanent moss line. No dependency — ~60 lines beat pulling a
//! progress framework whose aesthetics we would spend longer fighting.

use std::io::{IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use vela_protocol::cli_style as style;

const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub(crate) struct Spinner {
    running: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
    started: Instant,
    live: bool,
}

impl Spinner {
    pub(crate) fn start(msg: &str) -> Self {
        let started = Instant::now();
        let live = std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        let running = Arc::new(AtomicBool::new(true));
        let handle = if live {
            let flag = running.clone();
            let msg = msg.to_string();
            Some(std::thread::spawn(move || {
                let mut i = 0usize;
                while flag.load(Ordering::Relaxed) {
                    eprint!(
                        "\r  {} {} · {:.1}s ",
                        FRAMES[i % FRAMES.len()],
                        msg,
                        started.elapsed().as_secs_f32()
                    );
                    let _ = std::io::stderr().flush();
                    i += 1;
                    std::thread::sleep(std::time::Duration::from_millis(80));
                }
                eprint!("\r{}\r", " ".repeat(msg.len() + 24));
                let _ = std::io::stderr().flush();
            }))
        } else {
            eprintln!("  · {msg} …");
            None
        };
        Self {
            running,
            handle,
            started,
            live,
        }
    }

    /// Stop and leave one permanent line.
    pub(crate) fn finish(mut self, done: &str) {
        self.stop();
        println!(
            "  {} {}  ({:.1}s)",
            style::moss("·"),
            done,
            self.started.elapsed().as_secs_f32()
        );
    }

    fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        // A ceremony that fails mid-spin must not leave a corrupted line.
        if self.live {
            self.stop();
        }
    }
}
