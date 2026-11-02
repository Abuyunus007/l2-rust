use std::process::ExitCode;

use clap::Parser;
use l2_loadtest::options::Options;
use l2_loadtest::report;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    // The fake clients panic when a bot goes wrong; `run` counts those panics with their message,
    // so the default hook's output (2000 backtraces) is only noise.
    std::panic::set_hook(Box::new(|info| tracing::debug!("bot panic: {info}")));

    let options = Options::parse();
    let outcome = match l2_loadtest::run::run(&options).await {
        Ok(outcome) => outcome,
        Err(e) => {
            eprintln!("l2-loadtest: {e}");
            return ExitCode::from(2);
        }
    };
    let summary = report::summary_table(&outcome);
    println!("{summary}");
    if let Some(path) = &options.report
        && let Err(e) = report::write_report(path, &summary, &outcome)
    {
        eprintln!("l2-loadtest: cannot write the report: {e}");
        return ExitCode::from(2);
    }
    if outcome.verdict() == report::Verdict::Fail {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
