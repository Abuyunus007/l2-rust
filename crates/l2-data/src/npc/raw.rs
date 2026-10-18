//! The raw XML shape of `data/stats/npcs/*.xml` (`xsd/npcs.xsd`).
//!
//! Every attribute is optional text: Java reads attributes one by one and decides per attribute
//! what a missing or malformed value means (see [`super`]), so nothing is parsed here. Children are
//! kept in document order as enums, because Java walks the child nodes in order and a later node
//! overwrites an earlier one.

use serde::Deserialize;
use serde::de::IgnoredAny;

#[derive(Deserialize)]
pub(super) struct RawNpcFile {
    #[serde(rename = "npc", default)]
    pub npcs: Vec<RawNpc>,
}

#[derive(Deserialize)]
pub(super) struct RawNpc {
    #[serde(rename = "@id")]
    pub id: Option<String>,
    #[serde(rename = "@displayId")]
    pub display_id: Option<String>,
    #[serde(rename = "@level")]
    pub level: Option<String>,
    #[serde(rename = "@type")]
    pub kind: Option<String>,
    #[serde(rename = "@name")]
    pub name: Option<String>,
    #[serde(rename = "@usingServerSideName")]
    pub using_server_side_name: Option<String>,
    #[serde(rename = "@title")]
    pub title: Option<String>,
    #[serde(rename = "@usingServerSideTitle")]
    pub using_server_side_title: Option<String>,
    #[serde(rename = "$value", default)]
    pub children: Vec<RawNpcChild>,
}

#[derive(Deserialize)]
pub(super) enum RawNpcChild {
    #[serde(rename = "parameters")]
    Parameters(RawParameters),
    #[serde(rename = "race")]
    Race(RawText),
    #[serde(rename = "sex")]
    Sex(RawText),
    #[serde(rename = "equipment")]
    Equipment(RawEquipment),
    #[serde(rename = "acquire")]
    Acquire(RawAcquire),
    #[serde(rename = "stats")]
    Stats(RawStats),
    #[serde(rename = "status")]
    Status(RawStatus),
    #[serde(rename = "fakePlayer")]
    FakePlayer(IgnoredAny),
    #[serde(rename = "skillList")]
    SkillList(RawSkillList),
    #[serde(rename = "shots")]
    Shots(RawShots),
    #[serde(rename = "corpseTime")]
    CorpseTime(RawText),
    #[serde(rename = "ai")]
    Ai(RawAi),
    #[serde(rename = "dropLists")]
    DropLists(RawDropLists),
    #[serde(rename = "collision")]
    Collision(RawCollision),
    /// `exCrtEffect`, `sNpcPropHpRate`, comments' neighbours and anything unknown: Java reads
    /// the first two into its set but nothing ever uses them (`TODO: Implement me`).
    #[serde(other)]
    Other,
}

