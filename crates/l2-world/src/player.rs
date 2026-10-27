//! Players and the data shared by every creature (D-17: data structs, no inheritance).
//!
//! Java: `entity/actor/Creature.java`, `Playable.java`, `Player.java`. Only what M2 needs is
//! here; later milestones add fields (effects, AI state…).

use std::collections::BTreeMap;
use std::net::IpAddr;

use l2_util::ObjectId;

use crate::data::DEFAULT_TITLE_COLOR;
use crate::effects::SavedEffect;
use crate::effects::item_extras::ItemElemental;
use crate::inventory::handlers::SavedItemReuse;
use crate::stats::Calculators;

/// What every creature has (players now, NPCs in M3).
#[derive(Debug, Clone, PartialEq)]
pub struct CreatureCore {
    pub object_id: ObjectId,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    pub level: i32,
    pub cur_hp: f64,
    /// The last `getMaxHp()` of the stat engine (Java `PlayerStat._oldMaxHp`, the "stats watch"):
    /// refreshed when the stats change (`World::refresh_max_vitals`), read by `StatusUpdate`.
    pub max_hp: f64,
    pub cur_mp: f64,
    /// The last `getMaxMp()` (see `max_hp`).
    pub max_mp: f64,
    pub cur_cp: f64,
    /// The last `getMaxCp()` (see `max_hp`).
    pub max_cp: f64,
    /// Java `_calculators`: the stat functions (see `stats/`).
    pub calculators: Calculators,
    /// Java `_skills`: every skill the creature knows as `(id, level)`, one per id, in the order
    /// they were added (learned skills, item and armor set skills, penalty skills).
    pub skills: Vec<(i32, i32)>,
}

/// One item of a player (inventory or worn). Mirrors the `items` table row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemData {
    pub object_id: ObjectId,
    /// Template id (`items.item_id`).
    pub item_id: i32,
    pub count: i64,
    pub enchant_level: i32,
    /// `items.loc`: "INVENTORY", "PAPERDOLL", "WAREHOUSE", …
    pub loc: String,
    /// `items.loc_data`: the paperdoll slot when worn.
    pub loc_data: i32,
    pub custom_type1: i32,
    pub custom_type2: i32,
    /// `items.mana_left` (shadow items), -1 = none.
    pub mana_left: i32,
    /// `items.time` (limited-time items), -1 = none.
    pub time: i64,
    /// Java `Item._augmentation`: the augmentation id (`item_attributes.augAttributes`), `None`
    /// for an item that is not augmented. Two option ids are packed in it (see
    /// `l2_data::augmentation`).
    pub augmentation: Option<i32>,
    /// Java `Item._elementals`: the element attributes of the instance (`item_elementals`), in the
    /// order Java keeps them (empty = Java's `null`). See `effects/item_extras.rs`.
    pub elementals: Vec<ItemElemental>,
}

impl Default for ItemData {
    /// A plain item: no owner object, no count, not worn, no shadow mana (-1), no lifetime (-1),
    /// no augmentation, no elements. Build items as `ItemData { .., ..Default::default() }` so a
    /// field added later does not break the literal.
    fn default() -> Self {
        Self {
            object_id: ObjectId(0),
            item_id: 0,
            count: 0,
            enchant_level: 0,
            loc: String::new(),
            loc_data: 0,
            custom_type1: 0,
            custom_type2: 0,
            mana_left: -1,
            time: -1,
            augmentation: None,
            elementals: Vec::new(),
        }
    }
}

/// One learned skill (`character_skills`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillData {
    pub skill_id: i32,
    pub skill_level: i32,
}

/// One shortcut (`character_shortcuts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutData {
    pub slot: i32,
    pub page: i32,
    /// Java `ShortcutType` ordinal (item, skill, action, macro, recipe, bookmark).
    pub kind: i32,
    pub shortcut_id: i32,
    pub level: i32,
}

