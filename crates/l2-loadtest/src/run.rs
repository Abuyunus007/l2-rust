//! Starts the bots at the ramp rate, runs the sampler beside them and collects the result.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::bot::{BotContext, BotEnd, bot_seed, run_bot};
use crate::options::{Options, OptionsError, account_name};
use crate::report::Outcome;
use crate::sampler::{AdminPoller, sample_loop};
use crate::stats::Stats;

/// How long after the deadline a bot may still be busy (logout, a slow server) before it is
/// aborted and counted as an error.
const GRACE: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(transparent)]
    Options(#[from] OptionsError),
    #[error("cannot resolve the login server '{0}'")]
    Resolve(String),
}

/// Looks `host:port` up, preferring an IPv4 address.
async fn resolve(login: &str) -> Result<SocketAddr, RunError> {
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host(login)
        .await
        .map_err(|_| RunError::Resolve(login.to_owned()))?
        .collect();
    addrs
        .iter()
        .find(|a| a.is_ipv4())
        .or(addrs.first())
        .copied()
        .ok_or_else(|| RunError::Resolve(login.to_owned()))
}

/// The text of a panic payload.
fn panic_text(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "a bot panicked".to_owned()
    }
}

/// Runs the whole load test.
pub async fn run(options: &Options) -> Result<Outcome, RunError> {
    options.validate()?;
    let login_addr = resolve(&options.login).await?;
    let stats = Arc::new(Stats::default());
    let started = Instant::now();
    let deadline = started + Duration::from_secs(options.duration);

    let admin = match &options.admin {
        Some(addr) => match AdminPoller::connect(addr, &options.effective_admin_token()).await {
            Ok(poller) => Some(poller),
            Err(e) => {
                warn!("cannot connect to the admin endpoint {addr}: {e}; the server is not sampled");
                None
            }
        },
        None => None,
    };
    let sampler = tokio::spawn(sample_loop(
        Arc::clone(&stats),
        admin,
        started,
        deadline,
        Duration::from_secs(options.sample_interval),
    ));

    info!(
        "Starting {} bots at {}/s against {login_addr} for {} s.",
        options.count, options.ramp, options.duration
    );
    let mut bots: Vec<JoinHandle<Result<BotEnd, String>>> = Vec::new();
    for index in 0..options.count {
        let due = started + Duration::from_secs_f64(f64::from(index) / options.ramp);
        if due >= deadline {
            warn!("the ramp is longer than --duration: bots {}.. are not started", index + 1);
            break;
        }
        tokio::time::sleep_until(tokio::time::Instant::from_std(due)).await;
        stats.started.fetch_add(1, Ordering::Relaxed);
        bots.push(tokio::spawn(run_bot(BotContext {
            login_addr,
            account: account_name(&options.accounts, index),
            password: options.password.clone(),
            stats: Arc::clone(&stats),
            deadline,
            seed: bot_seed(options.seed, index),
        })));
    }

    let give_up = deadline + GRACE;
    for mut bot in bots {
        let left = give_up.saturating_duration_since(Instant::now());
        match tokio::time::timeout(left, &mut bot).await {
            Ok(Ok(Ok(_))) => {}
            Ok(Ok(Err(text))) => stats.record_error(&text),
            Ok(Err(join)) if join.is_panic() => stats.record_error(&panic_text(join.into_panic())),
            Ok(Err(_)) => stats.record_error("a bot task was cancelled"),
            Err(_) => {
                bot.abort();
                stats.record_error("a bot did not finish in time");
            }
        }
    }
    let samples = sampler.await.unwrap_or_default();

    Ok(Outcome {
        requested: options.count,
        started: stats.started.load(Ordering::Relaxed),
        logged_in: stats.logged_in.load(Ordering::Relaxed),
        entered: stats.entered.load(Ordering::Relaxed),
        peak_in_world: stats.peak_in_world.load(Ordering::Relaxed),
        restarts: stats.restarts.load(Ordering::Relaxed),
        errors: stats.errors.load(Ordering::Relaxed),
        disconnects: stats.disconnects.load(Ordering::Relaxed),
        packets_rx: stats.packets_rx.load(Ordering::Relaxed),
        packets_tx: stats.packets_tx.load(Ordering::Relaxed),
        elapsed_secs: started.elapsed().as_secs_f64(),
        timings: stats.timing_summaries(),
        error_table: stats.error_table(),
        samples,
    })
}
