//! Exposes the pinned claude-codes revision to the binary so boot logs can
//! state build provenance. Binary string-grepping proved unsound for
//! verifying deployed crate content (release LLVM inlines short byte
//! literals into immediates), so the chain is asserted at the source:
//! Cargo.lock -> compile-time env -> boot log line.

use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=../../Cargo.lock");

    let lock = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock"))
        .unwrap_or_default();

    let mut rev = String::from("unknown");
    let mut in_claude_codes = false;
    for line in lock.lines() {
        if line.starts_with("[[package]]") {
            in_claude_codes = false;
        }
        if line.trim() == "name = \"claude-codes\"" {
            in_claude_codes = true;
        }
        if in_claude_codes && line.trim_start().starts_with("source = ") {
            rev = match line.find("rev=") {
                Some(i) => {
                    let tail = &line[i + 4..];
                    let end = tail.find(['#', '"']).unwrap_or(tail.len());
                    tail[..end].to_string()
                }
                None => "crates-io".to_string(),
            };
            break;
        }
    }

    println!("cargo:rustc-env=CLAUDE_CODES_REV={rev}");
}
