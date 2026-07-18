use std::io::Read;
use std::path::Path;

const MAX_WITNESS_BYTES: u64 = 64 * 1024 * 1024;

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("vela-verify: {message}");
    std::process::exit(1);
}

fn read_bounded(path: &Path) -> Vec<u8> {
    let file = std::fs::File::open(path)
        .unwrap_or_else(|error| fail(format!("open {}: {error}", path.display())));
    let mut bytes = Vec::new();
    file.take(MAX_WITNESS_BYTES + 1)
        .read_to_end(&mut bytes)
        .unwrap_or_else(|error| fail(format!("read {}: {error}", path.display())));
    if bytes.len() as u64 > MAX_WITNESS_BYTES {
        fail(format!(
            "{} exceeds the {}-byte witness limit",
            path.display(),
            MAX_WITNESS_BYTES
        ));
    }
    bytes
}

fn main() {
    let mut arguments = std::env::args_os();
    let _binary = arguments.next();
    let Some(path) = arguments.next() else {
        fail("usage: vela-verify <witness.json>");
    };
    if arguments.next().is_some() {
        fail("usage: vela-verify <witness.json>");
    }
    let path = Path::new(&path);
    let bytes = read_bounded(path);
    let witness: vela_verify::Witness = serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| fail(format!("parse {}: {error}", path.display())));
    let result = vela_verify::verify_witness(&witness);
    println!(
        "{}",
        serde_json::to_string(&result).expect("VerifyResult serializes")
    );
    if !result.ok {
        std::process::exit(1);
    }
}