/// One command of a macro (Java `MacroCmd`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroCmdData {
    /// Java `MacroType` ordinal.
    pub kind: i32,
    pub d1: i32,
    pub d2: i32,
    pub cmd: String,
}

/// One macro (`character_macroses`, Java `Macro`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroData {
    pub id: i32,
    pub icon: i32,
    pub name: String,
    pub descr: String,
    pub acronym: String,
    pub commands: Vec<MacroCmdData>,
}

/// The part of a character that lives in tables the Java game server also owns (D-39): values the
/// enter-world packets show, read from them at load. The persistence task writes back only what
/// the world models: `vitality_points`, the Nevit variables and the hourglass time
/// (`variables_changed`, `variables_removed`, `reco_store`, M4-VIT-01), `exp_before_death` and the
/// saved buffs (`effects_to_store`, M5-SAVE-01); everything else only travels database -> world.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerProfile {
    /// `characters.fame`.
    pub fame: i32,
    /// `characters.nobless`.
    pub noble: bool,
    /// `characters.title_color`.
    pub title_color: i32,
    /// `characters.vitality_points`.
    pub vitality_points: i32,
    /// `characters.pccafe_points` (M6-FU-11, `pccafe.rs`).
    pub pccafe_points: i32,
    /// `characters.bookmarkslot`.
    pub bookmark_slot: i32,
    /// `characters.death_penalty_level`.
    pub death_penalty_level: i32,
    /// Java `_expBeforeDeath`: the exp the character had before a death took some. Java never
    /// reads the column back (it is 0 after a login) but writes the live value with every
    /// store, so this one is written too.
    pub exp_before_death: i64,
    /// `character_reco_bonus.rec_have` / `rec_left` / `time_left` (ms of the Nevit hourglass).
    pub recom_have: i32,
    pub recom_left: i32,
    pub recom_time_left_ms: i64,
    /// `character_macroses`.
    pub macros: Vec<MacroData>,
    /// `character_variables` (Java `PlayerVariables`).
    pub variables: BTreeMap<String, String>,
    /// `character_tpbookmark`: the teleport bookmarks, sorted by id (L-04, `bookmarks.rs`).
    pub bookmarks: Vec<crate::bookmarks::BookmarkData>,
    /// `character_contacts`: the names of the mail contacts (L-04, `contacts.rs`).
    pub contacts: Vec<String>,
    /// `character_hennas`: the dye id (`symbol_id`) of slots 1 to 3 of the active class, 0 for an
    /// empty slot (Java `restoreHenna`).
    pub hennas: [i32; 3],
    /// `characters.lastAccess` as loaded (Java `_lastAccess`): milliseconds of the last logout.
    pub last_access: i64,
    /// The variables the world set since the character was loaded (Java `AbstractVariables`
    /// `_added` and `_modified`): the persistence task upserts them with every store. Idempotent,
    /// so they are never cleared.
    pub variables_changed: std::collections::BTreeSet<String>,
    /// The variables the world removed since the load (Java `_deleted`): deleted with every store.
    pub variables_removed: std::collections::BTreeSet<String>,
    /// The world keeps `character_reco_bonus` (Nevit is on): a store writes `recom_have`,
    /// `recom_left` and `recom_time_left_ms` (Java `storeRecommendations`).
    pub reco_store: bool,
    /// The `character_skills_save` rows of the last logout, loaded for the active class index in
    /// buff order (Java `restoreEffects` reads them). The world takes them at enter world, so they
    /// are restored once (see `effects/save.rs`).
    pub effects_to_restore: Vec<SavedEffect>,
    /// The buffs and reuse timers the persistence task writes with this store (Java `storeEffect`):
    /// the rows replace the character's `character_skills_save` rows. `None` leaves them alone
    /// (`StoreSkillCooltime` is off, or the snapshot carries no effects).
    pub effects_to_store: Option<Vec<SavedEffect>>,
    /// The `character_item_reuse_save` rows of the last logout (Java `restoreItemReuse` reads
    /// them). The world takes them at enter world (`inventory/handlers/reuse.rs`).
    pub item_reuse_to_restore: Vec<SavedItemReuse>,
    /// The item reuse timers the persistence task writes with this store (Java
    /// `storeItemReuseDelay`): the rows replace the character's `character_item_reuse_save` rows.
    /// `None` leaves them alone.
    pub item_reuse_to_store: Option<Vec<SavedItemReuse>>,
    /// Java `_classIndex`: the class index the saved buffs are stored under (0 for the base class).
    pub class_index: i32,
    /// The `character_summon_skills_save` rows of the character's active class index: the buffs
    /// its servitors had when they were last unsummoned (Java `Servitor.restoreEffects` reads them
    /// when a servitor of the same skill spawns). The world takes them at enter world
    /// (`summon/servitor.rs`).
    pub summon_effects: Vec<crate::summon::servitor::SavedSummonEffect>,
    /// The `pets` rows of the collars the character holds (Java `Pet.restore` reads one when its
    /// pet spawns). The world takes them at enter world (`summon/pet.rs`).
    pub pet_rows: Vec<crate::summon::pet::SavedPet>,
    /// The `character_pet_skills_save` rows of those collars (Java `Pet.restoreEffects`).
    pub pet_effects: Vec<crate::summon::pet::SavedPetEffect>,
    /// The clan columns of `characters` (`clanid`, `clan_privs`, `power_grade`, `subpledge`, ...)
    /// and the clan values of `Player`, see `clan/`. `None`: a snapshot that does not carry them
    /// (the store keeps the row's values); the world fills it when the player enters.
    pub clan: Option<crate::clan::PlayerClanState>,
    /// The character's clan as the session read it at `CharacterSelect` (`clanid` of the row),
    /// `None` without a clan or when the read failed. The world installs it when its clan table
    /// does not know the clan (one the Java game server made after this server started, D-39);
    /// it is taken at enter world, so snapshots never carry it.
    pub clan_record: Option<Box<crate::clan::ClanRecord>>,
    /// The private warehouse, the freight and the other characters of the account as the session
    /// read them (M6-WH-01). The world takes them when the player is created
    /// ([`Player::from_data`]), so snapshots never carry them.
    pub warehouse: crate::warehouse::WarehouseLoad,
    /// The friend list as the session read it (`character_friends` with relation 0, M6-FRIEND-01).
    /// The world takes it at enter world (`friends.rs`), so snapshots never carry it.
    pub friends: Vec<crate::friends::FriendEntry>,
    /// The block list as the session read it (`character_friends` with relation 1). Taken at
    /// enter world like `friends`.
    pub block_list: Vec<crate::friends::FriendEntry>,
    /// The character's mails as the session read them at `CharacterSelect` (M6-SYNC-01):
    /// `None` when the read failed. The world replaces its copies of these messages when the
    /// player enters (`mail/refresh.rs`) and takes it, so snapshots never carry it.
    pub mail_refresh: Option<Box<crate::mail::MailRefresh>>,
    /// The residences of the character's clan as the session read them at `CharacterSelect`
    /// (M7-RES-01, D-50, D-51): `None` without a clan or when the read failed. The world applies
    /// them when the player enters (`residence/refresh.rs`) and takes it, so snapshots never carry
    /// it.
    pub residence_refresh: Option<Box<crate::residence::ResidenceRefresh>>,
    /// The recipe book and the manufacture shop rows as the session read them (M6-CRAFT-01). The
    /// world takes them when the player is created ([`Player::from_data`]), so snapshots never
    /// carry them.
    pub recipes: crate::craft::RecipeLoad,
    /// The manufacture shop the persistence task writes with this store (Java
    /// `storeRecipeShopList`, only with `StoreRecipeShopList` and a shop): `(recipeId, price)` in
    /// the order of the shop. `None` leaves the rows alone.
    pub recipe_shop_to_store: Option<Vec<(i32, i64)>>,
    /// `characters.online` of the next store: 1, or 2 for a player without a client (offline
    /// trade, Java `isOnlineInt()`).
    pub online_status: i32,
    /// `characters.isin7sdungeon` (Java `_isIn7sDungeon`, M8-SS-01).
    pub in_7s_dungeon: bool,
    /// The character's `seven_signs` row and the status row as the session read them at
    /// `CharacterSelect` (M8-SS-01, D-50): `None` when the read failed. The world applies them
    /// when the player enters (`sevensigns/cycle.rs`) and takes it, so snapshots never carry it.
    pub seven_signs_refresh: Option<Box<crate::sevensigns::SevenSignsRefresh>>,
    /// The `seven_signs` row `Player.store` writes (`saveSevenSignsData(objectId)`); `None` for a
    /// character that never joined. Filled by the snapshot of a store.
    pub seven_signs_to_store: Option<crate::sevensigns::SignsPlayer>,
    /// `character_instance_time` (template id, time) as the session read it at `CharacterSelect`;
    /// the world takes them at enter (M8-INST-01, `instance/times.rs`).
    pub instance_times: Vec<(i32, i64)>,
}

