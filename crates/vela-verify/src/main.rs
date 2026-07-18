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
    let first = arguments
        .next()
        .unwrap_or_else(|| fail("usage: vela-verify [--claim <exact-claim>] <witness.json>"));
    let (claim, path) = if first == "--claim" {
        let claim = arguments
            .next()
            .unwrap_or_else(|| fail("usage: vela-verify [--claim <exact-claim>] <witness.json>"));
        let path = arguments
            .next()
            .unwrap_or_else(|| fail("usage: vela-verify [--claim <exact-claim>] <witness.json>"));
        (Some(claim), path)
    } else {
        (None, first)
    };
    if arguments.next().is_some() {
        fail("usage: vela-verify [--claim <exact-claim>] <witness.json>");
    }
    let path = Path::new(&path);
    let bytes = read_bounded(path);
    let witness: vela_verify::Witness = serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| fail(format!("parse {}: {error}", path.display())));
    let mut result = vela_verify::verify_witness(&witness);
    if result.ok
        && let Some(claim) = claim
    {
        let claim = claim
            .to_str()
            .unwrap_or_else(|| fail("exact claim must be valid UTF-8"));
        let faithfulness = vela_verify::claim_witness_faithful(claim, &witness);
        if !faithfulness.faithful {
            result = vela_verify::VerifyResult::fail(format!(
                "claim is not faithful to the witness: {}",
                faithfulness.reasons.join("; ")
            ));
        }
    }
    println!(
        "{}",
        serde_json::to_string(&result).expect("VerifyResult serializes")
    );
    if !result.ok {
        std::process::exit(1);
    }
}
