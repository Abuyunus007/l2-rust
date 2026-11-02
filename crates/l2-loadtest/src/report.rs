//! The summary table and the CSV of the samples.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::sampler::Sample;
use crate::stats::{Summary, TimingSummaries};

/// The tick p99 the server must stay under (the world ticks every 50 ms).
pub const TICK_P99_LIMIT_US: u32 = 50_000;

/// The header of the CSV.
pub const CSV_HEADER: &str =
    "t_secs,in_world,packets_per_s,errors,disconnects,online,memory_mb,tick_avg_ms,tick_p99_ms";

/// Everything a finished run knows.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub requested: u32,
    pub started: u32,
    pub logged_in: u32,
    pub entered: u32,
    pub peak_in_world: u32,
    pub restarts: u32,
    pub errors: u32,
    pub disconnects: u32,
    pub packets_rx: u64,
    pub packets_tx: u64,
    pub elapsed_secs: f64,
    pub timings: TimingSummaries,
    pub error_table: Vec<(String, u32)>,
    pub samples: Vec<Sample>,
}

/// Did the server pass (tick p99 under 50 ms, no disconnects)?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    /// No admin samples: the tick time was not measured.
    Unchecked,
}

impl Outcome {
    /// The largest tick p99 of any admin sample, in microseconds. `None` without admin samples.
    pub fn worst_tick_p99_us(&self) -> Option<u32> {
        self.samples
            .iter()
            .filter_map(|s| s.admin.as_ref())
            .filter(|a| a.tick_samples > 0)
            .map(|a| a.tick_p99_us)
            .max()
    }

    pub fn worst_tick_avg_us(&self) -> Option<u32> {
        self.samples
            .iter()
            .filter_map(|s| s.admin.as_ref())
            .filter(|a| a.tick_samples > 0)
            .map(|a| a.tick_avg_us)
            .max()
    }

    /// Disconnects or errors fail the run whatever the tick says; without a tick measurement a
    /// clean run is `Unchecked`, not a pass.
    pub fn verdict(&self) -> Verdict {
        if self.disconnects > 0 || self.errors > 0 {
            return Verdict::Fail;
        }
        match self.worst_tick_p99_us() {
            None => Verdict::Unchecked,
            Some(p99) if p99 < TICK_P99_LIMIT_US => Verdict::Pass,
            Some(_) => Verdict::Fail,
        }
    }
}

fn ms(us: u32) -> String {
    format!("{:.1}", f64::from(us) / 1000.0)
}

fn timing_row(out: &mut String, label: &str, s: Option<Summary>) {
    match s {
        Some(s) => {
            let _ = writeln!(
                out,
                "  {label:<22}{:>7}{:>9}{:>9}{:>9}{:>9}{:>9}",
                s.count,
                ms(s.min),
                ms(s.avg),
                ms(s.p50),
                ms(s.p99),
                ms(s.max)
            );
        }
        None => {
            let _ = writeln!(out, "  {label:<22}{:>7}", 0);
        }
    }
}

/// The summary table as text.
pub fn summary_table(o: &Outcome) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "l2-loadtest summary ({:.0} s)", o.elapsed_secs);
    let _ = writeln!(
        out,
        "  bots requested / started   {} / {}",
        o.requested, o.started
    );
    let _ = writeln!(out, "  logged in (PlayOk)         {}", o.logged_in);
    let _ = writeln!(
        out,
        "  entered the world          {} (peak {} at once, {} restarts)",
        o.entered, o.peak_in_world, o.restarts
    );
    let _ = writeln!(out, "  errors                     {}", o.errors);
    let _ = writeln!(out, "  disconnects                {}", o.disconnects);
    let pps = if o.elapsed_secs > 0.0 {
        o.packets_rx as f64 / o.elapsed_secs
    } else {
        0.0
    };
    let _ = writeln!(
        out,
        "  packets received / sent    {} / {} ({pps:.0} per second received)",
        o.packets_rx, o.packets_tx
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "  {:<22}{:>7}{:>9}{:>9}{:>9}{:>9}{:>9}",
        "times (ms)", "n", "min", "avg", "p50", "p99", "max"
    );
    timing_row(&mut out, "login server connect", o.timings.connect);
    timing_row(&mut out, "login (to PlayOk)", o.timings.login);
    timing_row(&mut out, "game connect + auth", o.timings.game);
    timing_row(&mut out, "enter world", o.timings.enter);
    let _ = writeln!(out);
    let admin: Vec<_> = o.samples.iter().filter_map(|s| s.admin.as_ref()).collect();
    if admin.is_empty() {
        let _ = writeln!(out, "  game server (admin): no samples");
    } else {
        let _ = writeln!(out, "  game server (admin), {} samples:", admin.len());
        let _ = writeln!(
            out,
            "    tick avg, worst sample   {} ms",
            ms(o.worst_tick_avg_us().unwrap_or(0))
        );
        let _ = writeln!(
            out,
            "    tick p99, worst sample   {} ms (limit {} ms)",
            ms(o.worst_tick_p99_us().unwrap_or(0)),
            TICK_P99_LIMIT_US / 1000
        );
        let _ = writeln!(
            out,
            "    online, peak             {}",
            admin.iter().map(|a| a.online).max().unwrap_or(0)
        );
        let _ = writeln!(
            out,
            "    memory, peak             {} MB",
            admin.iter().map(|a| a.memory_bytes).max().unwrap_or(0) / 0x10_0000
        );
    }
    if !o.error_table.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "  errors by cause:");
        for (text, count) in o.error_table.iter().take(10) {
            let _ = writeln!(out, "    {count:>5} x {text}");
        }
    }
    let _ = writeln!(out);
    let verdict = match o.verdict() {
        Verdict::Pass => "PASS (tick p99 under 50 ms, no disconnects, no errors)".to_owned(),
        Verdict::Fail => "FAIL (needs tick p99 under 50 ms, no disconnects, no errors)".to_owned(),
        Verdict::Unchecked => "UNCHECKED (no admin samples, so the tick time was not measured)".to_owned(),
    };
    let _ = writeln!(out, "  verdict: {verdict}");
    out
}