impl Default for PlayerProfile {
    fn default() -> Self {
        Self {
            fame: 0,
            noble: false,
            title_color: DEFAULT_TITLE_COLOR,
            vitality_points: 0,
            pccafe_points: 0,
            bookmark_slot: 0,
            death_penalty_level: 0,
            exp_before_death: 0,
            recom_have: 0,
            recom_left: 0,
            recom_time_left_ms: 0,
            macros: Vec::new(),
            variables: BTreeMap::new(),
            bookmarks: Vec::new(),
            contacts: Vec::new(),
            hennas: [0; 3],
            last_access: 0,
            variables_changed: std::collections::BTreeSet::new(),
            variables_removed: std::collections::BTreeSet::new(),
            reco_store: false,
            effects_to_restore: Vec::new(),
            effects_to_store: None,
            item_reuse_to_restore: Vec::new(),
            item_reuse_to_store: None,
            class_index: 0,
            summon_effects: Vec::new(),
            pet_rows: Vec::new(),
            pet_effects: Vec::new(),
            clan: None,
            clan_record: None,
            warehouse: crate::warehouse::WarehouseLoad::default(),
            friends: Vec::new(),
            block_list: Vec::new(),
            mail_refresh: None,
            residence_refresh: None,
            recipes: crate::craft::RecipeLoad::default(),
            recipe_shop_to_store: None,
            online_status: 1,
            in_7s_dungeon: false,
            seven_signs_refresh: None,
            seven_signs_to_store: None,
            instance_times: Vec::new(),
        }
    }
}

