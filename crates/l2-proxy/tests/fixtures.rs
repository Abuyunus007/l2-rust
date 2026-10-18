//! The committed capture fixtures (M2-CAP-01): they parse, and they went through the scrubber.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use l2_proxy::capture::{CaptureReader, Channel, Dir};

fn fixtures() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/captures");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("java-gs-") && n.ends_with(".jsonl"))
        })
        .collect();
    files.sort();
    files
}

#[test]
fn every_game_fixture_parses_and_is_scrubbed() {
    let files = fixtures();
    assert!(!files.is_empty(), "no java-gs-*.jsonl fixtures found");
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let records = CaptureReader::open(&path).unwrap().read_all().unwrap();
        assert!(!records.is_empty(), "{name} is empty");
        for (i, r) in records.iter().enumerate() {
            let raw = r
                .raw_bytes()
                .unwrap_or_else(|_| panic!("{name} record {i}: bad hex"));
            // The scrubber keeps the game channel only.
            assert_eq!(r.channel, Channel::Game, "{name} record {i}: not a game packet");
            // AuthLogin carries the session key: only its opcode may be stored.
            if r.dir == Dir::C2s && r.opcode == 0x2B {
                assert_eq!(raw.len(), 1, "{name} record {i}: AuthLogin body was kept");
            }
            // KeyPacket: the 8 key bytes are zeroed.
            if r.dir == Dir::S2c && r.opcode == 0x2E && raw.len() >= 10 {
                assert_eq!(&raw[2..10], &[0; 8], "{name} record {i}: game key was kept");
            }
        }
    }
}
