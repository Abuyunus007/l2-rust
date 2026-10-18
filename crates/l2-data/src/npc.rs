//! NPC templates: everything an NPC is made of except what the M4/M5 cards need (skills, AI
//! skill lists, drops are kept raw).
//!
//! Java: `gameserver/data/xml/NpcData.java`, `entity/actor/templates/{NpcTemplate,CreatureTemplate}.java`,
//! `entity/actor/holders/npc/{MinionHolder,DropHolder,DropGroupHolder}.java`.
//! Data: `data/stats/npcs/*.xml` (and `custom/` when enabled), `xsd/npcs.xsd`.
//!
//! # How Java builds an NPC (and what is ported)
//! `NpcData` walks the children of every `<npc>` in document order and writes what it finds into a
//! `StatSet` (attribute values are read with `Integer.decode`, `Double.valueOf`,
//! `Boolean.valueOf`; an absent attribute is simply not stored). `NpcTemplate.set` then reads typed
//! values back with *its own* defaults, truncating doubles with `getInt` (`basePAtk="8.47"` is 8),
//! wrapping `level` to a byte and so on. The same `StatSet` flow is used here ([`stat_set`]), so
//! these quirks come for free. Enum values (`race`, `sex`, `ai type`, `attack type`) are exact
//! case; an unknown one fails the NPC.
//!
//! An NPC id that was loaded before is overwritten by the later definition (Java calls
//! `template.set(set)` on the existing object), so the count is the number of distinct ids.
//! Files are read in sorted path order; the `custom` folder comes after the main one, so a custom
//! definition wins.
//!
//! # Kept raw (for M4/M5)
//! * `skillList`: the `(id, level)` pairs of [`NpcTemplate::skills`], in document order. Java
//!   resolves them against `SkillData` (warning `skill not found`), builds the AI skill scope
//!   lists from the skills' effects and keeps the last entry per skill id; all of that waits for
//!   the skill tables.
//! * `dropLists`: [`NpcTemplate::drops`], [`NpcTemplate::spoils`] and [`NpcTemplate::drop_groups`],
//!   in document order. Java checks every item id against `ItemData` (warning
//!   `Could not find drop with id`), sorts the lists by chance (descending; group items ascending)
//!   and may add the configurable boss drops (`BossDropEnable`); none of that is done here.
//! * `parameters` ([`NpcTemplate::parameters`]): kept completely, including `<skill>` and
//!   `<minions>` entries; the teleport/html/dialog parameters of gatekeepers live here.
//! * AI: `aiType`, ranges, chaos/aggressive flags, `minSkillChance`/`maxSkillChance` and the clan
//!   sets are typed fields; the per-skill ids of `<ai><skill .../>` (`primaryId`, ...) are read
//!   (and must parse) but, as in Java, `NpcTemplate` does not keep them.
//!
//! # Not ported
//! * `loadNpcsSkillLearn` (teach info from `SkillLearnData`) and the `fakePlayer` look
//!   (`FakePlayerHolder`: class, hair, equipment...). With fake players disabled (Java's default)
//!   such NPCs are skipped like in Java; when enabled they load as plain templates with
//!   [`NpcTemplate::fake_player`] set.
//! * The custom `NpcStatMultipliersConfig` (default off) that scales hp/mp/atk/def/aggro per type.
//! * `hitTime`, `exCrtEffect`, `sNpcPropHpRate` (Java marks them `TODO: Implement me` and nothing
//!   reads them), `accuracy`, `evasion`, `reuseDelay` and `distance` of the stats are parsed and dropped.
//!
//! NOTE(java-quirk): element names are matched exactly as written in the XSD (`skillList`,
//! `corpseTime`, ...). Java lower-cases them, so it also accepts `<SKILLLIST>`; no shipped file
//! relies on that.
//!
//! NOTE(java-quirk): in Java any exception (a missing `ground` speed, a non-numeric attribute, an
//! unknown enum value) aborts the *rest of the file*, silently losing every later NPC in it. Here
//! only the broken NPC is skipped, with a warning. The shipped data has no such NPC.

mod raw;
mod stat_set;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use tracing::warn;

use crate::item::WeaponType;
use crate::loader::{DataError, load_dir, log_loaded};
use raw::{
    RawAi, RawAiChild, RawAttack, RawAttribute, RawAttributeChild, RawClanListChild, RawCollision,
    RawCollisionChild, RawDefence, RawDropItem, RawDropList, RawDropLists, RawDropListsChild, RawDropNode,
    RawEquipment, RawGroupNode, RawMinion, RawMinions, RawNpc, RawNpcChild, RawNpcFile, RawParameter,
    RawParameters, RawShots, RawSkillList, RawSkillListChild, RawSpeed, RawSpeedChild, RawStats,
    RawStatsChild, RawStatus, RawText, RawVitals,
};
use stat_set::{StatSet, decode_i32, decode_i64, parse_java_double};