/// A character as loaded from (and stored to) the database. The session task builds it before
/// `EnterWorld`; the world hands snapshots of it to the persistence task.
///
/// Field names follow the `characters` columns (Java `Player.restore` / `store`).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerData {
    pub object_id: ObjectId,
    pub account: String,
    pub name: String,
    pub title: String,
    pub class_id: i32,
    pub base_class: i32,
    pub race: i32,
    pub sex: i32,
    pub face: i32,
    pub hair_style: i32,
    pub hair_color: i32,
    pub level: i32,
    pub exp: i64,
    pub sp: i64,
    pub karma: i32,
    pub pvp_kills: i32,
    pub pk_kills: i32,
    pub access_level: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    pub cur_hp: f64,
    pub max_hp: f64,
    pub cur_mp: f64,
    pub max_mp: f64,
    pub cur_cp: f64,
    pub max_cp: f64,
    /// Total time online in seconds (`characters.onlinetime`).
    pub online_time: i64,
    pub items: Vec<ItemData>,
    pub skills: Vec<SkillData>,
    pub shortcuts: Vec<ShortcutData>,
    /// Values loaded for the enter-world packets that the persistence task never writes.
    pub profile: PlayerProfile,
}

/// A player inside the world.
#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    pub core: CreatureCore,
    pub data: PlayerData,
    /// World tick at which the player entered (for online time and admin views).
    pub entered_at_tick: u64,
    /// Unix time (ms) at which the player entered. Set by the world; only the admin view uses it.
    pub entered_at_unix_ms: i64,
    /// The client's address when the session entered (`None` if unknown).
    pub client_ip: Option<IpAddr>,
    /// Java `hasEnteredWorld()`: set once the enter-world packets went out. `UserInfo` tells the
    /// client it can equip a cloak only before that.
    pub entered_world: bool,
    /// Java `_recomHave` / `_recomLeft` and the milliseconds the Nevit hourglass has left; loaded
    /// from the profile when the player enters (`Player.checkRecoBonusTask`), 0 before that.
    pub recom_have: i32,
    pub recom_left: i32,
    pub hourglass_ms: i64,
    /// Java `getKnownRelations()`: the relation (mask, auto-attackable) each viewer was last told
    /// about this player, so `RelationChanged` only goes out when it changes.
    pub known_relations: std::collections::HashMap<ObjectId, (i32, bool)>,
    /// Java `_move`, the movement-related fields of `Creature`/`Player` and the move intention of
    /// `PlayerAI` (see `movement.rs`).
    pub movement: crate::movement::MovementState,
    /// The zones the player is inside and the flags they set (see `zones.rs`).
    pub zones: crate::zones::PlayerZones,
    /// Java `isTeleporting()` and the teleport watchdog (see `teleport.rs`).
    pub tele: crate::teleport::TeleportState,
    /// Java `getTarget()`: the object the player has selected (see `target.rs`).
    pub target: Option<ObjectId>,
    /// The dialogs the player was shown and the NPC it clicked last (see `dialog.rs`).
    pub talk: crate::dialog::PlayerTalk,
    /// Java `_inventoryUpdate` waiting for its 100 ms timer (see `item_ops.rs`).
    pub pending_inventory: crate::item_ops::PendingInventoryUpdate,
    /// Java `_curWeightPenalty` (`refreshOverloaded`).
    pub weight_penalty: i32,
    /// Java `_expertiseWeaponPenalty` / `_expertiseArmorPenalty` (`refreshExpertisePenalty`).
    pub expertise_weapon_penalty: i32,
    pub expertise_armor_penalty: i32,
    /// The delayed sends of the inventory code (see `inventory/notify.rs`).
    pub inv_tasks: crate::inventory::InventoryTasks,
    /// The trade list, the open request and the trade refusal (see `trade/`).
    pub trade: crate::trade::TradeState,
    /// Java `_transformation`, `_transformSkills`, the visible name and title, the inventory
    /// block of a transformation (see `transform/`).
    pub transform: crate::transform::TransformState,
    /// The enchant scroll and attribute stone in use (see `enchant/`).
    pub enchant: crate::enchant::EnchantState,
    /// The refund list and the inventory block of an open shop window (see `shop/`).
    pub shop: crate::shop::ShopState,
    /// The private warehouse, the freight and the active warehouse (see `warehouse/`).
    pub warehouse: crate::warehouse::PlayerWarehouse,
    /// Java `_multiSell`: the multisell list the player was last shown (see `multisell.rs`).
    pub multisell: Option<crate::multisell::PreparedList>,
    /// The recipe books, the manufacture shop and the crafting flag (see `craft/`).
    pub craft: crate::craft::PlayerCraft,
    /// Java `_fishing`, `_fish`, `_lure`, the bite task and `_fishCombat` (see `fishing/`).
    pub fishing: crate::fishing::PlayerFishing,
    /// Java `_mountType`, `_mountNpcId`, the food of the mount and its tasks (see `mount.rs`).
    pub mount: crate::mount::MountState,
    /// The GM list entry, silence and diet mode (see `admin/flags.rs`).
    pub gm: crate::admin::flags::GmFlags,
    /// Java `_cursedWeaponEquippedId`: the item id of the cursed weapon the player holds, 0 for
    /// none (see `cursed/`). Not stored: `checkPlayer` sets it when the character is loaded.
    pub cursed_weapon_equipped_id: i32,
    /// The macro revision, the published items (L-02, see `leftover.rs`).
    pub leftover: crate::leftover::LeftoverState,
}

