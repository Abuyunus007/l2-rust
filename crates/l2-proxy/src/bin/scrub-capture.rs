//! `scrub-capture IN OUT [--secret TEXT]...`: makes an `l2-proxy` capture safe to commit as a
//! test fixture (M2-CAP-01). See `l2_proxy::scrub` for what is removed. Prints counts only.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use l2_proxy::capture::{CaptureReader, CaptureWriter};
use l2_proxy::scrub::scrub;

#[derive(Parser)]
#[command(about = "Remove names, chat and session keys from an l2-proxy capture")]
struct Cli {
    input: PathBuf,
    output: PathBuf,
    /// Extra text to replace (clan names, ...). Account and character names, titles and chat
    /// lines are found automatically.
    #[arg(long = "secret")]
    secrets: Vec<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let records = CaptureReader::open(&cli.input)
        .with_context(|| format!("reading {}", cli.input.display()))?
        .read_all()?;
    let (records, report) = scrub(records, &cli.secrets)?;
    let mut writer =
        CaptureWriter::create(&cli.output).with_context(|| format!("writing {}", cli.output.display()))?;
    for record in &records {
        writer.write_record(record)?;
    }
    println!(
        "scrubbed: {} records kept, {} non-game records dropped, {} secrets replaced, {} packets rewritten",
        report.kept, report.dropped, report.secrets, report.rewritten
    );
    Ok(())
}
