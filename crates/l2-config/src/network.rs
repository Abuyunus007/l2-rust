//! `Network.ini`.
// Java: org/l2jmobius/commons/network/ConnectionConfig.java

use crate::IniFile;

/// File name inside the config directory.
pub const NETWORK_CONFIG_FILE: &str = "Network.ini";

/// Network settings shared by all servers.
///
/// The per-size buffer pools (`BufferPool.<Name>.Size` / `.BufferSize`) only tune Java's own
/// buffer pooling and are not read. The scalar buffer keys are parsed but nothing uses them.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkConfig {
    /// Key: `ShutdownWaitTime` (seconds, int), default 5. Stored in milliseconds like Java
    /// (`getInt * 1000L`).
    pub shutdown_wait_time: i64,
    /// Key: `ThreadPoolSize`, default 2. Below 1 Java uses `processors * 4`.
    pub thread_pool_size: i32,
    /// Key: `ThreadPriority`, default 5 (`Thread.NORM_PRIORITY`). Unused by Rust.
    pub thread_priority: i32,
    /// Key: `BufferPool.AutoExpandCapacity`, default true. Unused by Rust.
    pub auto_expand_pool_capacity: bool,
    /// Key: `BufferPool.InitFactor`, default 0. Unused by Rust.
    pub init_buffer_pool_factor: f32,
    /// Key: `DropPackets`, default false.
    pub drop_packets: bool,
    /// Key: `DropPacketThreshold`, default 250.
    pub drop_packet_threshold: i32,
    /// Key: `BufferSegmentSize`, default 64 (`ResourcePool._bufferSegmentSize`). Unused by Rust.
    pub buffer_segment_size: i32,
}

impl NetworkConfig {
    /// Reads the values like the `ConnectionConfig` constructor.
    ///
    /// `processors` is `Runtime.availableProcessors()` (a parameter so tests are deterministic).
    pub fn from_ini(ini: &IniFile, processors: i32) -> Self {
        let shutdown_wait_time = i64::from(ini.get_int("ShutdownWaitTime", 5)) * 1000;

        let mut thread_pool_size = ini.get_int("ThreadPoolSize", 2);
        if thread_pool_size < 1 {
            thread_pool_size = processors.saturating_mul(4);
        }

        Self {
            shutdown_wait_time,
            thread_pool_size,
            thread_priority: ini.get_int("ThreadPriority", 5),
            auto_expand_pool_capacity: ini.get_bool("BufferPool.AutoExpandCapacity", true),
            init_buffer_pool_factor: ini.get_float("BufferPool.InitFactor", 0.0),
            drop_packets: ini.get_bool("DropPackets", false),
            drop_packet_threshold: ini.get_int("DropPacketThreshold", 250),
            buffer_segment_size: ini.get_int("BufferSegmentSize", 64),
        }
    }
}
