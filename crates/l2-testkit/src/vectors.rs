//! Loading Java-generated test vectors.
//!
//! The Java harness in `java-vectors/` calls the real L2JMobius classes and writes JSON files to
//! `java-vectors/out/<name>.json`. Byte values in those files are lowercase hex strings
//! (convert with [`crate::from_hex`]). The files are committed, so tests run without Java.

use std::path::PathBuf;

use serde::de::DeserializeOwned;

/// The workspace root (two levels above this crate's manifest).
pub fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|crates| crates.parent())
        .map(PathBuf::from)
        .expect("l2-testkit must live in <root>/crates/l2-testkit")
}

/// Path of a vector file: `java-vectors/out/<name>.json`.
pub fn vector_path(name: &str) -> PathBuf {
    workspace_root()
        .join("java-vectors")
        .join("out")
        .join(format!("{name}.json"))
}

/// Loads and deserializes `java-vectors/out/<name>.json`.
///
/// Panics with the file path when the file is missing or doesn't match `T`.
pub fn load<T: DeserializeOwned>(name: &str) -> T {
    let path = vector_path(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read vector file {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| {
        panic!(
            "vector file {} doesn't match the expected shape: {e}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct Sample {
        source: String,
        generated_by: String,
        cases: Vec<SampleCase>,
    }

    #[derive(Deserialize)]
    struct SampleCase {
        input: String,
        output: String,
    }

    #[test]
    fn testkit_loads_vector() {
        let sample: Sample = load("_sample");
        assert_eq!(sample.generated_by, "java-vectors");
        assert!(!sample.source.is_empty());
        assert_eq!(sample.cases.len(), 2);
        assert_eq!(crate::from_hex(&sample.cases[0].input), vec![0x01, 0x02, 0xff]);
        assert_eq!(crate::from_hex(&sample.cases[1].output), Vec::<u8>::new());
    }

    #[test]
    #[should_panic(expected = "cannot read vector file")]
    fn missing_vector_names_the_file() {
        let _: serde_json::Value = load("does_not_exist");
    }
}