/// The samples as CSV (header first, `\n` line ends). Empty admin columns when a sample has none.
pub fn samples_csv(samples: &[Sample]) -> String {
    let mut out = String::from(CSV_HEADER);
    out.push('\n');
    for s in samples {
        let admin = match &s.admin {
            Some(a) => format!(
                "{},{},{:.3},{:.3}",
                a.online,
                a.memory_bytes / 0x10_0000,
                f64::from(a.tick_avg_us) / 1000.0,
                f64::from(a.tick_p99_us) / 1000.0
            ),
            None => ",,,".to_owned(),
        };
        let _ = writeln!(
            out,
            "{:.1},{},{:.1},{},{},{admin}",
            s.t_secs, s.in_world, s.packets_per_s, s.errors, s.disconnects
        );
    }
    out
}

/// The path of the CSV that belongs to the report file `path`: `<path>.csv`.
pub fn csv_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".csv");
    PathBuf::from(name)
}

/// Writes the summary to `path` and the samples to `<path>.csv`.
pub fn write_report(path: &Path, summary: &str, outcome: &Outcome) -> std::io::Result<()> {
    std::fs::write(path, summary)?;
    std::fs::write(csv_path(path), samples_csv(&outcome.samples))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sampler::AdminSample;

    fn admin(p99_us: u32) -> Option<AdminSample> {
        Some(AdminSample {
            online: 2000,
            memory_bytes: 3 * 0x10_0000,
            tick_avg_us: 4_000,
            tick_p99_us: p99_us,
            tick_samples: 1200,
        })
    }

    fn sample(t: f64, admin: Option<AdminSample>) -> Sample {
        Sample {
            t_secs: t,
            in_world: 5,
            packets_per_s: 120.5,
            errors: 0,
            disconnects: 0,
            admin,
        }
    }

    fn outcome(samples: Vec<Sample>) -> Outcome {
        Outcome {
            requested: 5,
            started: 5,
            logged_in: 5,
            entered: 6,
            peak_in_world: 5,
            restarts: 1,
            errors: 0,
            disconnects: 0,
            packets_rx: 6000,
            packets_tx: 500,
            elapsed_secs: 60.0,
            timings: TimingSummaries::default(),
            error_table: Vec::new(),
            samples,
        }
    }

    #[test]
    fn csv_has_a_header_and_empty_columns_without_admin() {
        let csv = samples_csv(&[sample(5.0, admin(12_345)), sample(10.0, None)]);
        assert_eq!(
            csv,
            "t_secs,in_world,packets_per_s,errors,disconnects,online,memory_mb,tick_avg_ms,tick_p99_ms\n\
             5.0,5,120.5,0,0,2000,3,4.000,12.345\n\
             10.0,5,120.5,0,0,,,,\n"
        );
    }

    #[test]
    fn verdict_needs_tick_under_50_ms_and_no_disconnects() {
        assert_eq!(outcome(vec![sample(5.0, admin(49_999))]).verdict(), Verdict::Pass);
        assert_eq!(outcome(vec![sample(5.0, admin(50_000))]).verdict(), Verdict::Fail);
        assert_eq!(outcome(vec![sample(5.0, None)]).verdict(), Verdict::Unchecked);
        let mut o = outcome(vec![sample(5.0, admin(1_000))]);
        o.disconnects = 1;
        assert_eq!(o.verdict(), Verdict::Fail);
        let mut o = outcome(vec![sample(5.0, admin(1_000))]);
        o.errors = 1;
        assert_eq!(o.verdict(), Verdict::Fail);
    }

    #[test]
    fn the_worst_sample_counts() {
        let o = outcome(vec![
            sample(5.0, admin(10_000)),
            sample(10.0, admin(70_000)),
            sample(15.0, None),
        ]);
        assert_eq!(o.worst_tick_p99_us(), Some(70_000));
        assert_eq!(o.verdict(), Verdict::Fail);
    }

    #[test]
    fn the_table_shows_the_numbers_and_the_verdict() {
        let mut o = outcome(vec![sample(5.0, admin(12_000))]);
        o.error_table = vec![("login failed".into(), 3)];
        let table = summary_table(&o);
        assert!(table.contains("bots requested / started   5 / 5"), "{table}");
        assert!(table.contains("peak 5 at once, 1 restarts"), "{table}");
        assert!(
            table.contains("tick p99, worst sample   12.0 ms (limit 50 ms)"),
            "{table}"
        );
        assert!(table.contains("3 x login failed"), "{table}");
        assert!(summary_table(&outcome(vec![])).contains("UNCHECKED"));
    }

    #[test]
    fn the_csv_goes_next_to_the_report() {
        assert_eq!(
            csv_path(Path::new("out/run1.txt")),
            PathBuf::from("out/run1.txt.csv")
        );
    }
}