macro_rules! java_enum {
    (
        $(#[$meta:meta])*
        $name:ident { $( $(#[$vmeta:meta])* $variant:ident = $java:literal ),+ $(,)? }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $( $(#[$vmeta])* $variant ),+
        }

        impl $name {
            /// Every variant, in Java's declaration (ordinal) order.
            pub const ALL: &'static [$name] = &[ $( $name::$variant ),+ ];

            /// The Java constant name, e.g. `DARK_ELF`.
            pub fn name(self) -> &'static str {
                match self { $( $name::$variant => $java ),+ }
            }

            /// Java `valueOf(name)`: exact and case sensitive; `None` where Java throws.
            pub fn from_name(name: &str) -> Option<Self> {
                match name { $( $java => Some($name::$variant), )+ _ => None }
            }

            /// Java `ordinal()`.
            pub fn ordinal(self) -> usize {
                self as usize
            }
        }
    };
}

java_enum! {
    /// Java `entity/actor/enums/creature/Race` (the NPC races; the player races are only the first six).
    NpcRace {
        /// `HUMAN`
        Human = "HUMAN",
        /// `ELF`
        Elf = "ELF",
        /// `DARK_ELF`
        DarkElf = "DARK_ELF",
        /// `ORC`
        Orc = "ORC",
        /// `DWARF`
        Dwarf = "DWARF",
        /// `KAMAEL`
        Kamael = "KAMAEL",
        /// `ANIMAL`
        Animal = "ANIMAL",
        /// `BEAST`
        Beast = "BEAST",
        /// `BUG`
        Bug = "BUG",
        /// `CASTLE_GUARD`
        CastleGuard = "CASTLE_GUARD",
        /// `CONSTRUCT`
        Construct = "CONSTRUCT",
        /// `DEMONIC`
        Demonic = "DEMONIC",
        /// `DIVINE`
        Divine = "DIVINE",
        /// `DRAGON`
        Dragon = "DRAGON",
        /// `ELEMENTAL`
        Elemental = "ELEMENTAL",
        /// `ETC`
        Etc = "ETC",
        /// `FAIRY`
        Fairy = "FAIRY",
        /// `GIANT`
        Giant = "GIANT",
        /// `HUMANOID`
        Humanoid = "HUMANOID",
        /// `MERCENARY`
        Mercenary = "MERCENARY",
        /// `NONE`
        None = "NONE",
        /// `PLANT`
        Plant = "PLANT",
        /// `SIEGE_WEAPON`
        SiegeWeapon = "SIEGE_WEAPON",
        /// `UNDEAD`
        Undead = "UNDEAD",
    }
}

java_enum! {
    /// Java `entity/actor/enums/player/Sex`.
    NpcSex {
        /// `MALE`
        Male = "MALE",
        /// `FEMALE`
        Female = "FEMALE",
        /// `ETC`
        Etc = "ETC",
    }
}

java_enum! {
    /// Java `entity/actor/enums/npc/AIType`.
    AiType {
        /// `FIGHTER`
        Fighter = "FIGHTER",
        /// `ARCHER`
        Archer = "ARCHER",
        /// `BALANCED`
        Balanced = "BALANCED",
        /// `MAGE`
        Mage = "MAGE",
        /// `HEALER`
        Healer = "HEALER",
        /// `CORPSE`
        Corpse = "CORPSE",
    }
}

java_enum! {
    /// Java `entity/actor/enums/npc/DropType`: which list a drop belongs to.
    DropType {
        /// `DROP`: dropped on death.
        Drop = "DROP",
        /// `SPOIL`: taken with the Spoil skill.
        Spoil = "SPOIL",
    }
}

/// What the server config contributes to loading NPCs. The defaults are Java's *code* defaults
/// (not the values of the shipped `.ini` files), like [`crate::item::ItemTemplates::load`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcLoadOptions {
    /// `General.ini` `CustomNpcData` (default false): also load `data/stats/npcs/custom`.
    pub custom_npc_data: bool,
    /// `Custom/FakePlayers.ini` `EnableFakePlayers` (default false): NPCs with a `<fakePlayer>`
    /// element are skipped unless this is true.
    pub fake_players_enabled: bool,
    /// `NPC.ini` `MaxAggroRange` (default 450): the aggro range of a `Monster` is capped to it.
    pub max_aggro_range: i32,
    /// `NPC.ini` `DefaultCorpseTime` (default 7): the corpse time of NPCs without `<corpseTime>`.
    pub default_corpse_time: i32,
}

impl Default for NpcLoadOptions {
    fn default() -> Self {
        Self {
            custom_npc_data: false,
            fake_players_enabled: false,
            max_aggro_range: 450,
            default_corpse_time: 7,
        }
    }
}

/// Java `MinionHolder`: one `<npc>` of a `<minions>` parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinionDef {
    /// Minion NPC id.
    pub id: i32,
    /// Fixed count, or the lower bound when `max` is bigger.
    pub count: i32,
    /// Upper bound of a random count (0 when absent). Java picks `Rnd.get(count, max)` when `max > count`.
    pub max: i32,
    /// Respawn time in seconds.
    pub respawn_time: i64,
    /// `weightPoint`, default 0.
    pub weight_point: i32,
}

/// One entry of `<parameters>` (Java stores `String`, `SkillHolder` or `List<MinionHolder>` objects).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NpcParam {
    /// `<param name=".." value=".."/>`.
    Text(String),
    /// `<skill name=".." id=".." level=".."/>` (Java `SkillHolder`).
    Skill {
        /// Skill id.
        id: i32,
        /// Skill level.
        level: i32,
    },
    /// `<minions name="..">` with at least one `<npc>`.
    Minions(Vec<MinionDef>),
}

/// An `(id, level)` pair of `<skillList>`, not yet resolved against the skill tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcSkillRef {
    /// Skill id.
    pub id: i32,
    /// Skill level.
    pub level: i32,
}

/// Java `DropHolder`, unchecked: the item id is not validated against the item table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropEntry {
    /// Which list this entry was written in.
    pub drop_type: DropType,
    /// Item id.
    pub item_id: i32,
    /// Minimum count.
    pub min: i64,
    /// Maximum count.
    pub max: i64,
    /// Chance in percent.
    pub chance: f64,
}

/// Java `DropGroupHolder`, with its items in document order.
#[derive(Debug, Clone, PartialEq)]
pub struct DropGroup {
    /// Chance of the group in percent.
    pub chance: f64,
    /// The group's items.
    pub items: Vec<DropEntry>,
}

/// Java `NpcTemplate` (with the `CreatureTemplate` fields it inherits).
#[derive(Debug, Clone, PartialEq)]
pub struct NpcTemplate {
    // ---- identity ----
    /// `id`.
    pub id: i32,
    /// `displayId`: the template id shown to the client; the NPC id when absent.
    pub display_id: i32,
    /// `level` (Java `byte`, default 85 when the attribute is missing).
    pub level: i8,
    /// `type`: Java's instance class name (`Folk`, `Monster`, `RaidBoss`, `Merchant`...), default `Folk`.
    /// Kept as text like Java does; compare with [`NpcTemplate::is_type`].
    pub npc_type: String,
    /// `name` (empty when absent).
    pub name: String,
    /// `usingServerSideName`.
    pub using_server_side_name: bool,
    /// `title` (empty when absent).
    pub title: String,
    /// `usingServerSideTitle`.
    pub using_server_side_title: bool,
    /// `<race>`, default `NONE`.
    pub race: NpcRace,
    /// `<sex>`, default `ETC`.
    pub sex: NpcSex,
    /// `<parameters>`; the key is the parameter name.
    pub parameters: BTreeMap<String, NpcParam>,

    // ---- equipment and rewards ----
    /// `<equipment chest>` item id.
    pub chest_id: i32,
    /// `<equipment rhand>` item id (the weapon shown in the right hand).
    pub rhand_id: i32,
    /// `<equipment lhand>` item id.
    pub lhand_id: i32,
    /// `<equipment weaponEnchant>`.
    pub weapon_enchant: i32,
    /// `<acquire exp>`.
    pub exp: f64,
    /// `<acquire sp>`.
    pub sp: f64,
    /// `<acquire raidPoints>`.
    pub raid_points: f64,

