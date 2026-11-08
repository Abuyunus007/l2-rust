//! The course of a rift, without the world: `Idle -> Announced -> Open (wave 1..N) -> Boss ->
//! Completed | Failed -> Idle`. The script ticks it once a second and does what it answers.

/// Where the rift stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    /// No rift; the next one may be planned.
    #[default]
    Idle,
    /// The next rift is announced and opens soon.
    Announced,
    /// The rift is open; its waves come out.
    Open,
    /// The waves are over; the rift boss is out.
    Boss,
}

/// How a rift ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The boss died.
    Completed,
    /// The time ran out first.
    Failed,
    /// A GM stopped it, or the feature was switched off.
    Stopped,
}

/// What the script has to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Tell everybody where and when the rift opens.
    Announce,
    /// The reminder one minute before the start.
    Remind,
    /// Open the rift (its marker).
    Open,
    /// Spawn wave `n` (1 is the first).
    Wave(usize),
    /// Spawn the rift boss.
    Boss,
    /// The rift is over: rewards, announcement, nothing of it left in the world.
    Finish(Outcome),
}

/// The times of `L2R.ini` the course needs, in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    pub lead_ms: i64,
    pub reminder: bool,
    pub wave_count: usize,
    pub wave_interval_ms: i64,
    pub duration_ms: i64,
}

/// The reminder comes this long before the start.
pub const REMIND_MS: i64 = 60_000;

/// The state of the course.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Machine {
    phase: Phase,
    /// Idle and announced: the time until the rift opens (`None`: nothing planned).
    until_open: Option<i64>,
    reminded: bool,
    /// Open and boss: the time since the rift opened.
    elapsed: i64,
    /// The last wave that came out (1 is the first, 0 before the first).
    wave: usize,
    /// The time since that wave came out.
    since_wave: i64,
}

impl Machine {
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The last wave that came out.
    pub fn wave(&self) -> usize {
        self.wave
    }

    /// The rift is open (waves or boss).
    pub fn is_running(&self) -> bool {
        matches!(self.phase, Phase::Open | Phase::Boss)
    }

    /// The time until the rift opens, when one is planned or announced.
    pub fn until_open(&self) -> Option<i64> {
        self.until_open
    }

    /// The time the open rift has left.
    pub fn time_left(&self, timing: &Timing) -> i64 {
        (timing.duration_ms - self.elapsed).max(0)
    }

    /// Plans the next rift `in_ms` from now (only while idle and nothing is planned).
    pub fn plan(&mut self, in_ms: i64) {
        if self.phase == Phase::Idle && self.until_open.is_none() {
            self.until_open = Some(in_ms.max(0));
        }
    }

    /// A GM opens a rift now: announced and opened at once. Nothing while one is open.
    pub fn start_now(&mut self) -> Vec<Action> {
        if self.is_running() {
            return Vec::new();
        }
        let mut out = Vec::new();
        if self.phase == Phase::Idle {
            out.push(Action::Announce);
        }
        self.phase = Phase::Announced;
        self.until_open = Some(0);
        self.reminded = true;
        self.open_if_due(&mut out);
        out
    }

    /// Stops what is going on: an open rift ends as stopped, an announced one is called off.
    pub fn stop(&mut self) -> Vec<Action> {
        match self.phase {
            Phase::Idle => {
                self.until_open = None;
                Vec::new()
            }
            Phase::Announced | Phase::Open | Phase::Boss => {
                let mut out = Vec::new();
                self.finish(Outcome::Stopped, &mut out);
                out
            }
        }
    }

    /// `dt` milliseconds passed. `wave_cleared`: every monster of the last wave is dead;
    /// `boss_dead`: the rift boss is dead.
    pub fn tick(&mut self, dt: i64, timing: &Timing, wave_cleared: bool, boss_dead: bool) -> Vec<Action> {
        let mut out = Vec::new();
        match self.phase {
            Phase::Idle => {
                let Some(until) = self.until_open.as_mut() else {
                    return out;
                };
                *until -= dt;
                if *until <= timing.lead_ms {
                    self.phase = Phase::Announced;
                    // A reminder only when the announcement came more than a minute ahead.
                    self.reminded = timing.lead_ms <= REMIND_MS;
                    out.push(Action::Announce);
                    self.open_if_due(&mut out);
                }
            }
            Phase::Announced => {
                let until = self.until_open.map_or(0, |u| u - dt);
                self.until_open = Some(until);
                if timing.reminder && !self.reminded && until <= REMIND_MS && until > 0 {
                    self.reminded = true;
                    out.push(Action::Remind);
                }
                self.open_if_due(&mut out);
            }
            Phase::Open => {
                self.elapsed += dt;
                self.since_wave += dt;
                if self.elapsed >= timing.duration_ms {
                    self.finish(Outcome::Failed, &mut out);
                } else if wave_cleared || self.since_wave >= timing.wave_interval_ms {
                    if self.wave < timing.wave_count {
                        self.wave += 1;
                        self.since_wave = 0;
                        out.push(Action::Wave(self.wave));
                    } else {
                        self.phase = Phase::Boss;
                        out.push(Action::Boss);
                    }
                }
            }
            Phase::Boss => {
                self.elapsed += dt;
                // The boss dying in the last second still counts.
                if boss_dead {
                    self.finish(Outcome::Completed, &mut out);
                } else if self.elapsed >= timing.duration_ms {
                    self.finish(Outcome::Failed, &mut out);
                }
            }
        }
        out
    }

    fn open_if_due(&mut self, out: &mut Vec<Action>) {
        if self.until_open.is_some_and(|u| u <= 0) {
            self.phase = Phase::Open;
            self.until_open = None;
            self.elapsed = 0;
            self.wave = 1;
            self.since_wave = 0;
            out.push(Action::Open);
            out.push(Action::Wave(1));
        }
    }

