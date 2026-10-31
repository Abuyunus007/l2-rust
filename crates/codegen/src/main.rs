//! `codegen`: generates Rust sources from the Java reference code (D-23).
//!
//! ```text
//! cargo run -p codegen -- opcodes            # write crates/*/src/generated/...
//! cargo run -p codegen -- messages           # SystemMessageId / NpcStringId
//! cargo run -p codegen -- config             # game config structs + docs/analysis/config-manual-fields.md
//! cargo run -p codegen -- java-facts         # JSON facts the tests read instead of .java files
//! cargo run -p codegen -- all --check        # compare instead of writing (CI)
//! ```
//!
//! The Java sources are read from `$L2J_ROOT` (default `C:\l2-rust\l2jmobius`). CI has no Java
//! checkout: `--check` then prints a notice and succeeds. The generated files are committed.

mod java_enum;
mod targets;

use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use std::path::{Path, PathBuf};
use targets::Output;

const DEFAULT_L2J_ROOT: &str = r"C:\l2-rust\l2jmobius";

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Target {
    /// Game packet opcode tables (l2-protocol-game).
    Opcodes,
    /// SystemMessageId and NpcStringId tables (l2-protocol-game).
    Messages,
    /// Game server config structs (l2-config) and the report of the fields left for hand-porting.
    Config,
    /// Facts the tests compare with the Java source text (`java-vectors/out/java_facts/*.json`).
    JavaFacts,
    /// Every target.
    All,
}

#[derive(Parser)]
#[command(about = "Generate Rust sources from the Java reference code")]
struct Cli {
    target: Target,
    /// Compare the generated output with the files on disk instead of writing it.
    #[arg(long)]
    check: bool,
}

/// The workspace root (this crate lives in `<root>/crates/codegen`).
fn workspace_root() -> Result<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .context("cannot find the workspace root")
}

fn generate(target: Target, l2j_root: &Path) -> Result<Vec<Output>> {
    match target {
        Target::Opcodes => targets::opcodes::generate(&targets::opcodes::read_sources(l2j_root)?),
        Target::Messages => targets::messages::generate(&targets::messages::read_sources(l2j_root)?),
        Target::Config => targets::config::generate(&targets::config::read_sources(l2j_root)?),
        Target::JavaFacts => targets::java_facts::generate(&targets::java_facts::read_sources(l2j_root)?),
        Target::All => {
            // Every target may emit a shared `generated/mod.rs`; keep one copy of each path.
            let mut all: Vec<Output> = Vec::new();
            for target in [
                Target::Opcodes,
                Target::Messages,
                Target::Config,
                Target::JavaFacts,
            ] {
                for output in generate(target, l2j_root)? {
                    if !all.iter().any(|o| o.path == output.path) {
                        all.push(output);
                    }
                }
            }
            Ok(all)
        }
    }
}

/// Line endings don't matter for the comparison (a Windows checkout may have CRLF).
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let l2j_root = PathBuf::from(std::env::var("L2J_ROOT").unwrap_or_else(|_| DEFAULT_L2J_ROOT.to_string()));

    if !l2j_root.is_dir() {
        if cli.check {
            println!(
                "codegen: Java source not found at {} (set L2J_ROOT); skipping --check",
                l2j_root.display()
            );
            return Ok(());
        }
        bail!("Java source not found at {} (set L2J_ROOT)", l2j_root.display());
    }

    let root = workspace_root()?;
    let outputs = generate(cli.target, &l2j_root)?;

    if cli.check {
        let stale: Vec<_> = outputs
            .iter()
            .filter(|o| {
                std::fs::read_to_string(root.join(&o.path))
                    .map(|disk| normalize(&disk) != o.contents)
                    .unwrap_or(true)
            })
            .map(|o| o.path.display().to_string())
            .collect();
        if !stale.is_empty() {
            bail!(
                "generated files are out of date: {}\nrun `cargo run -p codegen -- {}` and commit the result",
                stale.join(", "),
                cli.target
                    .to_possible_value()
                    .map(|v| v.get_name().to_string())
                    .unwrap_or_default()
            );
        }
        println!("codegen: {} generated file(s) are up to date", outputs.len());
        return Ok(());
    }

    for output in &outputs {
        let path = root.join(&output.path);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        std::fs::write(&path, &output.contents).with_context(|| format!("writing {}", path.display()))?;
        println!("codegen: wrote {}", output.path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs only where the Java checkout exists (not in CI). Verifies the real tables parse and that
    /// the counts match docs/analysis/02-network-crypto.md.
    #[test]
    fn real_java_counts_and_committed_output() {
        let root = PathBuf::from(std::env::var("L2J_ROOT").unwrap_or_else(|_| DEFAULT_L2J_ROOT.to_string()));
        if !root.is_dir() {
            eprintln!("skipped: no Java source at {}", root.display());
            return;
        }
        let src = targets::opcodes::read_sources(&root).unwrap();
        let count = |text: &str, name: &str| java_enum::parse_enum(text, name).unwrap().len();
        assert_eq!(count(&src.client, "ClientPackets"), 170);
        assert_eq!(count(&src.ex_client, "ExClientPackets"), 144);
        assert_eq!(count(&src.server, "ServerPackets"), 485);

        let msg = targets::messages::read_sources(&root).unwrap();
        let sm = targets::messages::parse_entries(&msg.system_message_id, "SystemMessageId").unwrap();
        let ns = targets::messages::parse_entries(&msg.npc_string_id, "NpcStringId").unwrap();
        assert_eq!(sm.len(), 3236);
        assert_eq!(ns.len(), 5915);

        // The committed files equal the generator output (same as `--check`).
        let ws = workspace_root().unwrap();
        for o in generate(Target::All, &root).unwrap() {
            let disk = std::fs::read_to_string(ws.join(&o.path)).unwrap();
            assert_eq!(normalize(&disk), o.contents, "{} is stale", o.path.display());
        }
    }
}