    // ---- status flags ----
    /// `<status unique>`.
    pub unique: bool,
    /// `<status attackable>`, default true.
    pub attackable: bool,
    /// `<status targetable>`, default true.
    pub targetable: bool,
    /// `<status talkable>`, default true.
    pub talkable: bool,
    /// Java `isQuestMonster()`: the title contains `Quest`.
    pub quest_monster: bool,
    /// `<status undying>`; defaults to true unless the type is `Monster`, `RaidBoss` or `GrandBoss`.
    pub undying: bool,
    /// `<status showName>`, default true.
    pub show_name: bool,
    /// `<status randomWalk>`; defaults to true unless the type is `Guard`. See [`NpcTemplate::is_random_walk_enabled`].
    pub random_walk: bool,
    /// `<status randomAnimation>`, default true.
    pub random_animation: bool,
    /// `<status flying>`.
    pub flying: bool,
    /// True when the NPC has a `<fakePlayer>` element (only loaded when fake players are enabled).
    pub fake_player: bool,
    /// Java `canMove()`: `<status canMove>` (default true), or true when the ground walk speed is 0.1.
    pub can_move: bool,
    /// `<status noSleepMode>`.
    pub no_sleep_mode: bool,
    /// `<status passableDoor>`.
    pub passable_door: bool,
    /// `<status hasSummoner>`.
    pub has_summoner: bool,
    /// `<status canBeSown>`.
    pub can_be_sown: bool,
    /// `<corpseTime>` in seconds, else the configured default.
    pub corpse_time: i32,

    // ---- AI (raw) ----
    /// `<ai type>`, default `FIGHTER`.
    pub ai_type: AiType,
    /// `<ai aggroRange>`; capped to the configured maximum for type `Monster`.
    pub aggro_range: i32,
    /// `<ai clanHelpRange>`.
    pub clan_help_range: i32,
    /// `<ai isChaos>`.
    pub is_chaos: bool,
    /// `<ai isAggressive>`, default true.
    pub is_aggressive: bool,
    /// `<shots soul>`.
    pub soul_shot: i32,
    /// `<shots spirit>`.
    pub spirit_shot: i32,
    /// `<shots shotChance>`.
    pub soul_shot_chance: i32,
    /// `<shots spiritChance>`.
    pub spirit_shot_chance: i32,
    /// `<ai><skill minChance>`, default 7.
    pub min_skill_chance: i32,
    /// `<ai><skill maxChance>`, default 15.
    pub max_skill_chance: i32,
    /// Ids of the clans this NPC belongs to (see [`NpcTemplates::clan_id`]); empty when it has none.
    pub clans: BTreeSet<i32>,
    /// `<ignoreNpcId>` entries of the clan list.
    pub ignore_clan_npc_ids: BTreeSet<i32>,

    // ---- skills and drops (raw) ----
    /// `<skillList>`, unresolved.
    pub skills: Vec<NpcSkillRef>,
    /// `<dropLists><drop>` items outside groups.
    pub drops: Vec<DropEntry>,
    /// `<dropLists><spoil>` items.
    pub spoils: Vec<DropEntry>,
    /// `<dropLists>` groups (their items keep the type of the list they were written in).
    pub drop_groups: Vec<DropGroup>,

    // ---- base stats (CreatureTemplate) ----
    /// `<stats str>`.
    pub base_str: i32,
    /// `<stats con>`.
    pub base_con: i32,
    /// `<stats dex>`.
    pub base_dex: i32,
    /// `<stats int>`.
    pub base_int: i32,
    /// `<stats wit>`.
    pub base_wit: i32,
    /// `<stats men>`.
    pub base_men: i32,
    /// `<vitals hp>`.
    pub base_hp_max: f32,
    /// `<vitals mp>`.
    pub base_mp_max: f32,
    /// `<vitals hpRegen>`.
    pub base_hp_reg: f32,
    /// `<vitals mpRegen>`.
    pub base_mp_reg: f32,
    /// `<attack physical>`, truncated to an integer like Java.
    pub base_p_atk: i32,
    /// `<attack magical>`, truncated.
    pub base_m_atk: i32,
    /// `<defence physical>`, truncated.
    pub base_p_def: i32,
    /// `<defence magical>`, truncated.
    pub base_m_def: i32,
    /// `<attack attackSpeed>`, default 300.
    pub base_p_atk_spd: i32,
    /// Java default 333 (NpcData never sets it).
    pub base_m_atk_spd: i32,
    /// `<defence shield>`.
    pub base_shld_def: i32,
    /// `<defence shieldRate>`.
    pub base_shld_rate: i32,
    /// `<attack range>`, default 40.
    pub base_attack_range: i32,
    /// `<attack width>`: the attack angle, default 120 (Java `getBaseAttackAngle`).
    pub base_attack_angle: i32,
    /// `<attack random>`.
    pub random_damage: i32,
    /// `<attack type>`, default `FIST`.
    pub base_attack_type: WeaponType,
    /// `<attack critical>`, default 4.
    pub base_crit_rate: i32,
    /// Java default 5 (NpcData never sets it).
    pub base_mcrit_rate: i32,
    /// Java default 100 (NpcData never sets it).
    pub base_breath: i32,
    /// `<attribute><attack type="FIRE">`.
    pub base_fire: i32,
    /// `<attribute><attack type="WIND">`.
    pub base_wind: i32,
    /// `<attribute><attack type="WATER">`.
    pub base_water: i32,
    /// `<attribute><attack type="EARTH">`.
    pub base_earth: i32,
    /// `<attribute><attack type="HOLY">`.
    pub base_holy: i32,
    /// `<attribute><attack type="DARK">`.
    pub base_dark: i32,
    /// `<attribute><defence fire>`.
    pub base_fire_res: f64,
    /// `<attribute><defence wind>`.
    pub base_wind_res: f64,
    /// `<attribute><defence water>`.
    pub base_water_res: f64,
    /// `<attribute><defence earth>`.
    pub base_earth_res: f64,
    /// `<attribute><defence holy>`.
    pub base_holy_res: f64,
    /// `<attribute><defence dark>`.
    pub base_dark_res: f64,
    /// `<attribute><defence default>`.
    pub base_element_res: f64,
    /// `<collision><radius normal>` as sent to the client (Java `getFCollisionRadius`).
    pub collision_radius: f32,
    /// `<collision><height normal>` as sent to the client.
    pub collision_height: f32,
    /// `<collision><radius grown>`.
    pub collision_radius_grown: f64,
    /// `<collision><height grown>`.
    pub collision_height_grown: f64,
    /// `<speed><run ground>`, default 120 (0.1 when the XML has 0 or less).
    pub run_speed: f64,
    /// `<speed><walk ground>`, default 50 (0.1 when the XML has 0 or less).
    pub walk_speed: f64,
    /// `<run swim>`, defaults to the run speed.
    pub swim_run_speed: f64,
    /// `<walk swim>`, defaults to the walk speed.
    pub swim_walk_speed: f64,
    /// `<run fly>`, defaults to the run speed.
    pub fly_run_speed: f64,
    /// `<walk fly>`, defaults to the walk speed.
    pub fly_walk_speed: f64,
}

impl NpcTemplate {
    /// Java `isType(type)`: the instance class name, ignoring case.
    pub fn is_type(&self, npc_type: &str) -> bool {
        self.npc_type.eq_ignore_ascii_case(npc_type)
    }

