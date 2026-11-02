//! Samples every few seconds: our own counters and, when `--admin` is given, the game server's
//! `GameInfo` (online count, memory, measured tick time).

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use l2_admin_api::{AdminClient, GameAction, GameInfo, Request, Response};

use crate::stats::Stats;

/// How long one admin request may take. The game server answers from its world thread within
/// a tick or two; a world that needs longer is stuck, and the sample says so by being empty.
const ADMIN_TIMEOUT: Duration = Duration::from_secs(5);

/// The game server's numbers at one sample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminSample {
    pub online: u32,
    pub memory_bytes: u64,
    /// The measured tick duration over the server's last samples, in microseconds.
    pub tick_avg_us: u32,
    pub tick_p99_us: u32,
    pub tick_samples: u32,
}

impl AdminSample {
    pub fn from_info(info: &GameInfo) -> Self {
        Self {
            online: info.online,
            memory_bytes: info.memory_bytes,
            tick_avg_us: info.tick_stats.avg_us,
            tick_p99_us: info.tick_stats.p99_us,
            tick_samples: info.tick_stats.samples,
        }
    }
}

/// One row of the CSV.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// Seconds since the start of the run.
    pub t_secs: f64,
    pub in_world: u32,
    /// Packets received per second since the last sample.
    pub packets_per_s: f64,
    pub errors: u32,
    pub disconnects: u32,
    /// `None`: no `--admin`, or the request failed.
    pub admin: Option<AdminSample>,
}

/// A connection to the game server's admin endpoint.
pub struct AdminPoller {
    client: AdminClient,
}

impl AdminPoller {
    /// Connects to `host:port` (`ws://` is added).
    pub async fn connect(addr: &str, token: &str) -> Result<Self, String> {
        let (client, _events) = AdminClient::connect(&format!("ws://{addr}"), token)
            .await
            .map_err(|e| e.to_string())?;
        Ok(Self { client })
    }

    /// `//serverinfo` as numbers.
    pub async fn poll(&self) -> Result<AdminSample, String> {
        let request = Request::GameAdmin {
            server: None,
            action: GameAction::ServerInfo,
        };
        match tokio::time::timeout(ADMIN_TIMEOUT, self.client.request(request)).await {
            Err(_) => Err("no answer from the admin endpoint".to_owned()),
            Ok(Err(e)) => Err(e.to_string()),
            Ok(Ok(Response::GameInfo { info })) => Ok(AdminSample::from_info(&info)),
            Ok(Ok(Response::Error { message })) => Err(message),
            Ok(Ok(_)) => Err("unexpected answer to the server info request".to_owned()),
        }
    }
}

/// Takes a sample every `interval` until `deadline` (and one at the deadline). `started` is time
/// zero of the run.
pub async fn sample_loop(
    stats: Arc<Stats>,
    admin: Option<AdminPoller>,
    started: Instant,
    deadline: Instant,
    interval: Duration,
) -> Vec<Sample> {
    let mut samples = Vec::new();
    let mut last_at = started;
    let mut last_rx = 0u64;
    loop {
        let next = (last_at + interval).min(deadline);
        tokio::time::sleep_until(tokio::time::Instant::from_std(next)).await;
        let now = Instant::now();
        let rx = stats.packets_rx.load(Ordering::Relaxed);
        let secs = now.duration_since(last_at).as_secs_f64().max(0.001);
        let admin_sample = match &admin {
            Some(poller) => match poller.poll().await {
                Ok(sample) => Some(sample),
                Err(e) => {
                    tracing::warn!("admin sample failed: {e}");
                    None
                }
            },
            None => None,
        };
        samples.push(Sample {
            t_secs: now.duration_since(started).as_secs_f64(),
            in_world: stats.in_world.load(Ordering::Relaxed),
            packets_per_s: rx.saturating_sub(last_rx) as f64 / secs,
            errors: stats.errors.load(Ordering::Relaxed),
            disconnects: stats.disconnects.load(Ordering::Relaxed),
            admin: admin_sample,
        });
        last_at = now;
        last_rx = rx;
        if next >= deadline {
            return samples;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use l2_admin_api::{AdminHandler, Session, TickStats, Token, serve};
    use tokio::net::TcpListener;

    use super::*;

    /// An admin endpoint that answers `ServerInfo` with fixed numbers.
    struct FakeGame;

    impl AdminHandler for FakeGame {
        async fn handle(&self, request: Request, _session: &Session) -> Response {
            match request {
                Request::GameAdmin {
                    action: GameAction::ServerInfo,
                    ..
                } => Response::GameInfo {
                    info: GameInfo {
                        uptime_secs: 10,
                        online: 7,
                        memory_bytes: 123_456,
                        threads: 4,
                        ticks: 200,
                        tick_ms: 50,
                        tick_stats: TickStats {
                            avg_us: 1_500,
                            p99_us: 9_000,
                            samples: 200,
                        },
                        shutdown_in_secs: None,
                    },
                },
                _ => Response::Error {
                    message: "not supported".into(),
                },
            }
        }
    }

    async fn fake_endpoint(token: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(serve(
            listener,
            "game".into(),
            Token::new(token),
            Arc::new(FakeGame),
        ));
        addr
    }

    #[tokio::test]
    async fn polls_the_tick_stats_over_the_admin_endpoint() {
        let addr = fake_endpoint("t0ken").await;
        let poller = AdminPoller::connect(&addr, "t0ken").await.unwrap();
        let sample = poller.poll().await.unwrap();
        assert_eq!(
            sample,
            AdminSample {
                online: 7,
                memory_bytes: 123_456,
                tick_avg_us: 1_500,
                tick_p99_us: 9_000,
                tick_samples: 200,
            }
        );
    }

    #[tokio::test]
    async fn a_wrong_token_is_refused() {
        let addr = fake_endpoint("right").await;
        assert!(AdminPoller::connect(&addr, "wrong").await.is_err());
    }

    #[tokio::test]
    async fn samples_come_at_the_interval_and_end_at_the_deadline() {
        let addr = fake_endpoint("t").await;
        let poller = AdminPoller::connect(&addr, "t").await.unwrap();
        let stats = Arc::new(Stats::default());
        stats.packets_rx.store(500, Ordering::Relaxed);
        stats.world_entered();
        let started = Instant::now();
        let samples = sample_loop(
            Arc::clone(&stats),
            Some(poller),
            started,
            started + Duration::from_millis(450),
            Duration::from_millis(200),
        )
        .await;
        // At 200 ms, 400 ms and the deadline at 450 ms (a slow machine may drop the middle one).
        assert!((2..=3).contains(&samples.len()), "{samples:?}");
        assert!(samples[0].packets_per_s > 0.0 && samples[1].packets_per_s == 0.0);
        assert!(samples.iter().all(|s| s.in_world == 1));
        let last = samples.last().unwrap();
        assert_eq!(last.admin.as_ref().map(|a| a.tick_p99_us), Some(9_000));
        assert!(samples.windows(2).all(|w| w[0].t_secs < w[1].t_secs));
    }

    #[tokio::test]
    async fn without_admin_only_our_numbers_are_sampled() {
        let stats = Arc::new(Stats::default());
        let started = Instant::now();
        let samples = sample_loop(
            stats,
            None,
            started,
            started + Duration::from_millis(50),
            Duration::from_millis(500),
        )
        .await;
        assert_eq!(samples.len(), 1);
        assert!(samples[0].admin.is_none());
    }
}
