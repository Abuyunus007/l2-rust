//! `Threads.ini`.
// Java: org/l2jmobius/commons/config/ThreadConfig.java

use crate::IniFile;

/// File name inside the config directory.
pub const THREADS_CONFIG_FILE: &str = "Threads.ini";

/// Thread pool sizes. The Rust servers use tokio, so these are informational for now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadConfig {
    /// Key: `ScheduledThreadPoolSize`, default -1, which Java replaces by `processors * 4`.
    /// Any other value (0, negative) is kept as is.
    pub scheduled_thread_pool_size: i32,
    /// Derived, no key: `max(2, scheduled / 4)` when both `scheduled_thread_pool_size` and
    /// `instant_thread_pool_size` are above 2, else 0.
    pub high_priority_scheduled_thread_pool_size: i32,
    /// Key: `InstantThreadPoolSize`, default -1, which Java replaces by `processors * 2`.
    pub instant_thread_pool_size: i32,
    /// Key: `ThreadsForLoading`, default false.
    pub threads_for_loading: bool,
}

impl ThreadConfig {
    /// Reads the values like `ThreadConfig.load()`.
    ///
    /// `processors` is `Runtime.availableProcessors()`; it is a parameter so tests do not depend
    /// on the machine.
    pub fn from_ini(ini: &IniFile, processors: i32) -> Self {
        let mut scheduled_thread_pool_size = ini.get_int("ScheduledThreadPoolSize", -1);
        if scheduled_thread_pool_size == -1 {
            scheduled_thread_pool_size = processors.saturating_mul(4);
        }

        let mut instant_thread_pool_size = ini.get_int("InstantThreadPoolSize", -1);
        if instant_thread_pool_size == -1 {
            instant_thread_pool_size = processors.saturating_mul(2);
        }

        let high_priority_scheduled_thread_pool_size =
            if scheduled_thread_pool_size > 2 && instant_thread_pool_size > 2 {
                (scheduled_thread_pool_size / 4).max(2)
            } else {
                0
            };

        // Java checks `containsKey` first, so a missing key does not log a warning.
        let threads_for_loading =
            ini.contains_key("ThreadsForLoading") && ini.get_bool("ThreadsForLoading", false);

        Self {
            scheduled_thread_pool_size,
            high_priority_scheduled_thread_pool_size,
            instant_thread_pool_size,
            threads_for_loading,
        }
    }
}
