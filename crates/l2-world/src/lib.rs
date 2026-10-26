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

pub mod actions;
pub mod admin;
pub mod ai;
pub mod augment;
pub mod bot_report;
pub mod bypass;
pub mod chat;
pub mod clan;
pub mod clanhall;
pub mod combat;
pub mod command;
pub mod craft;
pub mod data;
pub mod death;
pub mod dialog;
pub mod door;
pub mod duel;
pub mod effects;
pub mod enchant;
pub mod enter;
pub mod fishing;
pub mod flood;
pub mod formulas;
pub mod fort;
pub mod friends;
pub mod ground_item;
pub mod ids;
pub mod interact;
pub mod inventory;
pub mod item_ops;
pub mod leave;
pub mod mail;
pub mod manor;
pub mod mount;
pub mod movement;
pub mod multisell;
pub mod npc;
pub mod objects;
pub mod outbox;
pub mod party;
pub mod player;
mod player_view;
pub mod punishments;
pub mod pvp;
pub mod region;
pub mod residence;
pub mod respawn;
pub mod rewards;
pub mod runner;
pub mod shop;
pub mod shortcuts;
pub mod siege;
pub mod skills;
pub mod snapshot;
pub mod stats;
pub mod status;
pub mod summon;
pub mod target;
pub mod teleport;
pub mod teleporter;
pub mod territory;
pub mod timer;
pub mod trade;
pub mod transform;
pub mod user_command;
mod visibility;
pub mod warehouse;
pub mod world;
pub mod zone_trap;
pub mod zones;

pub use admin::{AdminHandler, AdminHandlers, AdminRights, JavaException};
pub use ai::AiState;
pub use ai::attackable::AiSettings;
pub use augment::{AugmentInput, AugmentSettings, AugmentWorld};
pub use bot_report::{BotPunishment, BotReportRow, BotReportSettings};
pub use bypass::{BypassHandler, BypassHandlers};
pub use clan::{Clan, ClanInput, ClanRecord, ClanSettings, ClanWrite, PlayerClanState};
pub use clanhall::{
    AuctionBidRecord, AuctionRecord, AuctionWrite, ClanHallAuction, ClanHallSettings, ClanHallWorld,
    FlagWarRecord, HallGuardRecord, HallScript, HallSiegeHallLoad, HallSiegeLoad, HallSiegeRows,
    HallSiegeSettings, HallSiegeWrite, SiegeStatus,
};
pub use combat::{AggroInfo, CombatSettings, CombatState, HitOutcome, HitTask};
pub use command::{AdminQuery, ClientInput, LeaveReason, SessionId, WorldCommand};
pub use craft::{
    CraftInput, CraftSettings, CraftWorld, ManufactureItem, PlayerCraft, RecipeBookEntry, RecipeLoad,
    RecipeWrite,
};
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
pub use enchant::{EnchantInput, EnchantSettings, EnchantState, EnchantWorld};
pub use fishing::{
    ChampionshipData, ChampionshipRow, FishingSettings, FishingTimer, FishingWorld, PlayerFishing,
};
pub use flood::{
    FloodAction, FloodCheck, FloodConfig, FloodKind, FloodPunishment, FloodSettings, PlayerFlood,
};
pub use fort::{FortInput, FortSettings};
pub use friends::{FriendEntry, FriendWorld, FriendWrite};
pub use ground_item::{GroundItem, GroundItemRow, GroundItemSettings, GroundWorld};
pub use ids::{IdError, IdFactory, JAVA_FIRST_OBJECT_ID, RUST_FIRST_OBJECT_ID, RUST_LAST_OBJECT_ID};
pub use inventory::InventorySettings;
pub use inventory::handlers::SavedItemReuse;
pub use inventory::handlers::shots::{ShotHandler, ShotType};
pub use l2_util::{ObjectId, WorldRng};
pub use mail::{MailCharacter, MailInput, MailLoad, MailMessage, MailRefresh, MailSettings, MailWrite};
pub use manor::{
    CropProcure, ManorInput, ManorLoad, ManorMode, ManorProcureRow, ManorProductionRow, ManorSettings,
    ManorTimer, ManorWrite, SeedProduction,
};
pub use movement::{
    Intention, MoveData, MovementState, NpcMovement, PathRequests, object_of_requester, requester_of,
};
pub use multisell::{MultisellSettings, PreparedList};
pub use npc::{NpcClass, NpcSettings, SpawnReport};
pub use objects::{Npc, ObjectEntry, ObjectKind};
pub use outbox::{ItemRowOp, LsEvent, Outbox, PacketSink, PersistCommand};
pub use party::{
    CommandChannelId, CommandChannelWorld, PartyExpType, PartyId, PartyMessageType, PartySettings, PartyWorld,
};
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
pub use residence::{
    CastleId, ClanHallId, FortId, ResidenceLoad, ResidenceRef, ResidenceRefresh, ResidenceSettings,
    ResidenceWrite,
};
pub use rewards::drop::{DropSettings, ItemHolder};
pub use rewards::exp::{ExpSettings, ExpWorld};
pub use rewards::nevit::NevitSettings;
pub use rewards::vitality::VitalitySettings;
pub use shop::{ProductRow, ShopInput, ShopSettings, ShopState, ShopTimer, buy_handler};
pub use siege::{
    CastleSiegeConfig, Siege, SiegeClan, SiegeClanRecord, SiegeClanType, SiegeGuardRecord, SiegeInput,
    SiegeLoad, SiegeSettings, SiegeTimer, TowerSpawn,
};
pub use skills::learn::LearnSettings;
pub use skills::{CastSettings, CastTimer, MagicUseTask, ReuseTimeStamp, SkillUse};
pub use snapshot::snapshot_for_store;
pub use stats::{
    BaseStat, BaseStatBonus, CalcSkill, Calculators, ClassBalance, FuncOwner, StatData, StatSettings,
    StatView,
};
pub use status::RegenSettings;
pub use summon::pet::{PetRowData, PetRowWrite, PetSettings, PetState, SavedPet, SavedPetEffect};
pub use summon::pet_items::PetInput;
pub use summon::pet_offline::OfflinePetRows;
pub use summon::servitor::{SavedSummonEffect, SummonSkillSave, SummonWrite};
pub use summon::{Summon, SummonKind, SummonSettings};
pub use teleport::{TeleportSettings, TeleportState, TeleportTarget, TeleportWhereType};
pub use territory::{TerritoryInput, TerritoryLoad, TerritorySettings, TerritoryWrite};
pub use timer::{TimerId, TimerKind, TimerWheel};
pub use trade::{
    AddItemRules, PrivateStoreType, StoreInput, StoreSettings, TradeItem, TradeList, TradeState,
};
pub use warehouse::{ContainerRef, PlayerWarehouse, WarehouseInput, WarehouseLoad, WarehouseSettings};
pub use world::{DEFAULT_AUTOSAVE_INTERVAL, TICK, World, game_time_minutes};
pub use zones::{PlayerZones, ZoneId, ZoneSettings};