    /// Java `getCollisionRadius()`: the radius cut to an integer.
    pub fn collision_radius_int(&self) -> i32 {
        self.collision_radius as i32
    }

    /// Java `getCollisionHeight()`: the height cut to an integer.
    pub fn collision_height_int(&self) -> i32 {
        self.collision_height as i32
    }

    /// A `<param>` value, if the parameter exists and is plain text.
    pub fn param_str(&self, name: &str) -> Option<&str> {
        match self.parameters.get(name) {
            Some(NpcParam::Text(text)) => Some(text),
            _ => None,
        }
    }

    /// Java `getParameters().getInt(name)` for a plain-text parameter; `None` when it is missing.
    /// (Java throws when the text is not an integer; that also gives `None` here.)
    pub fn param_int(&self, name: &str) -> Option<i32> {
        self.param_str(name)?.parse().ok()
    }

    /// The `<minions>` list stored under `name` (Java `getMinionList`), empty when there is none.
    pub fn minion_list(&self, name: &str) -> &[MinionDef] {
        match self.parameters.get(name) {
            Some(NpcParam::Minions(list)) => list,
            _ => &[],
        }
    }

    /// Java `isRandomWalkEnabled()`: the flag and the `MoveArounding` parameter (default 1).
    pub fn is_random_walk_enabled(&self) -> bool {
        self.random_walk && self.param_int("MoveArounding").unwrap_or(1) != 0
    }

    /// Reads a finished `StatSet` into a template, like `NpcTemplate.set`.
    fn from_set(set: &StatSet, parts: Parts, options: &NpcLoadOptions) -> Result<Self, DataError> {
        let id = set.get_int("id", 0)?;
        let npc_type = set.get_string("type", "Folk");
        let title = set.get_string("title", "");
        let is_monster_group = matches!(npc_type.as_str(), "Monster" | "RaidBoss" | "GrandBoss");

        let collision_height = set.get_float("collisionHeight", 0.0)?;
        let collision_radius = set.get_float("collisionRadius", 0.0)?;

        // Speeds: swim and fly fall back to the (already defaulted) run/walk speeds.
        let run_speed = set.get_double("baseRunSpd", 120.0)?;
        let walk_speed = set.get_double("baseWalkSpd", 50.0)?;

        // L2J aggro range tempfix.
        let aggro_value = set.get_int("aggroRange", 0)?;
        let aggro_range = if npc_type.eq_ignore_ascii_case("Monster") && aggro_value > options.max_aggro_range
        {
            options.max_aggro_range
        } else {
            aggro_value
        };

        Ok(Self {
            id,
            display_id: set.get_int("displayId", id)?,
            level: set.get_byte("level", 70)?,
            name: set.get_string("name", ""),
            using_server_side_name: set.get_bool("usingServerSideName", false),
            using_server_side_title: set.get_bool("usingServerSideTitle", false),
            race: set.get_enum("race", NpcRace::None, NpcRace::from_name)?,
            sex: set.get_enum("sex", NpcSex::Etc, NpcSex::from_name)?,
            parameters: parts.parameters,
            chest_id: set.get_int("chestId", 0)?,
            rhand_id: set.get_int("rhandId", 0)?,
            lhand_id: set.get_int("lhandId", 0)?,
            weapon_enchant: set.get_int("weaponEnchant", 0)?,
            exp: set.get_double("exp", 0.0)?,
            sp: set.get_double("sp", 0.0)?,
            raid_points: set.get_double("raidPoints", 0.0)?,
            unique: set.get_bool("unique", false),
            attackable: set.get_bool("attackable", true),
            targetable: set.get_bool("targetable", true),
            talkable: set.get_bool("talkable", true),
            quest_monster: title.contains("Quest"),
            undying: set.get_bool("undying", !is_monster_group),
            show_name: set.get_bool("showName", true),
            random_walk: set.get_bool("randomWalk", npc_type != "Guard"),
            random_animation: set.get_bool("randomAnimation", true),
            flying: set.get_bool("flying", false),
            fake_player: set.get_bool("fakePlayer", false),
            can_move: set.get_double("baseWalkSpd", 1.0)? <= 0.1 || set.get_bool("canMove", true),
            no_sleep_mode: set.get_bool("noSleepMode", false),
            passable_door: set.get_bool("passableDoor", false),
            has_summoner: set.get_bool("hasSummoner", false),
            can_be_sown: set.get_bool("canBeSown", false),
            corpse_time: set.get_int("corpseTime", options.default_corpse_time)?,
            ai_type: set.get_enum("aiType", AiType::Fighter, AiType::from_name)?,
            aggro_range,
            clan_help_range: set.get_int("clanHelpRange", 0)?,
            is_chaos: set.get_bool("isChaos", false),
            is_aggressive: set.get_bool("isAggressive", true),
            soul_shot: set.get_int("soulShot", 0)?,
            spirit_shot: set.get_int("spiritShot", 0)?,
            soul_shot_chance: set.get_int("soulShotChance", 0)?,
            spirit_shot_chance: set.get_int("spiritShotChance", 0)?,
            min_skill_chance: set.get_int("minSkillChance", 7)?,
            max_skill_chance: set.get_int("maxSkillChance", 15)?,
            clans: parts.clans,
            ignore_clan_npc_ids: parts.ignore_clan_npc_ids,
            skills: parts.skills,
            drops: parts.drops,
            spoils: parts.spoils,
            drop_groups: parts.drop_groups,
            base_str: set.get_int("baseSTR", 0)?,
            base_con: set.get_int("baseCON", 0)?,
            base_dex: set.get_int("baseDEX", 0)?,
            base_int: set.get_int("baseINT", 0)?,
            base_wit: set.get_int("baseWIT", 0)?,
            base_men: set.get_int("baseMEN", 0)?,
            base_hp_max: set.get_float("baseHpMax", 0.0)?,
            base_mp_max: set.get_float("baseMpMax", 0.0)?,
            base_hp_reg: set.get_float("baseHpReg", 0.0)?,
            base_mp_reg: set.get_float("baseMpReg", 0.0)?,
            base_p_atk: set.get_int("basePAtk", 0)?,
            base_m_atk: set.get_int("baseMAtk", 0)?,
            base_p_def: set.get_int("basePDef", 0)?,
            base_m_def: set.get_int("baseMDef", 0)?,
            base_p_atk_spd: set.get_int("basePAtkSpd", 300)?,
            base_m_atk_spd: set.get_int("baseMAtkSpd", 333)?,
            base_shld_def: set.get_int("baseShldDef", 0)?,
            base_shld_rate: set.get_int("baseShldRate", 0)?,
            base_attack_range: set.get_int("baseAtkRange", 40)?,
            base_attack_angle: set.get_int("width", 120)?,
            random_damage: set.get_int("baseRndDam", 0)?,
            base_attack_type: set.get_enum("baseAtkType", WeaponType::Fist, WeaponType::from_name)?,
            base_crit_rate: set.get_int("baseCritRate", 4)?,
            base_mcrit_rate: set.get_int("baseMCritRate", 5)?,
            base_breath: set.get_int("baseBreath", 100)?,
            base_fire: set.get_int("baseFire", 0)?,
            base_wind: set.get_int("baseWind", 0)?,
            base_water: set.get_int("baseWater", 0)?,
            base_earth: set.get_int("baseEarth", 0)?,
            base_holy: set.get_int("baseHoly", 0)?,
            base_dark: set.get_int("baseDark", 0)?,
            base_fire_res: f64::from(set.get_int("baseFireRes", 0)?),
            base_wind_res: f64::from(set.get_int("baseWindRes", 0)?),
            base_water_res: f64::from(set.get_int("baseWaterRes", 0)?),
            base_earth_res: f64::from(set.get_int("baseEarthRes", 0)?),
            base_holy_res: f64::from(set.get_int("baseHolyRes", 0)?),
            base_dark_res: f64::from(set.get_int("baseDarkRes", 0)?),
            base_element_res: f64::from(set.get_int("baseElementRes", 0)?),
            collision_radius,
            collision_height,
            collision_radius_grown: set.get_double("collisionRadiusGrown", 0.0)?,
            collision_height_grown: set.get_double("collisionHeightGrown", 0.0)?,
            run_speed,
            walk_speed,
            swim_run_speed: set.get_double("baseSwimRunSpd", run_speed)?,
            swim_walk_speed: set.get_double("baseSwimWalkSpd", walk_speed)?,
            fly_run_speed: set.get_double("baseFlyRunSpd", run_speed)?,
            fly_walk_speed: set.get_double("baseFlyWalkSpd", walk_speed)?,
            npc_type,
            title,
        })
    }
}