impl Player {
    pub fn from_data(mut data: PlayerData, tick: u64) -> Self {
        // The warehouses are the player's own lists, not part of the snapshots.
        let warehouse = crate::warehouse::PlayerWarehouse::from_load(
            std::mem::take(&mut data.profile.warehouse),
            data.object_id,
        );
        let craft = crate::craft::PlayerCraft::from_load(std::mem::take(&mut data.profile.recipes));
        let core = CreatureCore {
            object_id: data.object_id,
            x: data.x,
            y: data.y,
            z: data.z,
            heading: data.heading,
            level: data.level,
            cur_hp: data.cur_hp,
            max_hp: data.max_hp,
            cur_mp: data.cur_mp,
            max_mp: data.max_mp,
            cur_cp: data.cur_cp,
            max_cp: data.max_cp,
            // The world builds the calculators when the player enters (`init_player_stats`).
            calculators: Calculators::default(),
            skills: Vec::new(),
        };
        let movement = crate::movement::MovementState::at(data.x, data.y, data.z);
        Self {
            core,
            data,
            entered_at_tick: tick,
            entered_at_unix_ms: 0,
            client_ip: None,
            entered_world: false,
            recom_have: 0,
            recom_left: 0,
            hourglass_ms: 0,
            known_relations: std::collections::HashMap::new(),
            movement,
            zones: crate::zones::PlayerZones::default(),
            tele: crate::teleport::TeleportState::default(),
            target: None,
            talk: crate::dialog::PlayerTalk::default(),
            pending_inventory: crate::item_ops::PendingInventoryUpdate::default(),
            weight_penalty: 0,
            expertise_weapon_penalty: 0,
            expertise_armor_penalty: 0,
            inv_tasks: crate::inventory::InventoryTasks::default(),
            trade: crate::trade::TradeState::default(),
            transform: crate::transform::TransformState::default(),
            enchant: crate::enchant::EnchantState::default(),
            shop: crate::shop::ShopState::default(),
            warehouse,
            multisell: None,
            craft,
            fishing: crate::fishing::PlayerFishing::default(),
            mount: crate::mount::MountState::default(),
            gm: crate::admin::flags::GmFlags::default(),
            cursed_weapon_equipped_id: 0,
            leftover: crate::leftover::LeftoverState::default(),
        }
    }

    /// The data to store, with the live values (position, hp…) copied back.
    pub fn snapshot(&self) -> PlayerData {
        let mut data = self.data.clone();
        let c = &self.core;
        data.x = c.x;
        data.y = c.y;
        data.z = c.z;
        data.heading = c.heading;
        data.level = c.level;
        data.cur_hp = c.cur_hp;
        data.cur_mp = c.cur_mp;
        data.cur_cp = c.cur_cp;
        // Java `storeCharBase` writes `getMaxHp()` ...: the stat engine's values.
        data.max_hp = c.max_hp;
        data.max_mp = c.max_mp;
        data.max_cp = c.max_cp;
        data
    }
}

/// One line of the admin "online players" list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnlinePlayer {
    pub object_id: ObjectId,
    pub account: String,
    pub name: String,
    pub level: i32,
    pub class_id: i32,
    /// Seconds since the player entered the world.
    pub online_secs: u64,
    /// Unix time (ms) at which the player entered the world.
    pub online_since_unix_ms: i64,
    /// The client's address (`None` when the session didn't report one).
    pub ip: Option<IpAddr>,
}