/// An element whose value is its text (`<race>HUMAN</race>`).
#[derive(Deserialize)]
pub(super) struct RawText {
    #[serde(rename = "$text")]
    pub text: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawParameters {
    #[serde(rename = "$value", default)]
    pub children: Vec<RawParameter>,
}

#[derive(Deserialize)]
pub(super) enum RawParameter {
    #[serde(rename = "param")]
    Param(RawParam),
    #[serde(rename = "skill")]
    Skill(RawParamSkill),
    #[serde(rename = "minions")]
    Minions(RawMinions),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct RawParam {
    #[serde(rename = "@name")]
    pub name: Option<String>,
    #[serde(rename = "@value")]
    pub value: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawParamSkill {
    #[serde(rename = "@name")]
    pub name: Option<String>,
    #[serde(rename = "@id")]
    pub id: Option<String>,
    #[serde(rename = "@level")]
    pub level: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawMinions {
    #[serde(rename = "@name")]
    pub name: Option<String>,
    #[serde(rename = "$value", default)]
    pub children: Vec<RawMinion>,
}

#[derive(Deserialize)]
pub(super) enum RawMinion {
    #[serde(rename = "npc")]
    Npc(RawMinionNpc),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct RawMinionNpc {
    #[serde(rename = "@id")]
    pub id: Option<String>,
    #[serde(rename = "@count")]
    pub count: Option<String>,
    #[serde(rename = "@max")]
    pub max: Option<String>,
    #[serde(rename = "@respawnTime")]
    pub respawn_time: Option<String>,
    #[serde(rename = "@weightPoint")]
    pub weight_point: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawEquipment {
    #[serde(rename = "@chest")]
    pub chest: Option<String>,
    #[serde(rename = "@rhand")]
    pub rhand: Option<String>,
    #[serde(rename = "@lhand")]
    pub lhand: Option<String>,
    #[serde(rename = "@weaponEnchant")]
    pub weapon_enchant: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawAcquire {
    #[serde(rename = "@exp")]
    pub exp: Option<String>,
    #[serde(rename = "@sp")]
    pub sp: Option<String>,
    #[serde(rename = "@raidPoints")]
    pub raid_points: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawStats {
    #[serde(rename = "@str")]
    pub str_: Option<String>,
    #[serde(rename = "@int")]
    pub int: Option<String>,
    #[serde(rename = "@dex")]
    pub dex: Option<String>,
    #[serde(rename = "@wit")]
    pub wit: Option<String>,
    #[serde(rename = "@con")]
    pub con: Option<String>,
    #[serde(rename = "@men")]
    pub men: Option<String>,
    #[serde(rename = "$value", default)]
    pub children: Vec<RawStatsChild>,
}

#[derive(Deserialize)]
pub(super) enum RawStatsChild {
    #[serde(rename = "vitals")]
    Vitals(RawVitals),
    #[serde(rename = "attack")]
    Attack(RawAttack),
    #[serde(rename = "defence")]
    Defence(RawDefence),
    #[serde(rename = "attribute")]
    Attribute(RawAttribute),
    #[serde(rename = "speed")]
    Speed(RawSpeed),
    /// `hitTime`: Java stores the text of the whole `<stats>` element under that key and never reads it.
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct RawVitals {
    #[serde(rename = "@hp")]
    pub hp: Option<String>,
    #[serde(rename = "@hpRegen")]
    pub hp_regen: Option<String>,
    #[serde(rename = "@mp")]
    pub mp: Option<String>,
    #[serde(rename = "@mpRegen")]
    pub mp_regen: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawAttack {
    #[serde(rename = "@physical")]
    pub physical: Option<String>,
    #[serde(rename = "@magical")]
    pub magical: Option<String>,
    #[serde(rename = "@random")]
    pub random: Option<String>,
    #[serde(rename = "@critical")]
    pub critical: Option<String>,
    #[serde(rename = "@accuracy")]
    pub accuracy: Option<String>,
    #[serde(rename = "@attackSpeed")]
    pub attack_speed: Option<String>,
    #[serde(rename = "@reuseDelay")]
    pub reuse_delay: Option<String>,
    #[serde(rename = "@type")]
    pub kind: Option<String>,
    #[serde(rename = "@range")]
    pub range: Option<String>,
    #[serde(rename = "@distance")]
    pub distance: Option<String>,
    #[serde(rename = "@width")]
    pub width: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawDefence {
    #[serde(rename = "@physical")]
    pub physical: Option<String>,
    #[serde(rename = "@magical")]
    pub magical: Option<String>,
    #[serde(rename = "@evasion")]
    pub evasion: Option<String>,
    #[serde(rename = "@shield")]
    pub shield: Option<String>,
    #[serde(rename = "@shieldRate")]
    pub shield_rate: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawAttribute {
    #[serde(rename = "$value", default)]
    pub children: Vec<RawAttributeChild>,
}

#[derive(Deserialize)]
pub(super) enum RawAttributeChild {
    #[serde(rename = "attack")]
    Attack(RawAttributeAttack),
    #[serde(rename = "defence")]
    Defence(RawAttributeDefence),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct RawAttributeAttack {
    #[serde(rename = "@type")]
    pub kind: Option<String>,
    #[serde(rename = "@value")]
    pub value: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawAttributeDefence {
    #[serde(rename = "@fire")]
    pub fire: Option<String>,
    #[serde(rename = "@water")]
    pub water: Option<String>,
    #[serde(rename = "@wind")]
    pub wind: Option<String>,
    #[serde(rename = "@earth")]
    pub earth: Option<String>,
    #[serde(rename = "@holy")]
    pub holy: Option<String>,
    #[serde(rename = "@dark")]
    pub dark: Option<String>,
    #[serde(rename = "@default")]
    pub default: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawSpeed {
    #[serde(rename = "$value", default)]
    pub children: Vec<RawSpeedChild>,
}

#[derive(Deserialize)]
pub(super) enum RawSpeedChild {
    #[serde(rename = "walk")]
    Walk(RawMoveSpeed),
    #[serde(rename = "run")]
    Run(RawMoveSpeed),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct RawMoveSpeed {
    #[serde(rename = "@ground")]
    pub ground: Option<String>,
    #[serde(rename = "@swim")]
    pub swim: Option<String>,
    #[serde(rename = "@fly")]
    pub fly: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawStatus {
    #[serde(rename = "@unique")]
    pub unique: Option<String>,
    #[serde(rename = "@attackable")]
    pub attackable: Option<String>,
    #[serde(rename = "@targetable")]
    pub targetable: Option<String>,
    #[serde(rename = "@talkable")]
    pub talkable: Option<String>,
    #[serde(rename = "@undying")]
    pub undying: Option<String>,
    #[serde(rename = "@showName")]
    pub show_name: Option<String>,
    #[serde(rename = "@randomWalk")]
    pub random_walk: Option<String>,
    #[serde(rename = "@randomAnimation")]
    pub random_animation: Option<String>,
    #[serde(rename = "@flying")]
    pub flying: Option<String>,
    #[serde(rename = "@canMove")]
    pub can_move: Option<String>,
    #[serde(rename = "@noSleepMode")]
    pub no_sleep_mode: Option<String>,
    #[serde(rename = "@passableDoor")]
    pub passable_door: Option<String>,
    #[serde(rename = "@hasSummoner")]
    pub has_summoner: Option<String>,
    #[serde(rename = "@canBeSown")]
    pub can_be_sown: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawSkillList {
    #[serde(rename = "$value", default)]
    pub children: Vec<RawSkillListChild>,
}

#[derive(Deserialize)]
pub(super) enum RawSkillListChild {
    #[serde(rename = "skill")]
    Skill(RawSkillRef),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct RawSkillRef {
    #[serde(rename = "@id")]
    pub id: Option<String>,
    #[serde(rename = "@level")]
    pub level: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawShots {
    #[serde(rename = "@soul")]
    pub soul: Option<String>,
    #[serde(rename = "@spirit")]
    pub spirit: Option<String>,
    #[serde(rename = "@shotChance")]
    pub shot_chance: Option<String>,
    #[serde(rename = "@spiritChance")]
    pub spirit_chance: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawAi {
    #[serde(rename = "@type")]
    pub kind: Option<String>,
    #[serde(rename = "@aggroRange")]
    pub aggro_range: Option<String>,
    #[serde(rename = "@clanHelpRange")]
    pub clan_help_range: Option<String>,
    #[serde(rename = "@isChaos")]
    pub is_chaos: Option<String>,
    #[serde(rename = "@isAggressive")]
    pub is_aggressive: Option<String>,
    #[serde(rename = "$value", default)]
    pub children: Vec<RawAiChild>,
}

#[derive(Deserialize)]
pub(super) enum RawAiChild {
    #[serde(rename = "skill")]
    Skill(RawAiSkill),
    #[serde(rename = "clanList")]
    ClanList(RawClanList),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct RawAiSkill {
    #[serde(rename = "@minChance")]
    pub min_chance: Option<String>,
    #[serde(rename = "@maxChance")]
    pub max_chance: Option<String>,
    #[serde(rename = "@primaryId")]
    pub primary_id: Option<String>,
    #[serde(rename = "@shortRangeId")]
    pub short_range_id: Option<String>,
    #[serde(rename = "@shortRangeChance")]
    pub short_range_chance: Option<String>,
    #[serde(rename = "@longRangeId")]
    pub long_range_id: Option<String>,
    #[serde(rename = "@longRangeChance")]
    pub long_range_chance: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawClanList {
    #[serde(rename = "$value", default)]
    pub children: Vec<RawClanListChild>,
}

#[derive(Deserialize)]
pub(super) enum RawClanListChild {
    #[serde(rename = "clan")]
    Clan(RawText),
    #[serde(rename = "ignoreNpcId")]
    IgnoreNpcId(RawText),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct RawDropLists {
    #[serde(rename = "$value", default)]
    pub children: Vec<RawDropListsChild>,
}

#[derive(Deserialize)]
pub(super) enum RawDropListsChild {
    #[serde(rename = "drop")]
    Drop(RawDropList),
    #[serde(rename = "spoil")]
    Spoil(RawDropList),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct RawDropList {
    #[serde(rename = "$value", default)]
    pub children: Vec<RawDropNode>,
}

#[derive(Deserialize)]
pub(super) enum RawDropNode {
    #[serde(rename = "group")]
    Group(RawDropGroup),
    #[serde(rename = "item")]
    Item(RawDropItem),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct RawDropGroup {
    #[serde(rename = "@chance")]
    pub chance: Option<String>,
    #[serde(rename = "$value", default)]
    pub children: Vec<RawGroupNode>,
}

#[derive(Deserialize)]
pub(super) enum RawGroupNode {
    #[serde(rename = "item")]
    Item(RawDropItem),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct RawDropItem {
    #[serde(rename = "@id")]
    pub id: Option<String>,
    #[serde(rename = "@min")]
    pub min: Option<String>,
    #[serde(rename = "@max")]
    pub max: Option<String>,
    #[serde(rename = "@chance")]
    pub chance: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RawCollision {
    #[serde(rename = "$value", default)]
    pub children: Vec<RawCollisionChild>,
}

#[derive(Deserialize)]
pub(super) enum RawCollisionChild {
    #[serde(rename = "radius")]
    Radius(RawSize),
    #[serde(rename = "height")]
    Height(RawSize),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct RawSize {
    #[serde(rename = "@normal")]
    pub normal: Option<String>,
    #[serde(rename = "@grown")]
    pub grown: Option<String>,
}