/// All NPC templates, immutable after loading. Java `NpcData`.
#[derive(Debug, Default)]
pub struct NpcTemplates {
    npcs: BTreeMap<i32, NpcTemplate>,
    /// Java `_clans`: clan name to the id handed out in first-seen order.
    clans: BTreeMap<String, i32>,
    /// Java `_masterMonsterIDs`.
    master_monsters: BTreeSet<i32>,
}

impl NpcTemplates {
    const LOADER: &'static str = "NpcData";

    /// Loads every file in `<root>/data/stats/npcs` (not its subfolders) and logs Java's count
    /// line. With [`NpcLoadOptions::custom_npc_data`] the files below `data/stats/npcs/custom`
    /// (subfolders included) are loaded after them, with Java's second count line.
    pub fn load(root: &Path, options: &NpcLoadOptions) -> Self {
        let mut table = Self::default();
        load_dir(&root.join("data/stats/npcs"), false, |doc: RawNpcFile| {
            table.add_file(doc, options);
            Ok(())
        });
        log_loaded(Self::LOADER, table.len(), "NPCs");
        if options.custom_npc_data {
            let before = table.len();
            load_dir(&root.join("data/stats/npcs/custom"), true, |doc: RawNpcFile| {
                table.add_file(doc, options);
                Ok(())
            });
            log_loaded(Self::LOADER, table.len() - before, "custom NPCs");
        }
        table
    }

