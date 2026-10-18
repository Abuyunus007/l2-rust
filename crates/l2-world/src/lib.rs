//! The game world (D-10..D-15). Design: `docs/design/world-loop.md`.
//!
//! One [`World`] owns all game state and runs on one thread ([`runner::spawn`]). Everything
//! else talks to it through [`WorldCommand`]s and hears back through [`PacketSink`] (per
//! player connection) and [`Outbox`] (persistence, login server link). Tests drive the world
//! synchronously with [`World::handle`] and [`World::tick`].
//!
//! Rules for code in this crate:
//! - Never block, never await, never touch the database or a socket.
//! - Refer to other objects by id ([`ObjectId`]), never by reference (D-11).
//! - Randomness only from [`World::rng`] (D-13); delays only through the timer wheel (D-12).

pub mod chat;
pub mod command;
pub mod data;
pub mod door;
pub mod enter;
pub mod ids;
pub mod leave;
pub mod objects;
pub mod outbox;
pub mod player;
mod player_view;
pub mod region;
pub mod runner;
pub mod snapshot;
pub mod timer;
mod visibility;
pub mod world;

pub use command::{AdminQuery, ClientInput, LeaveReason, SessionId, WorldCommand};
pub use data::{
    AccessLevelStatic, ChatBroadcast, DEFAULT_TITLE_COLOR, DerivedStats, ItemStatic, NoData, RACE_DWARF,
    StaticData, WorldSettings,
};
pub use door::{Door, DoorSnapshot, Fence, StaticObject, WorldObstacles};
pub use ids::{IdError, IdFactory, JAVA_FIRST_OBJECT_ID, RUST_FIRST_OBJECT_ID, RUST_LAST_OBJECT_ID};
pub use l2_util::{ObjectId, WorldRng};
pub use objects::{Npc, ObjectEntry, ObjectKind};
pub use outbox::{LsEvent, Outbox, PacketSink, PersistCommand};
pub use player::{
    CreatureCore, ItemData, MacroCmdData, MacroData, OnlinePlayer, Player, PlayerData, PlayerProfile,
    ShortcutData, SkillData,
};
pub use region::{RegionId, region_of};
pub use snapshot::snapshot_for_store;
pub use timer::{TimerId, TimerKind, TimerWheel};
pub use world::{DEFAULT_AUTOSAVE_INTERVAL, TICK, World, game_time_minutes};