    fn finish(&mut self, outcome: Outcome, out: &mut Vec<Action>) {
        *self = Self::default();
        out.push(Action::Finish(outcome));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Timing = Timing {
        lead_ms: 300_000,
        reminder: true,
        wave_count: 3,
        wave_interval_ms: 90_000,
        duration_ms: 1_200_000,
    };

    /// Ticks one second at a time for `ms`, collecting the actions.
    fn run(m: &mut Machine, ms: i64, cleared: bool, boss_dead: bool) -> Vec<Action> {
        let mut out = Vec::new();
        for _ in 0..ms / 1000 {
            out.extend(m.tick(1000, &T, cleared, boss_dead));
        }
        out
    }

    #[test]
    fn nothing_happens_until_a_rift_is_planned() {
        let mut m = Machine::default();
        assert!(run(&mut m, 10_000_000, false, false).is_empty());
        assert_eq!(m.phase(), Phase::Idle);
    }

    #[test]
    fn a_planned_rift_is_announced_reminded_and_opened() {
        let mut m = Machine::default();
        m.plan(600_000);
        // Planning again does not move it.
        m.plan(5_000);
        assert_eq!(m.until_open(), Some(600_000));
        // Five minutes before the start: the announcement.
        assert!(run(&mut m, 299_000, false, false).is_empty());
        assert_eq!(run(&mut m, 1_000, false, false), vec![Action::Announce]);
        assert_eq!(m.phase(), Phase::Announced);
        // One minute before: the reminder.
        assert!(run(&mut m, 239_000, false, false).is_empty());
        assert_eq!(run(&mut m, 1_000, false, false), vec![Action::Remind]);
        // The start: the rift and its first wave.
        assert!(run(&mut m, 59_000, false, false).is_empty());
        assert_eq!(
            run(&mut m, 1_000, false, false),
            vec![Action::Open, Action::Wave(1)]
        );
        assert!(m.is_running());
        assert_eq!(m.wave(), 1);
    }

    #[test]
    fn no_reminder_without_it_or_with_a_short_lead() {
        let mut m = Machine::default();
        let quiet = Timing { reminder: false, ..T };
        m.plan(400_000);
        let mut out = Vec::new();
        for _ in 0..400 {
            out.extend(m.tick(1000, &quiet, false, false));
        }
        assert_eq!(out, vec![Action::Announce, Action::Open, Action::Wave(1)]);
        let short = Timing { lead_ms: 30_000, ..T };
        let mut m = Machine::default();
        m.plan(40_000);
        let mut out = Vec::new();
        for _ in 0..40 {
            out.extend(m.tick(1000, &short, false, false));
        }
        assert_eq!(out, vec![Action::Announce, Action::Open, Action::Wave(1)]);
    }

    #[test]
    fn a_plan_inside_the_lead_announces_and_a_zero_plan_opens_at_once() {
        let mut m = Machine::default();
        m.plan(0);
        assert_eq!(
            m.tick(1000, &T, false, false),
            vec![Action::Announce, Action::Open, Action::Wave(1)]
        );
    }

    #[test]
    fn the_next_wave_comes_when_the_last_is_beaten_or_its_time_is_up() {
        let mut m = Machine::default();
        m.start_now();
        // Beaten: the next wave at once.
        assert_eq!(m.tick(1000, &T, true, false), vec![Action::Wave(2)]);
        // Not beaten: after the interval.
        assert!(run(&mut m, 89_000, false, false).is_empty());
        assert_eq!(run(&mut m, 1_000, false, false), vec![Action::Wave(3)]);
        // After the last wave, the boss.
        assert_eq!(m.tick(1000, &T, true, false), vec![Action::Boss]);
        assert_eq!(m.phase(), Phase::Boss);
        // The boss dies: completed, idle again, nothing planned.
        assert_eq!(
            m.tick(1000, &T, false, true),
            vec![Action::Finish(Outcome::Completed)]
        );
        assert_eq!(m, Machine::default());
    }

    #[test]
    fn the_rift_fails_when_its_time_runs_out() {
        let mut m = Machine::default();
        m.start_now();
        let out = run(&mut m, 1_200_000, false, false);
        assert_eq!(
            out,
            vec![
                Action::Wave(2),
                Action::Wave(3),
                Action::Boss,
                Action::Finish(Outcome::Failed)
            ]
        );
        assert_eq!(m.phase(), Phase::Idle);
        assert_eq!(m.time_left(&T), T.duration_ms);
    }

    #[test]
    fn a_gm_start_opens_at_once_and_stop_ends_it() {
        let mut m = Machine::default();
        assert_eq!(
            m.start_now(),
            vec![Action::Announce, Action::Open, Action::Wave(1)]
        );
        // A second start is refused while it runs.
        assert!(m.start_now().is_empty());
        run(&mut m, 30_000, false, false);
        assert_eq!(m.time_left(&T), 1_170_000);
        assert_eq!(m.stop(), vec![Action::Finish(Outcome::Stopped)]);
        assert_eq!(m, Machine::default());
        // An announced rift opens at once on a start (it was announced already).
        m.plan(100_000);
        run(&mut m, 1_000, false, false);
        assert_eq!(m.phase(), Phase::Announced);
        assert_eq!(m.start_now(), vec![Action::Open, Action::Wave(1)]);
        // Stopping an idle course forgets the plan.
        let mut m = Machine::default();
        m.plan(100_000);
        assert!(m.stop().is_empty());
        assert_eq!(m.until_open(), None);
    }
}