    /// Builds a table from the text of one NPC file. For tests and tools.
    pub fn from_xml(xml: &str, options: &NpcLoadOptions) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add_file(doc, options);
        Ok(table)
    }

    /// Java `NpcData.parseDocument` for one file.
    fn add_file(&mut self, doc: RawNpcFile, options: &NpcLoadOptions) {
        for raw in doc.npcs {
            let label = raw.id.clone().unwrap_or_default();
            match self.build(raw, options) {
                Ok(Some(template)) => {
                    // Java: a later definition of the same id is applied to the existing template.
                    self.npcs.insert(template.id, template);
                }
                Ok(None) => {}
                Err(error) => warn!("{}: cannot create NPC {label}: {error}", Self::LOADER),
            }
        }
    }

    /// Java `getOrCreateClanId`.
    fn clan_id_or_create(&mut self, name: &str) -> i32 {
        if let Some(id) = self.clans.get(name) {
            return *id;
        }
        let id = i32::try_from(self.clans.len()).unwrap_or(i32::MAX);
        self.clans.insert(name.to_owned(), id);
        id
    }

    /// Java `getClanId(name)`: -1 for a clan no NPC mentions.
    pub fn clan_id(&self, name: &str) -> i32 {
        self.clans.get(name).copied().unwrap_or(-1)
    }

    /// Java `getGenericClanId()`: the id of the clan called `ALL`, -1 when there is none.
    pub fn generic_clan_id(&self) -> i32 {
        self.clan_id("ALL")
    }

    /// Java `getTemplate(id)`: `None` for an unknown id.
    pub fn get(&self, id: i32) -> Option<&NpcTemplate> {
        self.npcs.get(&id)
    }

    /// All templates, ordered by id.
    pub fn iter(&self) -> impl Iterator<Item = &NpcTemplate> {
        self.npcs.values()
    }

    /// Number of distinct NPC ids.
    pub fn len(&self) -> usize {
        self.npcs.len()
    }

    /// True when nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.npcs.is_empty()
    }

    /// Java `getTemplateByName(name)`: the first template (lowest id; Java's map order is
    /// unspecified) whose name equals `name` ignoring case.
    pub fn get_by_name(&self, name: &str) -> Option<&NpcTemplate> {
        self.iter().find(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// Java `getAllOfLevel(levels)`.
    pub fn all_of_level<'a>(&'a self, levels: &'a [i32]) -> impl Iterator<Item = &'a NpcTemplate> {
        self.iter().filter(move |t| levels.contains(&i32::from(t.level)))
    }

    /// Java `getAllMonstersOfLevel(levels)`.
    pub fn all_monsters_of_level<'a>(&'a self, levels: &'a [i32]) -> impl Iterator<Item = &'a NpcTemplate> {
        self.all_of_level(levels).filter(|t| t.is_type("Monster"))
    }

    /// Java `getAllNpcStartingWith(text)`: `Folk` NPCs whose name starts with `text`.
    pub fn all_npc_starting_with<'a>(&'a self, text: &'a str) -> impl Iterator<Item = &'a NpcTemplate> {
        self.iter()
            .filter(move |t| t.is_type("Folk") && t.name.starts_with(text))
    }

    /// Java `getAllNpcOfClassType(types)` (compared ignoring case).
    pub fn all_of_class_type<'a>(&'a self, types: &'a [&str]) -> impl Iterator<Item = &'a NpcTemplate> {
        self.iter()
            .filter(move |t| types.iter().any(|ty| ty.eq_ignore_ascii_case(&t.npc_type)))
    }

    /// Java `getMasterMonsterIDs()`: monsters with a `Privates` minion list and no `SummonPrivateRate`.
    pub fn master_monster_ids(&self) -> &BTreeSet<i32> {
        &self.master_monsters
    }

    /// Java `NpcData.parseDocument` for one `<npc>`; `Ok(None)` for a skipped fake player.
    fn build(&mut self, raw: RawNpc, options: &NpcLoadOptions) -> Result<Option<NpcTemplate>, DataError> {
        // `final int npcId = parseInteger(attrs, "id")`: a missing id is a NullPointerException.
        let npc_id = attr_int(&raw.id)?.ok_or_else(|| missing("id"))?;
        let level = attr_int(&raw.level)?.unwrap_or(85);
        let kind = raw.kind.clone().unwrap_or_else(|| "Folk".to_owned());

        let mut set = StatSet::default();
        set.set_int("id", Some(npc_id));
        set.set_int("displayId", attr_int(&raw.display_id)?);
        set.set_int("level", Some(level));
        set.set_text("type", Some(&kind));
        set.set_text("name", raw.name.as_deref());
        set.set_bool("usingServerSideName", attr_bool(&raw.using_server_side_name));
        set.set_text("title", raw.title.as_deref());
        set.set_bool("usingServerSideTitle", attr_bool(&raw.using_server_side_title));

        let mut parts = Parts::default();
        for child in &raw.children {
            match child {
                RawNpcChild::Parameters(parameters) => read_parameters(parameters, &mut parts.parameters)?,
                // `getTextContent().toUpperCase()`, not trimmed: whitespace around the name fails `valueOf`.
                RawNpcChild::Race(text) => set.set_text("race", Some(&upper(text))),
                RawNpcChild::Sex(text) => set.set_text("sex", Some(&upper(text))),
                RawNpcChild::Equipment(equipment) => read_equipment(equipment, &mut set)?,
                RawNpcChild::Acquire(acquire) => {
                    set.set_double("exp", attr_double(&acquire.exp)?);
                    set.set_double("sp", attr_double(&acquire.sp)?);
                    set.set_double("raidPoints", attr_double(&acquire.raid_points)?);
                }
                RawNpcChild::Stats(stats) => read_stats(stats, &mut set)?,
                RawNpcChild::Status(status) => read_status(status, &mut set),
                RawNpcChild::FakePlayer(_) => set.set_bool("fakePlayer", Some(true)),
                RawNpcChild::SkillList(list) => parts.skills = read_skill_list(list)?,
                RawNpcChild::Shots(shots) => read_shots(shots, &mut set)?,
                RawNpcChild::CorpseTime(text) => {
                    set.set_text("corpseTime", Some(text.text.as_deref().unwrap_or("")));
                }
                RawNpcChild::Ai(ai) => self.read_ai(ai, &mut set, &mut parts)?,
                RawNpcChild::DropLists(lists) => read_drop_lists(lists, npc_id, &mut parts),
                RawNpcChild::Collision(collision) => read_collision(collision, &mut set)?,
                RawNpcChild::Other => {}
            }
        }

        if !options.fake_players_enabled && set.get_bool("fakePlayer", false) {
            return Ok(None);
        }

        let template = NpcTemplate::from_set(&set, parts, options)?;
        if !template.minion_list("Privates").is_empty()
            && !template.parameters.contains_key("SummonPrivateRate")
        {
            self.master_monsters.insert(template.id);
        }
        Ok(Some(template))
    }

    /// Java `case "ai"`.
    fn read_ai(&mut self, ai: &RawAi, set: &mut StatSet, parts: &mut Parts) -> Result<(), DataError> {
        set.set_text("aiType", ai.kind.as_deref());
        set.set_int("aggroRange", attr_int(&ai.aggro_range)?);
        set.set_int("clanHelpRange", attr_int(&ai.clan_help_range)?);
        set.set_bool("isChaos", attr_bool(&ai.is_chaos));
        set.set_bool("isAggressive", attr_bool(&ai.is_aggressive));
        for child in &ai.children {
            match child {
                RawAiChild::Skill(skill) => {
                    set.set_int("minSkillChance", attr_int(&skill.min_chance)?);
                    set.set_int("maxSkillChance", attr_int(&skill.max_chance)?);
                    set.set_int("primarySkillId", attr_int(&skill.primary_id)?);
                    set.set_int("shortRangeSkillId", attr_int(&skill.short_range_id)?);
                    set.set_int("shortRangeSkillChance", attr_int(&skill.short_range_chance)?);
                    set.set_int("longRangeSkillId", attr_int(&skill.long_range_id)?);
                    set.set_int("longRangeSkillChance", attr_int(&skill.long_range_chance)?);
                }
                RawAiChild::ClanList(list) => {
                    for entry in &list.children {
                        match entry {
                            RawClanListChild::Clan(text) => {
                                let id = self.clan_id_or_create(text.text.as_deref().unwrap_or(""));
                                parts.clans.insert(id);
                            }
                            RawClanListChild::IgnoreNpcId(text) => {
                                // `Integer.parseInt` of the raw text.
                                let value = text.text.as_deref().unwrap_or("");
                                let id = value.parse().map_err(|_| {
                                    DataError::Invalid(format!("ignoreNpcId is not an integer: {value}"))
                                })?;
                                parts.ignore_clan_npc_ids.insert(id);
                            }
                            RawClanListChild::Other => {}
                        }
                    }
                }
                RawAiChild::Other => {}
            }
        }
        Ok(())
    }
}

/// What the loader collects besides the `StatSet`.
#[derive(Default)]
struct Parts {
    parameters: BTreeMap<String, NpcParam>,
    skills: Vec<NpcSkillRef>,
    clans: BTreeSet<i32>,
    ignore_clan_npc_ids: BTreeSet<i32>,
    drops: Vec<DropEntry>,
    spoils: Vec<DropEntry>,
    drop_groups: Vec<DropGroup>,
}

// ---- reading attributes the way IXmlReader does ----

fn upper(text: &RawText) -> String {
    text.text.as_deref().unwrap_or("").to_uppercase()
}

fn missing(what: &str) -> DataError {
    // Java unboxes a null `Integer`/`Double` and throws a NullPointerException.
    DataError::Invalid(format!("required attribute {what} is missing"))
}

/// `parseInteger(attrs, name)`: `Integer.decode`, `None` for an absent attribute.
fn attr_int(attr: &Option<String>) -> Result<Option<i32>, DataError> {
    attr.as_deref()
        .map(|text| {
            decode_i32(text)
                .ok_or_else(|| DataError::Invalid(format!("integer value required, found: {text}")))
        })
        .transpose()
}

/// `parseLong(attrs, name)`.
fn attr_long(attr: &Option<String>) -> Result<Option<i64>, DataError> {
    attr.as_deref()
        .map(|text| {
            decode_i64(text).ok_or_else(|| DataError::Invalid(format!("long value required, found: {text}")))
        })
        .transpose()
}

/// `parseDouble(attrs, name)`: `Double.valueOf`.
fn attr_double(attr: &Option<String>) -> Result<Option<f64>, DataError> {
    attr.as_deref()
        .map(|text| {
            parse_java_double(text)
                .ok_or_else(|| DataError::Invalid(format!("double value required, found: {text}")))
        })
        .transpose()
}

/// `parseBoolean(attrs, name)`: `Boolean.valueOf`, so only `true` (any case) is true.
fn attr_bool(attr: &Option<String>) -> Option<bool> {
    attr.as_deref().map(|text| text.eq_ignore_ascii_case("true"))
}

// ---- one reader per NpcData `case` ----

fn read_parameters(
    parameters: &RawParameters,
    out: &mut BTreeMap<String, NpcParam>,
) -> Result<(), DataError> {
    for child in &parameters.children {
        match child {
            RawParameter::Param(param) => {
                // Java `HashMap.put(name, null)` when `value` is missing: the entry is as good as gone.
                let Some(name) = &param.name else { continue };
                match &param.value {
                    Some(value) => out.insert(name.clone(), NpcParam::Text(value.clone())),
                    None => out.remove(name),
                };
            }
            RawParameter::Skill(skill) => {
                let id = attr_int(&skill.id)?.ok_or_else(|| missing("id"))?;
                let level = attr_int(&skill.level)?.ok_or_else(|| missing("level"))?;
                if let Some(name) = &skill.name {
                    out.insert(name.clone(), NpcParam::Skill { id, level });
                }
            }
            RawParameter::Minions(minions) => {
                let list = read_minions(minions)?;
                // Java only stores a non-empty list.
                if let (false, Some(name)) = (list.is_empty(), &minions.name) {
                    out.insert(name.clone(), NpcParam::Minions(list));
                }
            }
            RawParameter::Other => {}
        }
    }
    Ok(())
}

fn read_minions(minions: &RawMinions) -> Result<Vec<MinionDef>, DataError> {
    let mut list = Vec::new();
    for child in &minions.children {
        let RawMinion::Npc(npc) = child else { continue };
        list.push(MinionDef {
            id: attr_int(&npc.id)?.ok_or_else(|| missing("id"))?,
            count: attr_int(&npc.count)?.ok_or_else(|| missing("count"))?,
            max: attr_int(&npc.max)?.unwrap_or(0),
            respawn_time: i64::from(attr_int(&npc.respawn_time)?.ok_or_else(|| missing("respawnTime"))?),
            weight_point: attr_int(&npc.weight_point)?.unwrap_or(0),
        });
    }
    Ok(list)
}

fn read_equipment(equipment: &RawEquipment, set: &mut StatSet) -> Result<(), DataError> {
    set.set_int("chestId", attr_int(&equipment.chest)?);
    set.set_int("rhandId", attr_int(&equipment.rhand)?);
    set.set_int("lhandId", attr_int(&equipment.lhand)?);
    set.set_int("weaponEnchant", attr_int(&equipment.weapon_enchant)?);
    Ok(())
}

fn read_stats(stats: &RawStats, set: &mut StatSet) -> Result<(), DataError> {
    set.set_int("baseSTR", attr_int(&stats.str_)?);
    set.set_int("baseINT", attr_int(&stats.int)?);
    set.set_int("baseDEX", attr_int(&stats.dex)?);
    set.set_int("baseWIT", attr_int(&stats.wit)?);
    set.set_int("baseCON", attr_int(&stats.con)?);
    set.set_int("baseMEN", attr_int(&stats.men)?);
    for child in &stats.children {
        match child {
            RawStatsChild::Vitals(vitals) => read_vitals(vitals, set)?,
            RawStatsChild::Attack(attack) => read_attack(attack, set)?,
            RawStatsChild::Defence(defence) => read_defence(defence, set)?,
            RawStatsChild::Attribute(attribute) => read_attribute(attribute, set)?,
            RawStatsChild::Speed(speed) => read_speed(speed, set)?,
            RawStatsChild::Other => {}
        }
    }
    Ok(())
}

fn read_vitals(vitals: &RawVitals, set: &mut StatSet) -> Result<(), DataError> {
    set.set_double("baseHpMax", attr_double(&vitals.hp)?);
    set.set_double("baseHpReg", attr_double(&vitals.hp_regen)?);
    set.set_double("baseMpMax", attr_double(&vitals.mp)?);
    set.set_double("baseMpReg", attr_double(&vitals.mp_regen)?);
    Ok(())
}

fn read_attack(attack: &RawAttack, set: &mut StatSet) -> Result<(), DataError> {
    set.set_double("basePAtk", attr_double(&attack.physical)?);
    set.set_double("baseMAtk", attr_double(&attack.magical)?);
    set.set_int("baseRndDam", attr_int(&attack.random)?);
    set.set_int("baseCritRate", attr_int(&attack.critical)?);
    set.set_double("accuracy", attr_double(&attack.accuracy)?);
    set.set_int("basePAtkSpd", attr_int(&attack.attack_speed)?);
    set.set_int("reuseDelay", attr_int(&attack.reuse_delay)?);
    set.set_text("baseAtkType", attack.kind.as_deref());
    set.set_int("baseAtkRange", attr_int(&attack.range)?);
    set.set_int("distance", attr_int(&attack.distance)?);
    set.set_int("width", attr_int(&attack.width)?);
    Ok(())
}

fn read_defence(defence: &RawDefence, set: &mut StatSet) -> Result<(), DataError> {
    set.set_double("basePDef", attr_double(&defence.physical)?);
    set.set_double("baseMDef", attr_double(&defence.magical)?);
    set.set_int("evasion", attr_int(&defence.evasion)?);
    set.set_int("baseShldDef", attr_int(&defence.shield)?);
    set.set_int("baseShldRate", attr_int(&defence.shield_rate)?);
    Ok(())
}

fn read_attribute(attribute: &RawAttribute, set: &mut StatSet) -> Result<(), DataError> {
    for child in &attribute.children {
        match child {
            RawAttributeChild::Attack(attack) => {
                // `parseString(attrs, "type").toUpperCase()`: a missing type is a NullPointerException.
                let kind = attack.kind.as_deref().ok_or_else(|| missing("type"))?;
                let key = match kind.to_uppercase().as_str() {
                    "FIRE" => "baseFire",
                    "WATER" => "baseWater",
                    "WIND" => "baseWind",
                    "EARTH" => "baseEarth",
                    "DARK" => "baseDark",
                    "HOLY" => "baseHoly",
                    // Java's switch has no default: other types are ignored.
                    _ => continue,
                };
                set.set_int(key, attr_int(&attack.value)?);
            }
            RawAttributeChild::Defence(defence) => {
                set.set_int("baseFireRes", attr_int(&defence.fire)?);
                set.set_int("baseWaterRes", attr_int(&defence.water)?);
                set.set_int("baseWindRes", attr_int(&defence.wind)?);
                set.set_int("baseEarthRes", attr_int(&defence.earth)?);
                set.set_int("baseHolyRes", attr_int(&defence.holy)?);
                set.set_int("baseDarkRes", attr_int(&defence.dark)?);
                set.set_int("baseElementRes", attr_int(&defence.default)?);
            }
            RawAttributeChild::Other => {}
        }
    }
    Ok(())
}

fn read_speed(speed: &RawSpeed, set: &mut StatSet) -> Result<(), DataError> {
    for child in &speed.children {
        match child {
            RawSpeedChild::Walk(walk) => {
                // `final double groundWalk = parseDouble(...)`: unboxing a missing attribute throws.
                let ground = attr_double(&walk.ground)?.ok_or_else(|| missing("ground"))?;
                set.set_double("baseWalkSpd", Some(if ground <= 0.0 { 0.1 } else { ground }));
                set.set_double("baseSwimWalkSpd", attr_double(&walk.swim)?);
                set.set_double("baseFlyWalkSpd", attr_double(&walk.fly)?);
            }
            RawSpeedChild::Run(run) => {
                let ground = attr_double(&run.ground)?.ok_or_else(|| missing("ground"))?;
                set.set_double("baseRunSpd", Some(if ground <= 0.0 { 0.1 } else { ground }));
                set.set_double("baseSwimRunSpd", attr_double(&run.swim)?);
                set.set_double("baseFlyRunSpd", attr_double(&run.fly)?);
            }
            RawSpeedChild::Other => {}
        }
    }
    Ok(())
}

fn read_status(status: &RawStatus, set: &mut StatSet) {
    set.set_bool("unique", attr_bool(&status.unique));
    set.set_bool("attackable", attr_bool(&status.attackable));
    set.set_bool("targetable", attr_bool(&status.targetable));
    set.set_bool("talkable", attr_bool(&status.talkable));
    set.set_bool("undying", attr_bool(&status.undying));
    set.set_bool("showName", attr_bool(&status.show_name));
    set.set_bool("randomWalk", attr_bool(&status.random_walk));
    set.set_bool("randomAnimation", attr_bool(&status.random_animation));
    set.set_bool("flying", attr_bool(&status.flying));
    set.set_bool("canMove", attr_bool(&status.can_move));
    set.set_bool("noSleepMode", attr_bool(&status.no_sleep_mode));
    set.set_bool("passableDoor", attr_bool(&status.passable_door));
    set.set_bool("hasSummoner", attr_bool(&status.has_summoner));
    set.set_bool("canBeSown", attr_bool(&status.can_be_sown));
}

fn read_skill_list(list: &RawSkillList) -> Result<Vec<NpcSkillRef>, DataError> {
    let mut skills = Vec::new();
    for child in &list.children {
        let RawSkillListChild::Skill(skill) = child else {
            continue;
        };
        skills.push(NpcSkillRef {
            id: attr_int(&skill.id)?.ok_or_else(|| missing("id"))?,
            level: attr_int(&skill.level)?.ok_or_else(|| missing("level"))?,
        });
    }
    Ok(skills)
}

fn read_shots(shots: &RawShots, set: &mut StatSet) -> Result<(), DataError> {
    set.set_int("soulShot", attr_int(&shots.soul)?);
    set.set_int("spiritShot", attr_int(&shots.spirit)?);
    set.set_int("soulShotChance", attr_int(&shots.shot_chance)?);
    set.set_int("spiritShotChance", attr_int(&shots.spirit_chance)?);
    Ok(())
}

fn read_collision(collision: &RawCollision, set: &mut StatSet) -> Result<(), DataError> {
    for child in &collision.children {
        match child {
            RawCollisionChild::Radius(size) => {
                set.set_double("collisionRadius", attr_double(&size.normal)?);
                set.set_double("collisionRadiusGrown", attr_double(&size.grown)?);
            }
            RawCollisionChild::Height(size) => {
                set.set_double("collisionHeight", attr_double(&size.normal)?);
                set.set_double("collisionHeightGrown", attr_double(&size.grown)?);
            }
            RawCollisionChild::Other => {}
        }
    }
    Ok(())
}

/// `case "droplists"`, parsed tolerantly: a malformed entry is skipped with a warning (Java would
/// abort the file with a NullPointerException or NumberFormatException).
fn read_drop_lists(lists: &RawDropLists, npc_id: i32, parts: &mut Parts) {
    for list in &lists.children {
        let (drop_type, list): (DropType, &RawDropList) = match list {
            RawDropListsChild::Drop(list) => (DropType::Drop, list),
            RawDropListsChild::Spoil(list) => (DropType::Spoil, list),
            RawDropListsChild::Other => continue,
        };
        for node in &list.children {
            match node {
                RawDropNode::Group(group) => {
                    let Some(chance) = attr_double(&group.chance).ok().flatten() else {
                        warn!(
                            "{}: NPC {npc_id}: drop group without a valid chance skipped",
                            NpcTemplates::LOADER
                        );
                        continue;
                    };
                    let items = group
                        .children
                        .iter()
                        .filter_map(|child| match child {
                            RawGroupNode::Item(item) => drop_entry(item, drop_type, npc_id),
                            RawGroupNode::Other => None,
                        })
                        .collect();
                    parts.drop_groups.push(DropGroup { chance, items });
                }
                RawDropNode::Item(item) => {
                    if let Some(entry) = drop_entry(item, drop_type, npc_id) {
                        match drop_type {
                            DropType::Drop => parts.drops.push(entry),
                            DropType::Spoil => parts.spoils.push(entry),
                        }
                    }
                }
                RawDropNode::Other => {}
            }
        }
    }
}

fn drop_entry(item: &RawDropItem, drop_type: DropType, npc_id: i32) -> Option<DropEntry> {
    let parsed = (|| -> Result<Option<DropEntry>, DataError> {
        Ok(Some(DropEntry {
            drop_type,
            item_id: attr_int(&item.id)?.ok_or_else(|| missing("id"))?,
            min: attr_long(&item.min)?.ok_or_else(|| missing("min"))?,
            max: attr_long(&item.max)?.ok_or_else(|| missing("max"))?,
            chance: attr_double(&item.chance)?.ok_or_else(|| missing("chance"))?,
        }))
    })();
    match parsed {
        Ok(entry) => entry,
        Err(error) => {
            warn!("{}: NPC {npc_id}: drop skipped: {error}", NpcTemplates::LOADER);
            None
        }
    }
}
