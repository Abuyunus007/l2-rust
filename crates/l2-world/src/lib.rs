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

pub mod admin;
pub mod ai;
pub mod bypass;
pub mod chat;
pub mod clan;
pub mod combat;
pub mod command;
pub mod data;
pub mod death;
pub mod dialog;
pub mod door;
pub mod effects;
pub mod enter;
pub mod formulas;
pub mod ground_item;
pub mod ids;
pub mod interact;
pub mod inventory;
pub mod item_ops;
pub mod leave;
pub mod movement;
pub mod npc;
pub mod objects;
pub mod outbox;
pub mod party;
pub mod player;
mod player_view;
pub mod punishments;
pub mod pvp;
pub mod region;
pub mod respawn;
pub mod rewards;
pub mod runner;
pub mod shortcuts;
pub mod skills;
pub mod snapshot;
pub mod stats;
pub mod status;
pub mod target;
pub mod teleport;
pub mod teleporter;
pub mod timer;
pub mod trade;
pub mod transform;
pub mod user_command;
mod visibility;
pub mod warehouse;
pub mod world;
pub mod zones;

pub use admin::{AdminHandler, AdminHandlers, AdminRights, JavaException};
pub use ai::AiState;
pub use ai::attackable::AiSettings;
pub use bypass::{BypassHandler, BypassHandlers};
pub use clan::{Clan, ClanInput, ClanRecord, ClanSettings, ClanWrite, PlayerClanState};
pub use combat::{AggroInfo, CombatSettings, CombatState, HitOutcome, HitTask};
pub use command::{AdminQuery, ClientInput, LeaveReason, SessionId, WorldCommand};
pub use data::{
    AccessLevelStatic, ChatBroadcast, DEFAULT_TITLE_COLOR, ItemStatic, NoData, RACE_DWARF, StaticData,
    WorldSettings,
};
pub use death::DeathSettings;
pub use dialog::{HtmlActionScope, INTERACTION_DISTANCE, PlayerTalk, TalkSettings};
pub use door::{Door, DoorSnapshot, Fence, StaticObject, WorldObstacles};
pub use effects::apply::ApplyMode;
pub use effects::{
    BuffInfo, BuffInfoId, EffectHandler, EffectList, EffectSettings, EffectType, IconEntry, ListKind,
    SavedEffect, SkillFinishType, SkillKey, effect_flag,
};
pub use ground_item::{GroundItem, GroundItemRow, GroundItemSettings, GroundWorld};
pub use ids::{IdError, IdFactory, JAVA_FIRST_OBJECT_ID, RUST_FIRST_OBJECT_ID, RUST_LAST_OBJECT_ID};
pub use inventory::InventorySettings;
pub use inventory::handlers::SavedItemReuse;
pub use inventory::handlers::shots::{ShotHandler, ShotType};
pub use l2_util::{ObjectId, WorldRng};
pub use movement::{
    Intention, MoveData, MovementState, NpcMovement, PathRequests, object_of_requester, requester_of,
};
pub use npc::{NpcClass, NpcSettings, SpawnReport};
pub use objects::{Npc, ObjectEntry, ObjectKind};
pub use outbox::{ItemRowOp, LsEvent, Outbox, PacketSink, PersistCommand};
pub use party::{PartyExpType, PartyId, PartyMessageType, PartySettings, PartyWorld};
pub use player::{
    CreatureCore, ItemData, MacroCmdData, MacroData, OnlinePlayer, Player, PlayerData, PlayerProfile,
    ShortcutData, SkillData,
};
pub use punishments::{
    IllegalActionPunishment, NewPunishment, PunishmentAffect, PunishmentRecord, PunishmentSettings,
    PunishmentType, StoredPunishment,
};
pub use pvp::{PvpSettings, PvpTitleColorSettings};
pub use region::{RegionId, region_of};
pub use rewards::drop::{DropSettings, ItemHolder};
pub use rewards::exp::{ExpSettings, ExpWorld};
pub use rewards::nevit::NevitSettings;
pub use rewards::vitality::VitalitySettings;
pub use skills::learn::LearnSettings;
pub use skills::{CastSettings, CastTimer, MagicUseTask, ReuseTimeStamp, SkillUse};
pub use snapshot::snapshot_for_store;
pub use stats::{
    BaseStat, BaseStatBonus, CalcSkill, Calculators, ClassBalance, FuncOwner, StatData, StatSettings,
    StatView,
};
pub use status::RegenSettings;
pub use teleport::{TeleportSettings, TeleportState, TeleportTarget, TeleportWhereType};
pub use timer::{TimerId, TimerKind, TimerWheel};
pub use trade::{AddItemRules, TradeItem, TradeList, TradeState};
pub use warehouse::{ContainerRef, PlayerWarehouse, WarehouseInput, WarehouseLoad, WarehouseSettings};
pub use world::{DEFAULT_AUTOSAVE_INTERVAL, TICK, World, game_time_minutes};
pub use zones::{PlayerZones, ZoneId, ZoneSettings};
