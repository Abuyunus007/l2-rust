//! Condition trees (data only) and the parser that builds them from XML.
//!
//! Java: `mechanics/conditions/*.java` (one [`Condition`] variant per class `DocumentBase` can
//! create, constructor arguments kept as typed fields) and `util/DocumentBase.java`
//! (`parseCondition`, `parseLogicAnd/Or/Not`, `parse{Player,Target,Using,Game}Condition`, `joinAnd`,
//! `getValue`, and the `<conditions msg msgId addName>` handling of `parseTemplate`/`DocumentItem`).
//!
//! There is **no evaluation here** (M4-STAT-01 evaluates a [`Condition`] against the world). A
//! condition is plain data: `Clone`, `PartialEq`, no ids resolved, no references.
//!
//! # How to use the parser (items, and the skill loader M4-DATA-11)
//! ```ignore
//! // Items never have `#table` values: a resolver that finds nothing makes `#x` an error,
//! // exactly like Java's `IllegalStateException` for a non-skill template.
//! let parser = ConditionParser::new("00000-00099.xml", &|_| None);
//! // Skills pass the per-level lookup of the skill being built:
//! let parser = ConditionParser::new(file, &|name| tables.get(name).and_then(|t| t.get(level - 1)).cloned());
//! let condition = parser.parse_condition(&node.children, 0)?;          // Java parseCondition(n.getFirstChild())
//! let attached = parser.parse_conditions_block(&conditions_node)?;      // + msg / msgId / addName
//! ```
//! The resolver is Java's `getValue(value, template)` for a `Skill` (`getTableValue(name)`) or an
//! `Integer` level template (`getTableValue(name, level)`): it gets the `#name` including the `#`
//! and returns the table entry, or `None` for an unknown table or a level beyond the table.
//! Java calls `getValue(value, null)` for a number of attributes (`hp`, `mp`, `weight`, ...); for
//! those a `#table` reference is always an error, whatever the resolver says. The parser keeps
//! that difference (see `plain_value`).
//!
//! # Errors and warnings
//! Everything Java would throw (`NumberFormatException`, `Enum.valueOf`, missing tokens, ...)
//! is a [`ParseError`]; the caller fails the whole item/skill like Java does. The `LOGGER.severe`
//! / `info` lines are `tracing` events with Java's text (`Unrecognized <player> condition in <file>`).

use std::collections::BTreeSet;

use tracing::{error, info};

use super::enums::{AbnormalType, CategoryType, InstanceType, PlayerState};
use super::jnum::{decode_i32, java_split, number_format_message, parse_boolean, parse_int, tokens};
use super::node::XmlNode;
use crate::item::{ArmorType, BodyPart, WeaponType};
use crate::npc::NpcRace;

/// Java's `getValue(value, template)` for a skill/level template: `#name` to its table entry.
pub type TableResolver<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Why an XML node could not be turned into a condition or function (the cases where Java
/// throws). The text is Java's exception message where there is one.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    /// Anything Java would throw for.
    #[error("{0}")]
    Invalid(String),
}

impl ParseError {
    pub(crate) fn invalid(text: impl Into<String>) -> Self {
        ParseError::Invalid(text.into())
    }
}

/// Condition text and system message that `<conditions msg=".." msgId=".." addName="..">` attach
/// to the root of a condition (Java `Condition.setMessage/setMessageId/addName`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConditionMessage {
    /// `msg`: text sent to the player when the condition fails; wins over `msg_id`.
    pub message: Option<String>,
    /// `msgId`: a `SystemMessageId` id, 0 when absent.
    pub message_id: i32,
    /// `addName` (with a positive `msgId`): add the item name to the system message.
    pub add_name: bool,
}

/// A condition with its failure message. Java keeps both on the same `Condition` object.
#[derive(Debug, Clone, PartialEq)]
pub struct MessagedCondition {
    /// The condition tree.
    pub condition: Condition,
    /// What to tell the player when it fails.
    pub message: ConditionMessage,
}

/// Java `mechanics/conditions/Condition` and its subclasses that the XML parser creates.
///
/// Names follow the Java class without the `Condition` prefix (`ConditionPlayerLevel` is
/// `PlayerLevel`); the fields are the constructor arguments in Java's order.
#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    // ---- logic ----
    /// `ConditionLogicAnd`: all must hold. Empty is allowed (Java logs it and keeps it).
    And(Vec<Condition>),
    /// `ConditionLogicOr`: any must hold.
    Or(Vec<Condition>),
    /// `ConditionLogicNot`.
    Not(Box<Condition>),

    // ---- <player ...> ----
    /// `ConditionPlayerRace(Set<Race>)`; attribute `races`.
    PlayerRace { races: BTreeSet<NpcRace> },
    /// `ConditionPlayerLevel(int)`; attribute `level`.
    PlayerLevel { level: i32 },
    /// `ConditionPlayerLevelRange(int[2])`; attribute `levelRange` (`min;max`).
    PlayerLevelRange { levels: [i32; 2] },
    /// `ConditionPlayerState(PlayerState, boolean)`; attributes `resting`, `flying`, `moving`,
    /// `running`, `standing`, `behind`, `front`, `chaotic`, `olympiad`.
    PlayerState { check: PlayerState, required: bool },
    /// `ConditionPlayerIsHero(boolean)`; attribute `isHero`.
    PlayerIsHero { value: bool },
    /// `ConditionPlayerIsPvpFlagged(boolean)`; attribute `isPvpFlagged`.
    PlayerIsPvpFlagged { value: bool },
    /// `ConditionPlayerTransformationId(int)`; attribute `transformationId`.
    PlayerTransformationId { id: i32 },
    /// `ConditionPlayerIsRidingStrider(boolean)`; attribute `isRidingStrider`.
    PlayerIsRidingStrider { value: bool },
    /// `ConditionPlayerHp(int)`; attribute `hp`.
    PlayerHp { hp: i32 },
    /// `ConditionPlayerMp(int)`; attribute `mp`.
    PlayerMp { mp: i32 },
    /// `ConditionPlayerCp(int)`; attribute `cp`.
    PlayerCp { cp: i32 },
    /// `ConditionPlayerGrade(int)`; attribute `grade`.
    PlayerGrade { value: i32 },
    /// `ConditionPlayerPkCount(int)`; attribute `pkCount`.
    PlayerPkCount { pk: i32 },
    /// `ConditionSiegeZone(int, boolean)`; attribute `siegeZone` of `<player>` (`self` true) or
    /// `<target>` (`self` false).
    SiegeZone { value: i32, own: bool },
    /// `ConditionPlayerSiegeSide(int)`; attribute `siegeSide`.
    PlayerSiegeSide { siege_side: i32 },
    /// `ConditionPlayerCharges(int)`; attribute `charges`.
    PlayerCharges { charges: i32 },
    /// `ConditionPlayerSouls(int)`; attribute `souls`.
    PlayerSouls { souls: i32 },
    /// `ConditionPlayerWeight(int)`; attribute `weight`.
    PlayerWeight { weight: i32 },
    /// `ConditionPlayerInvSize(int)`; attribute `invSize`.
    PlayerInvSize { size: i32 },
    /// `ConditionPlayerIsClanLeader(boolean)`; attribute `isClanLeader`.
    PlayerIsClanLeader { value: bool },
    /// `ConditionPlayerPledgeClass(int)`; attribute `pledgeClass`.
    PlayerPledgeClass { pledge_class: i32 },
    /// `ConditionPlayerHasClanHall(List<Integer>)`; attribute `clanHall`.
    PlayerHasClanHall { clan_hall: Vec<i32> },
    /// `ConditionPlayerHasFort(int)`; attribute `fort`.
    PlayerHasFort { fort: i32 },
    /// `ConditionPlayerHasCastle(int)`; attribute `castle`.
    PlayerHasCastle { castle: i32 },
    /// `ConditionPlayerSex(int)`; attribute `sex`.
    PlayerSex { sex: i32 },
    /// `ConditionPlayerFlyMounted(boolean)`; attribute `flyMounted`.
    PlayerFlyMounted { value: bool },
    /// `ConditionPlayerVehicleMounted(boolean)`; attribute `vehicleMounted`.
    PlayerVehicleMounted { value: bool },
    /// `ConditionPlayerLandingZone(boolean)`; attribute `landingZone`.
    PlayerLandingZone { value: bool },
    /// `ConditionPlayerActiveEffectId(int[, int])`; attributes `active_effect_id`,
    /// `active_effect_id_lvl`. `effect_lvl` is -1 for "any level".
    PlayerActiveEffectId { effect_id: i32, effect_lvl: i32 },
    /// `ConditionPlayerActiveSkillId(int[, int])`; attributes `active_skill_id`,
    /// `active_skill_id_lvl`. `skill_level` is -1 for "any level".
    PlayerActiveSkillId { skill_id: i32, skill_level: i32 },
    /// `ConditionPlayerClassIdRestriction(Set<Integer>)`; attribute `class_id_restriction`.
    PlayerClassIdRestriction { class_ids: BTreeSet<i32> },
    /// `ConditionPlayerSubclass(boolean)`; attribute `subclass`.
    PlayerSubclass { value: bool },
    /// `ConditionPlayerInstanceId(Set<Integer>)`; attribute `instanceId`.
    PlayerInstanceId { instance_ids: BTreeSet<i32> },
    /// `ConditionPlayerAgathionId(int)`; attribute `agathionId`.
    PlayerAgathionId { agathion_id: i32 },
    /// `ConditionPlayerCloakStatus(boolean)`; attribute `cloakStatus`.
    PlayerCloakStatus { value: bool },
    /// `ConditionPlayerHasPet(List<Integer>)`; attribute `hasPet`.
    PlayerHasPet { control_item_ids: Vec<i32> },
    /// `ConditionPlayerServitorNpcId(List<Integer>)`; attribute `servitorNpcId`.
    PlayerServitorNpcId { npc_ids: Vec<i32> },
    /// `ConditionPlayerRangeFromNpc(Set<Integer>, int, boolean)`; attribute `npcIdRadius`
    /// (`id;id,radius,value`).
    PlayerRangeFromNpc {
        npc_ids: BTreeSet<i32>,
        radius: i32,
        value: bool,
    },
    /// `ConditionPlayerCallPc(boolean)`; attribute `callPc`.
    PlayerCallPc { value: bool },
    /// `ConditionPlayerCanCreateBase(boolean)`; attribute `canCreateBase`.
    PlayerCanCreateBase { value: bool },
    /// `ConditionPlayerCanCreateOutpost(boolean)`; attribute `canCreateOutpost`.
    PlayerCanCreateOutpost { value: bool },
    /// `ConditionPlayerCanEscape(boolean)`; attribute `canEscape`.
    PlayerCanEscape { value: bool },
    /// `ConditionPlayerCanRefuelAirship(int)`; attribute `canRefuelAirship`.
    PlayerCanRefuelAirship { value: i32 },
    /// `ConditionPlayerCanResurrect(boolean)`; attribute `canResurrect`.
    PlayerCanResurrect { value: bool },
    /// `ConditionPlayerCanSummon(boolean)`; attribute `canSummon`.
    PlayerCanSummon { value: bool },
    /// `ConditionPlayerCanSummonSiegeGolem(boolean)`; attribute `canSummonSiegeGolem`.
    PlayerCanSummonSiegeGolem { value: bool },
    /// `ConditionPlayerCanSweep(boolean)`; attribute `canSweep`.
    PlayerCanSweep { value: bool },
    /// `ConditionPlayerCanTakeCastle()`; attribute `canTakeCastle` (its value is ignored).
    PlayerCanTakeCastle,
    /// `ConditionPlayerCanTakeFort(boolean)`; attribute `canTakeFort`.
    PlayerCanTakeFort { value: bool },
    /// `ConditionPlayerCanTransform(boolean)`; attribute `canTransform`.
    PlayerCanTransform { value: bool },
    /// `ConditionPlayerCanUntransform(boolean)`; attribute `canUntransform`.
    PlayerCanUntransform { value: bool },
    /// `ConditionPlayerInsideZoneId(Set<Integer>)`; attribute `insideZoneId`.
    PlayerInsideZoneId { zones: BTreeSet<i32> },
    /// `ConditionPlayerCheckAbnormal(AbnormalType[, int])`; attribute `checkAbnormal`. `level`
    /// is -1 for "any level".
    PlayerCheckAbnormal { abnormal_type: AbnormalType, level: i32 },
    /// `ConditionCategoryType(Set<CategoryType>)`; attribute `categoryType`.
    CategoryType { category_types: BTreeSet<CategoryType> },

    // ---- <target ...> ----
    /// `ConditionTargetAggro(boolean)`; attribute `aggro`.
    TargetAggro { is_aggro: bool },
    /// `ConditionTargetLevel(int)`; attribute `level`.
    TargetLevel { level: i32 },
    /// `ConditionTargetLevelRange(int[2])`; attribute `levelRange`.
    TargetLevelRange { levels: [i32; 2] },
    /// `ConditionTargetMyPartyExceptMe(boolean)`; attribute `myPartyExceptMe`.
    TargetMyPartyExceptMe { value: bool },
    /// `ConditionTargetPlayable()`; attribute `playable` (its value is ignored).
    TargetPlayable,
    /// `ConditionTargetClassIdRestriction(Set<Integer>)`; attribute `class_id_restriction`.
    TargetClassIdRestriction { class_ids: BTreeSet<i32> },
    /// `ConditionTargetActiveEffectId(int[, int])`; `effect_lvl` is -1 for "any level".
    TargetActiveEffectId { effect_id: i32, effect_lvl: i32 },
    /// `ConditionTargetActiveSkillId(int[, int])`; `skill_level` is -1 for "any level".
    TargetActiveSkillId { skill_id: i32, skill_level: i32 },
    /// `ConditionTargetAbnormal(int)`; attribute `abnormal`.
    TargetAbnormal { abnormal_id: i32 },
    /// `ConditionMinDistance(int)`; attribute `minDistance`.
    MinDistance { distance: i32 },
    /// `ConditionTargetRace(Race)`; attribute `race`.
    TargetRace { race: NpcRace },
    /// `ConditionTargetUsesWeaponKind(int mask)`; attribute `using` (weapon and armor type names).
    TargetUsesWeaponKind { weapon_mask: i32 },
    /// `ConditionTargetNpcId(Set<Integer>)`; attribute `npcId`.
    TargetNpcId { npc_ids: BTreeSet<i32> },
    /// `ConditionTargetNpcType(InstanceType[])`; attribute `npcType`.
    TargetNpcType { npc_types: Vec<InstanceType> },
    /// `ConditionTargetWeight(int)`; attribute `weight`.
    TargetWeight { weight: i32 },
    /// `ConditionTargetInvSize(int)`; attribute `invSize`.
    TargetInvSize { size: i32 },

    // ---- <using ...> ----
    /// `ConditionUsingItemType(int mask)`; attribute `kind` (weapon and armor type names).
    UsingItemType { mask: i32 },
    /// `ConditionUsingSlotType(int mask)`; attribute `slot` (body part names).
    UsingSlotType { mask: i32 },
    /// `ConditionUsingSkill(int)`; attribute `skill`.
    UsingSkill { skill_id: i32 },
    /// `ConditionSlotItemId(int slot, int itemId, int enchantLevel)`; attribute `slotItem`
    /// (`itemId;slot[;enchant]`).
    SlotItemId {
        slot: i32,
        item_id: i32,
        enchant_level: i32,
    },
    /// `ConditionChangeWeapon(boolean)`; attribute `weaponChange`.
    ChangeWeapon { required: bool },

    // ---- <game ...> ----
    /// `ConditionWithSkill(boolean)`; attribute `skill`.
    WithSkill { skill: bool },
    /// `ConditionGameTime(boolean)`; attribute `night`.
    GameTime { required: bool },
    /// `ConditionGameChance(int)`; attribute `chance`.
    GameChance { chance: i32 },
}

/// Java `DocumentBase.joinAnd`: the first leaf stays alone, the second turns it into an `and`.
fn join_and(cond: Option<Condition>, c: Condition) -> Condition {
    match cond {
        None => c,
        Some(Condition::And(mut all)) => {
            all.push(c);
            Condition::And(all)
        }
        Some(first) => Condition::And(vec![first, c]),
    }
}

/// Parses conditions out of XML nodes. One value per file being loaded (it only carries the file
/// name for the log lines and the `#table` resolver).
pub struct ConditionParser<'a> {
    file: &'a str,
    resolve: TableResolver<'a>,
}

impl<'a> ConditionParser<'a> {
    /// `file` is only used in log messages (Java prints the `File`). `resolve` is Java's
    /// `getValue(value, template)`; see the module docs.
    pub fn new(file: &'a str, resolve: TableResolver<'a>) -> Self {
        Self { file, resolve }
    }

    /// The file name given to [`ConditionParser::new`].
    pub fn file(&self) -> &str {
        self.file
    }

    /// Java `getValue(value, template)` with a skill/level template: a `#table` value is looked
    /// up with the resolver, anything else is returned as is. An unknown table is an error
    /// (Java: `NullPointerException` / `ArrayIndexOutOfBoundsException`).
    pub fn value(&self, value: &str) -> Result<String, ParseError> {
        if value.starts_with('#') {
            return (self.resolve)(value)
                .ok_or_else(|| ParseError::invalid(format!("unknown table value {value}")));
        }
        Ok(value.to_owned())
    }

    /// Java `getValue(value, null)`: a `#table` reference is an `IllegalStateException`.
    fn plain_value<'v>(&self, value: &'v str) -> Result<&'v str, ParseError> {
        if value.starts_with('#') {
            return Err(ParseError::invalid(format!(
                "table reference {value} where Java has no template (IllegalStateException)"
            )));
        }
        Ok(value)
    }

    /// `Integer.decode(getValue(value, template))`.
    pub fn decode_value(&self, value: &str) -> Result<i32, ParseError> {
        decode(&self.value(value)?)
    }

    /// `Integer.decode(getValue(value, null))`.
    fn decode_plain(&self, value: &str) -> Result<i32, ParseError> {
        decode(self.plain_value(value)?)
    }

    /// Java `parseCondition(Node node, template)`: `nodes[start..]` are the siblings that begin
    /// at `node`. Skips non-condition `<value>` elements and builds the first real one;
    /// `Ok(None)` for no element or an unknown element name.
    ///
    /// NOTE(java-quirk): called from `parseLogicAnd/Or/Not` with an element that is itself a
    /// `<value>`, Java continues with the *following siblings* (and the enclosing loop then
    /// visits them again). This function does the same.
    pub fn parse_condition(&self, nodes: &[XmlNode], start: usize) -> Result<Option<Condition>, ParseError> {
        let Some(n) = nodes
            .iter()
            .skip(start)
            .find(|n| !n.name.eq_ignore_ascii_case("value"))
        else {
            return Ok(None);
        };
        match n.name.to_lowercase().as_str() {
            "and" => self.parse_logic_and(n).map(Some),
            "or" => self.parse_logic_or(n).map(Some),
            "not" => self.parse_logic_not(n),
            "player" => self.parse_player_condition(n),
            "target" => self.parse_target_condition(n),
            "using" => self.parse_using_condition(n),
            "game" => self.parse_game_condition(n),
            _ => Ok(None),
        }
    }

    /// Java `<conditions msg msgId addName>...</conditions>` as read by `DocumentItem` and
    /// `parseTemplate`: the condition of the first element and its failure message. `Ok(None)`
    /// when the block holds no recognised condition.
    pub fn parse_conditions_block(&self, block: &XmlNode) -> Result<Option<MessagedCondition>, ParseError> {
        let condition = self.parse_condition(&block.children, 0)?;
        let msg = block.attr("msg");
        let msg_id = block.attr("msgId");
        let Some(condition) = condition else {
            return Ok(None);
        };
        let mut message = ConditionMessage::default();
        if let Some(msg) = msg {
            message.message = Some(msg.to_owned());
        } else if let Some(msg_id) = msg_id {
            message.message_id = self.decode_plain(msg_id)?;
            if block.attr("addName").is_some() && self.decode_plain(msg_id)? > 0 {
                message.add_name = true;
            }
        }
        Ok(Some(MessagedCondition { condition, message }))
    }

    fn parse_logic_and(&self, node: &XmlNode) -> Result<Condition, ParseError> {
        let mut all = Vec::new();
        for index in 0..node.children.len() {
            if let Some(c) = self.parse_condition(&node.children, index)? {
                all.push(c);
            }
        }
        if all.is_empty() {
            error!("Empty <and> condition in {}", self.file);
        }
        Ok(Condition::And(all))
    }

    fn parse_logic_or(&self, node: &XmlNode) -> Result<Condition, ParseError> {
        let mut any = Vec::new();
        for index in 0..node.children.len() {
            if let Some(c) = self.parse_condition(&node.children, index)? {
                any.push(c);
            }
        }
        if any.is_empty() {
            error!("Empty <or> condition in {}", self.file);
        }
        Ok(Condition::Or(any))
    }

    /// NOTE(java-quirk): Java builds `ConditionLogicNot(null)` when the child is not a known
    /// condition and fails with a `NullPointerException` only when the item is used. Here such a
    /// `<not>` is dropped (`None`) with the same severe log, so no half-built tree exists.
    fn parse_logic_not(&self, node: &XmlNode) -> Result<Option<Condition>, ParseError> {
        if node.children.is_empty() {
            error!("Empty <not> condition in {}", self.file);
            return Ok(None);
        }
        Ok(self
            .parse_condition(&node.children, 0)?
            .map(|inner| Condition::Not(Box::new(inner))))
    }

    fn parse_player_condition(&self, n: &XmlNode) -> Result<Option<Condition>, ParseError> {
        let mut cond: Option<Condition> = None;
        for (name, raw) in &n.attrs {
            let raw = raw.as_str();
            let state = |check: PlayerState| Condition::PlayerState {
                check,
                required: parse_boolean(raw),
            };
            let leaf = match name.to_lowercase().as_str() {
                "races" => {
                    let mut races = BTreeSet::new();
                    for part in java_split(raw, ',') {
                        races.insert(NpcRace::from_name(part).ok_or_else(|| no_enum_constant("Race", part))?);
                    }
                    Condition::PlayerRace { races }
                }
                "level" => Condition::PlayerLevel {
                    level: self.decode_value(raw)?,
                },
                "levelrange" => match self.level_range(raw)? {
                    Some(levels) => Condition::PlayerLevelRange { levels },
                    None => continue,
                },
                "resting" => state(PlayerState::Resting),
                "flying" => state(PlayerState::Flying),
                "moving" => state(PlayerState::Moving),
                "running" => state(PlayerState::Running),
                "standing" => state(PlayerState::Standing),
                "behind" => state(PlayerState::Behind),
                "front" => state(PlayerState::Front),
                "chaotic" => state(PlayerState::Chaotic),
                "olympiad" => state(PlayerState::Olympiad),
                "ishero" => Condition::PlayerIsHero {
                    value: parse_boolean(raw),
                },
                "ispvpflagged" => Condition::PlayerIsPvpFlagged {
                    value: parse_boolean(raw),
                },
                "transformationid" => Condition::PlayerTransformationId { id: parse_i32(raw)? },
                "isridingstrider" => Condition::PlayerIsRidingStrider {
                    value: parse_boolean(raw),
                },
                "hp" => Condition::PlayerHp {
                    hp: self.decode_plain(raw)?,
                },
                "mp" => Condition::PlayerMp {
                    mp: self.decode_plain(raw)?,
                },
                "cp" => Condition::PlayerCp {
                    cp: self.decode_plain(raw)?,
                },
                "grade" => Condition::PlayerGrade {
                    value: self.decode_value(raw)?,
                },
                "pkcount" => Condition::PlayerPkCount {
                    pk: self.decode_value(raw)?,
                },
                "siegezone" => Condition::SiegeZone {
                    value: self.decode_plain(raw)?,
                    own: true,
                },
                "siegeside" => Condition::PlayerSiegeSide {
                    siege_side: self.decode_plain(raw)?,
                },
                "charges" => Condition::PlayerCharges {
                    charges: self.decode_value(raw)?,
                },
                "souls" => Condition::PlayerSouls {
                    souls: self.decode_value(raw)?,
                },
                "weight" => Condition::PlayerWeight {
                    weight: self.decode_plain(raw)?,
                },
                "invsize" => Condition::PlayerInvSize {
                    size: self.decode_plain(raw)?,
                },
                "isclanleader" => Condition::PlayerIsClanLeader {
                    value: parse_boolean(raw),
                },
                "pledgeclass" => Condition::PlayerPledgeClass {
                    pledge_class: self.decode_plain(raw)?,
                },
                "clanhall" => Condition::PlayerHasClanHall {
                    clan_hall: self.int_list(raw)?,
                },
                "fort" => Condition::PlayerHasFort {
                    fort: self.decode_plain(raw)?,
                },
                "castle" => Condition::PlayerHasCastle {
                    castle: self.decode_plain(raw)?,
                },
                "sex" => Condition::PlayerSex {
                    sex: self.decode_plain(raw)?,
                },
                "flymounted" => Condition::PlayerFlyMounted {
                    value: parse_boolean(raw),
                },
                "vehiclemounted" => Condition::PlayerVehicleMounted {
                    value: parse_boolean(raw),
                },
                "landingzone" => Condition::PlayerLandingZone {
                    value: parse_boolean(raw),
                },
                "active_effect_id" => Condition::PlayerActiveEffectId {
                    effect_id: self.decode_value(raw)?,
                    effect_lvl: -1,
                },
                "active_effect_id_lvl" => {
                    let (effect_id, effect_lvl) = self.id_and_level(raw)?;
                    Condition::PlayerActiveEffectId {
                        effect_id,
                        effect_lvl,
                    }
                }
                "active_skill_id" => Condition::PlayerActiveSkillId {
                    skill_id: self.decode_value(raw)?,
                    skill_level: -1,
                },
                "active_skill_id_lvl" => {
                    let (skill_id, skill_level) = self.id_and_level(raw)?;
                    Condition::PlayerActiveSkillId {
                        skill_id,
                        skill_level,
                    }
                }
                "class_id_restriction" => Condition::PlayerClassIdRestriction {
                    class_ids: self.int_set(raw)?,
                },
                "subclass" => Condition::PlayerSubclass {
                    value: parse_boolean(raw),
                },
                "instanceid" => Condition::PlayerInstanceId {
                    instance_ids: self.int_set(raw)?,
                },
                // NOTE(java-quirk): `Integer.decode` of the raw value, no `getValue`.
                "agathionid" => Condition::PlayerAgathionId {
                    agathion_id: decode(raw)?,
                },
                "cloakstatus" => Condition::PlayerCloakStatus {
                    value: parse_boolean(raw),
                },
                "haspet" => Condition::PlayerHasPet {
                    control_item_ids: self.int_list(raw)?,
                },
                "servitornpcid" => Condition::PlayerServitorNpcId {
                    npc_ids: self.int_list(raw)?,
                },
                "npcidradius" => {
                    let parts = tokens(raw, ',');
                    if parts.len() != 3 {
                        continue;
                    }
                    let mut npc_ids = BTreeSet::new();
                    for id in java_split(parts[0], ';') {
                        npc_ids.insert(parse_i32(&self.value(id)?)?);
                    }
                    Condition::PlayerRangeFromNpc {
                        npc_ids,
                        radius: parse_i32(parts[1])?,
                        value: parse_boolean(parts[2]),
                    }
                }
                "callpc" => Condition::PlayerCallPc {
                    value: parse_boolean(raw),
                },
                "cancreatebase" => Condition::PlayerCanCreateBase {
                    value: parse_boolean(raw),
                },
                "cancreateoutpost" => Condition::PlayerCanCreateOutpost {
                    value: parse_boolean(raw),
                },
                "canescape" => Condition::PlayerCanEscape {
                    value: parse_boolean(raw),
                },
                "canrefuelairship" => Condition::PlayerCanRefuelAirship {
                    value: parse_i32(raw)?,
                },
                "canresurrect" => Condition::PlayerCanResurrect {
                    value: parse_boolean(raw),
                },
                "cansummon" => Condition::PlayerCanSummon {
                    value: parse_boolean(raw),
                },
                "cansummonsiegegolem" => Condition::PlayerCanSummonSiegeGolem {
                    value: parse_boolean(raw),
                },
                "cansweep" => Condition::PlayerCanSweep {
                    value: parse_boolean(raw),
                },
                "cantakecastle" => Condition::PlayerCanTakeCastle,
                "cantakefort" => Condition::PlayerCanTakeFort {
                    value: parse_boolean(raw),
                },
                "cantransform" => Condition::PlayerCanTransform {
                    value: parse_boolean(raw),
                },
                "canuntransform" => Condition::PlayerCanUntransform {
                    value: parse_boolean(raw),
                },
                "insidezoneid" => Condition::PlayerInsideZoneId {
                    zones: self.int_set(raw)?,
                },
                "checkabnormal" => {
                    if raw.contains(',') {
                        let values = java_split(raw, ',');
                        let first = values.first().copied().unwrap_or("");
                        let abnormal_type = AbnormalType::from_name(first)
                            .ok_or_else(|| no_enum_constant("AbnormalType", first))?;
                        let level = values
                            .get(1)
                            .ok_or_else(|| ParseError::invalid("Index 1 out of bounds for length 1"))?;
                        Condition::PlayerCheckAbnormal {
                            abnormal_type,
                            level: self.decode_value(level)?,
                        }
                    } else {
                        Condition::PlayerCheckAbnormal {
                            abnormal_type: AbnormalType::from_name(raw)
                                .ok_or_else(|| no_enum_constant("AbnormalType", raw))?,
                            level: -1,
                        }
                    }
                }
                "categorytype" => {
                    let mut category_types = BTreeSet::new();
                    for value in java_split(raw, ',') {
                        let value = self.plain_value(value)?;
                        category_types.insert(
                            CategoryType::from_name(value)
                                .ok_or_else(|| no_enum_constant("CategoryType", value))?,
                        );
                    }
                    Condition::CategoryType { category_types }
                }
                _ => continue,
            };
            cond = Some(join_and(cond, leaf));
        }
        if cond.is_none() {
            error!("Unrecognized <player> condition in {}", self.file);
        }
        Ok(cond)
    }

    fn parse_target_condition(&self, n: &XmlNode) -> Result<Option<Condition>, ParseError> {
        let mut cond: Option<Condition> = None;
        for (name, raw) in &n.attrs {
            let raw = raw.as_str();
            let leaf = match name.to_lowercase().as_str() {
                "aggro" => Condition::TargetAggro {
                    is_aggro: parse_boolean(raw),
                },
                "siegezone" => Condition::SiegeZone {
                    value: self.decode_plain(raw)?,
                    own: false,
                },
                "level" => Condition::TargetLevel {
                    level: self.decode_value(raw)?,
                },
                "levelrange" => match self.level_range(raw)? {
                    Some(levels) => Condition::TargetLevelRange { levels },
                    None => continue,
                },
                "mypartyexceptme" => Condition::TargetMyPartyExceptMe {
                    value: parse_boolean(raw),
                },
                "playable" => Condition::TargetPlayable,
                "class_id_restriction" => Condition::TargetClassIdRestriction {
                    class_ids: self.int_set(raw)?,
                },
                "active_effect_id" => Condition::TargetActiveEffectId {
                    effect_id: self.decode_value(raw)?,
                    effect_lvl: -1,
                },
                "active_effect_id_lvl" => {
                    let (effect_id, effect_lvl) = self.id_and_level(raw)?;
                    Condition::TargetActiveEffectId {
                        effect_id,
                        effect_lvl,
                    }
                }
                "active_skill_id" => Condition::TargetActiveSkillId {
                    skill_id: self.decode_value(raw)?,
                    skill_level: -1,
                },
                "active_skill_id_lvl" => {
                    let (skill_id, skill_level) = self.id_and_level(raw)?;
                    Condition::TargetActiveSkillId {
                        skill_id,
                        skill_level,
                    }
                }
                "abnormal" => Condition::TargetAbnormal {
                    abnormal_id: self.decode_value(raw)?,
                },
                "mindistance" => Condition::MinDistance {
                    distance: self.decode_plain(raw)?,
                },
                "race" => Condition::TargetRace {
                    race: NpcRace::from_name(raw).ok_or_else(|| no_enum_constant("Race", raw))?,
                },
                "using" => {
                    let mut weapon_mask = 0;
                    for item in tokens(raw, ',') {
                        let item = item.trim();
                        if let Some(wt) = WeaponType::ALL.iter().find(|wt| wt.name() == item) {
                            weapon_mask |= wt.mask();
                        }
                        if let Some(at) = ArmorType::ALL.iter().find(|at| at.name() == item) {
                            weapon_mask |= at.mask();
                        }
                    }
                    Condition::TargetUsesWeaponKind { weapon_mask }
                }
                "npcid" => Condition::TargetNpcId {
                    npc_ids: self.int_set(raw)?,
                },
                "npctype" => {
                    let values = self.value(raw)?;
                    let mut npc_types = Vec::new();
                    // NOTE(java-quirk): `trim()` of the whole value only, the parts are not trimmed.
                    for part in java_split(values.trim(), ',') {
                        npc_types.push(
                            InstanceType::from_name(part)
                                .ok_or_else(|| no_enum_constant("InstanceType", part))?,
                        );
                    }
                    Condition::TargetNpcType { npc_types }
                }
                "weight" => Condition::TargetWeight {
                    weight: self.decode_plain(raw)?,
                },
                "invsize" => Condition::TargetInvSize {
                    size: self.decode_plain(raw)?,
                },
                _ => continue,
            };
            cond = Some(join_and(cond, leaf));
        }
        if cond.is_none() {
            error!("Unrecognized <target> condition in {}", self.file);
        }
        Ok(cond)
    }

    fn parse_using_condition(&self, n: &XmlNode) -> Result<Option<Condition>, ParseError> {
        let mut cond: Option<Condition> = None;
        for (name, raw) in &n.attrs {
            let raw = raw.as_str();
            let leaf = match name.to_lowercase().as_str() {
                "kind" => {
                    let mut mask = 0;
                    for item in tokens(raw, ',') {
                        let item = item.trim();
                        let old = mask;
                        for wt in WeaponType::ALL.iter().filter(|wt| wt.name() == item) {
                            mask |= wt.mask();
                        }
                        for at in ArmorType::ALL.iter().filter(|at| at.name() == item) {
                            mask |= at.mask();
                        }
                        if old == mask {
                            info!("[parseUsingCondition=\"kind\"] Unknown item type name: {item}");
                        }
                    }
                    Condition::UsingItemType { mask }
                }
                "slot" => {
                    let mut mask = 0;
                    for item in tokens(raw, ',') {
                        let item = item.trim();
                        let old = mask;
                        if let Some(body_part) = BodyPart::from_name(item) {
                            mask |= body_part.mask();
                        }
                        if old == mask {
                            info!("[parseUsingCondition=\"slot\"] Unknown item slot name: {item}");
                        }
                    }
                    Condition::UsingSlotType { mask }
                }
                "skill" => Condition::UsingSkill {
                    skill_id: parse_i32(raw)?,
                },
                "slotitem" => {
                    let parts = tokens(raw, ';');
                    let mut next = parts.iter();
                    let missing = || ParseError::invalid("NoSuchElementException");
                    let item_id = parse_i32(next.next().ok_or_else(missing)?.trim())?;
                    let slot = parse_i32(next.next().ok_or_else(missing)?.trim())?;
                    let enchant_level = match next.next() {
                        Some(enchant) => parse_i32(enchant.trim())?,
                        None => 0,
                    };
                    Condition::SlotItemId {
                        slot,
                        item_id,
                        enchant_level,
                    }
                }
                "weaponchange" => Condition::ChangeWeapon {
                    required: parse_boolean(raw),
                },
                _ => continue,
            };
            cond = Some(join_and(cond, leaf));
        }
        if cond.is_none() {
            error!("Unrecognized <using> condition in {}", self.file);
        }
        Ok(cond)
    }

    fn parse_game_condition(&self, n: &XmlNode) -> Result<Option<Condition>, ParseError> {
        let mut cond: Option<Condition> = None;
        for (name, raw) in &n.attrs {
            let raw = raw.as_str();
            // Three independent `if`s in Java, not a switch; the effect is the same.
            let leaf = if name.eq_ignore_ascii_case("skill") {
                Condition::WithSkill {
                    skill: parse_boolean(raw),
                }
            } else if name.eq_ignore_ascii_case("night") {
                Condition::GameTime {
                    required: parse_boolean(raw),
                }
            } else if name.eq_ignore_ascii_case("chance") {
                Condition::GameChance {
                    chance: self.decode_plain(raw)?,
                }
            } else {
                continue;
            };
            cond = Some(join_and(cond, leaf));
        }
        if cond.is_none() {
            error!("Unrecognized <game> condition in {}", self.file);
        }
        Ok(cond)
    }

    /// `levelRange="min;max"`: only a value with exactly two parts makes a condition; any other
    /// value is silently ignored by Java.
    fn level_range(&self, raw: &str) -> Result<Option<[i32; 2]>, ParseError> {
        let value = self.value(raw)?;
        let range = java_split(&value, ';');
        if range.len() != 2 {
            return Ok(None);
        }
        Ok(Some([decode(range[0])?, decode(range[1])?]))
    }

    /// `StringTokenizer(value, ",")`, each token trimmed, `Integer.decode(getValue(item, null))`.
    fn int_list(&self, raw: &str) -> Result<Vec<i32>, ParseError> {
        tokens(raw, ',')
            .into_iter()
            .map(|item| self.decode_plain(item.trim()))
            .collect()
    }

    /// Same as [`Self::int_list`] into a `Set<Integer>`.
    fn int_set(&self, raw: &str) -> Result<BTreeSet<i32>, ParseError> {
        Ok(self.int_list(raw)?.into_iter().collect())
    }

    /// `active_*_id_lvl="id,level"`: `val = getValue(v)`, then each half through `getValue` again.
    fn id_and_level(&self, raw: &str) -> Result<(i32, i32), ParseError> {
        let value = self.value(raw)?;
        let parts = java_split(&value, ',');
        let part = |index: usize| {
            parts.get(index).copied().ok_or_else(|| {
                ParseError::invalid(format!("Index {index} out of bounds for length {}", parts.len()))
            })
        };
        let id = self.decode_value(part(0)?)?;
        let level = self.decode_value(part(1)?)?;
        Ok((id, level))
    }
}

/// `Integer.decode(text)`, or Java's `NumberFormatException` text.
fn decode(text: &str) -> Result<i32, ParseError> {
    decode_i32(text).ok_or_else(|| ParseError::invalid(number_format_message(text)))
}

/// `Integer.parseInt(text)`.
fn parse_i32(text: &str) -> Result<i32, ParseError> {
    parse_int(text).ok_or_else(|| ParseError::invalid(number_format_message(text)))
}

/// `IllegalArgumentException: No enum constant ...` from `Enum.valueOf`.
fn no_enum_constant(class: &str, name: &str) -> ParseError {
    ParseError::invalid(format!("No enum constant {class}.{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::node::parse_xml;

    fn no_tables(_: &str) -> Option<String> {
        None
    }

    fn parse(xml: &str) -> Result<Option<Condition>, ParseError> {
        let node = parse_xml(&format!("<conditions>{xml}</conditions>")).unwrap();
        ConditionParser::new("test.xml", &no_tables).parse_condition(&node.children, 0)
    }

    fn ok(xml: &str) -> Condition {
        parse(xml).unwrap().unwrap()
    }

    #[test]
    fn player_leaves() {
        assert_eq!(
            ok(r#"<player races="HUMAN,ELF"/>"#),
            Condition::PlayerRace {
                races: [NpcRace::Human, NpcRace::Elf].into_iter().collect()
            }
        );
        assert_eq!(
            ok(r#"<player level="40"/>"#),
            Condition::PlayerLevel { level: 40 }
        );
        assert_eq!(
            ok(r#"<player level="0x28"/>"#),
            Condition::PlayerLevel { level: 40 }
        );
        assert_eq!(
            ok(r#"<player levelRange="20;39"/>"#),
            Condition::PlayerLevelRange { levels: [20, 39] }
        );
        assert_eq!(ok(r#"<player hp="60"/>"#), Condition::PlayerHp { hp: 60 });
        assert_eq!(ok(r#"<player mp="5"/>"#), Condition::PlayerMp { mp: 5 });
        assert_eq!(ok(r#"<player cp="5"/>"#), Condition::PlayerCp { cp: 5 });
        assert_eq!(
            ok(r#"<player weight="80"/>"#),
            Condition::PlayerWeight { weight: 80 }
        );
        assert_eq!(
            ok(r#"<player invSize="10"/>"#),
            Condition::PlayerInvSize { size: 10 }
        );
        assert_eq!(
            ok(r#"<player pledgeClass="5"/>"#),
            Condition::PlayerPledgeClass { pledge_class: 5 }
        );
        assert_eq!(ok(r#"<player sex="1"/>"#), Condition::PlayerSex { sex: 1 });
        assert_eq!(ok(r#"<player pkCount="4"/>"#), Condition::PlayerPkCount { pk: 4 });
        assert_eq!(
            ok(r#"<player castle="3"/>"#),
            Condition::PlayerHasCastle { castle: 3 }
        );
        assert_eq!(ok(r#"<player fort="3"/>"#), Condition::PlayerHasFort { fort: 3 });
        assert_eq!(
            ok(r#"<player clanHall="21, 22"/>"#),
            Condition::PlayerHasClanHall {
                clan_hall: vec![21, 22]
            }
        );
        assert_eq!(
            ok(r#"<player instanceId="1,2,1"/>"#),
            Condition::PlayerInstanceId {
                instance_ids: [1, 2].into_iter().collect()
            }
        );
        assert_eq!(
            ok(r#"<player insideZoneId="7"/>"#),
            Condition::PlayerInsideZoneId {
                zones: [7].into_iter().collect()
            }
        );
        assert_eq!(
            ok(r#"<player class_id_restriction="1,2"/>"#),
            Condition::PlayerClassIdRestriction {
                class_ids: [1, 2].into_iter().collect()
            }
        );
        assert_eq!(
            ok(r#"<player categoryType="FIGHTER_GROUP,MAGE_GROUP"/>"#),
            Condition::CategoryType {
                category_types: [CategoryType::FighterGroup, CategoryType::MageGroup]
                    .into_iter()
                    .collect()
            }
        );
    }

    #[test]
    fn player_booleans_and_states() {
        assert_eq!(
            ok(r#"<player flyMounted="false"/>"#),
            Condition::PlayerFlyMounted { value: false }
        );
        assert_eq!(
            ok(r#"<player isHero="true"/>"#),
            Condition::PlayerIsHero { value: true }
        );
        // Anything but "true" is false.
        assert_eq!(
            ok(r#"<player isHero="1"/>"#),
            Condition::PlayerIsHero { value: false }
        );
        assert_eq!(
            ok(r#"<player subclass="TRUE"/>"#),
            Condition::PlayerSubclass { value: true }
        );
        assert_eq!(
            ok(r#"<player cloakStatus="true"/>"#),
            Condition::PlayerCloakStatus { value: true }
        );
        assert_eq!(
            ok(r#"<player isPvpFlagged="false"/>"#),
            Condition::PlayerIsPvpFlagged { value: false }
        );
        assert_eq!(
            ok(r#"<player vehicleMounted="true"/>"#),
            Condition::PlayerVehicleMounted { value: true }
        );
        assert_eq!(
            ok(r#"<player chaotic="true"/>"#),
            Condition::PlayerState {
                check: PlayerState::Chaotic,
                required: true
            }
        );
        assert_eq!(
            ok(r#"<player resting="false"/>"#),
            Condition::PlayerState {
                check: PlayerState::Resting,
                required: false
            }
        );
        assert_eq!(
            ok(r#"<player canTakeCastle="false"/>"#),
            Condition::PlayerCanTakeCastle
        );
    }

    #[test]
    fn attribute_names_match_case_insensitively_and_siege_zone_has_a_capital() {
        // The shipped item files write `SiegeZone`.
        assert_eq!(
            ok(r#"<player SiegeZone="2"/>"#),
            Condition::SiegeZone { value: 2, own: true }
        );
        assert_eq!(ok(r#"<player LEVEL="3"/>"#), Condition::PlayerLevel { level: 3 });
    }

    #[test]
    fn several_attributes_join_in_alphabetical_order() {
        // Xerces keeps attributes sorted by name: levelRange < races.
        assert_eq!(
            ok(r#"<player races="HUMAN" level="40"/>"#),
            Condition::And(vec![
                Condition::PlayerLevel { level: 40 },
                Condition::PlayerRace {
                    races: [NpcRace::Human].into_iter().collect()
                },
            ])
        );
        // joinAnd appends to an existing and: three leaves are one flat and.
        let three = ok(r#"<player level="40" hp="5" mp="6"/>"#);
        assert_eq!(
            three,
            Condition::And(vec![
                Condition::PlayerHp { hp: 5 },
                Condition::PlayerLevel { level: 40 },
                Condition::PlayerMp { mp: 6 },
            ])
        );
    }

    #[test]
    fn logic_nodes() {
        let c = ok(
            r#"<and><player level="40"/><or><player sex="0"/><player sex="1"/></or><not><player isHero="true"/></not></and>"#,
        );
        assert_eq!(
            c,
            Condition::And(vec![
                Condition::PlayerLevel { level: 40 },
                Condition::Or(vec![
                    Condition::PlayerSex { sex: 0 },
                    Condition::PlayerSex { sex: 1 }
                ]),
                Condition::Not(Box::new(Condition::PlayerIsHero { value: true })),
            ])
        );
        // Unknown children are dropped.
        assert_eq!(
            ok("<and><foo/><player level=\"1\"/></and>"),
            Condition::And(vec![Condition::PlayerLevel { level: 1 }])
        );
        // Empty and/or are kept (Java only logs).
        assert_eq!(ok("<and/>"), Condition::And(vec![]));
        assert_eq!(ok("<or/>"), Condition::Or(vec![]));
        // A <not> without a known child has no condition.
        assert_eq!(parse("<not/>").unwrap(), None);
        assert_eq!(parse("<not><foo/></not>").unwrap(), None);
    }

    #[test]
    fn value_elements_are_skipped_and_unknown_elements_give_none() {
        let node = parse_xml(r#"<c><value level="1"/><player level="5"/></c>"#).unwrap();
        let parser = ConditionParser::new("t.xml", &no_tables);
        assert_eq!(
            parser.parse_condition(&node.children, 0).unwrap(),
            Some(Condition::PlayerLevel { level: 5 })
        );
        assert_eq!(parse("<foo/>").unwrap(), None);
        assert_eq!(parse("").unwrap(), None);
        assert_eq!(parse("<player/>").unwrap(), None);
        assert_eq!(parse(r#"<player unknownAttribute="1"/>"#).unwrap(), None);
    }

    #[test]
    fn value_siblings_inside_logic_are_visited_twice() {
        // NOTE(java-quirk) of parseLogicAnd: the <value> child hands over to the next sibling,
        // which the loop then visits again.
        let c = ok(r#"<and><value/><player level="5"/></and>"#);
        assert_eq!(
            c,
            Condition::And(vec![
                Condition::PlayerLevel { level: 5 },
                Condition::PlayerLevel { level: 5 }
            ])
        );
    }

    #[test]
    fn target_leaves() {
        assert_eq!(
            ok(r#"<target levelRange="1;50"/>"#),
            Condition::TargetLevelRange { levels: [1, 50] }
        );
        assert_eq!(ok(r#"<target level="5"/>"#), Condition::TargetLevel { level: 5 });
        assert_eq!(
            ok(r#"<target aggro="true"/>"#),
            Condition::TargetAggro { is_aggro: true }
        );
        assert_eq!(ok(r#"<target playable="true"/>"#), Condition::TargetPlayable);
        assert_eq!(
            ok(r#"<target myPartyExceptMe="true"/>"#),
            Condition::TargetMyPartyExceptMe { value: true }
        );
        assert_eq!(
            ok(r#"<target race="UNDEAD"/>"#),
            Condition::TargetRace {
                race: NpcRace::Undead
            }
        );
        assert_eq!(
            ok(r#"<target abnormal="12"/>"#),
            Condition::TargetAbnormal { abnormal_id: 12 }
        );
        assert_eq!(
            ok(r#"<target minDistance="30"/>"#),
            Condition::MinDistance { distance: 30 }
        );
        assert_eq!(
            ok(r#"<target npcId="1,2"/>"#),
            Condition::TargetNpcId {
                npc_ids: [1, 2].into_iter().collect()
            }
        );
        assert_eq!(
            ok(r#"<target npcType="Monster,Folk"/>"#),
            Condition::TargetNpcType {
                npc_types: vec![InstanceType::Monster, InstanceType::Folk]
            }
        );
        assert_eq!(
            ok(r#"<target weight="1"/>"#),
            Condition::TargetWeight { weight: 1 }
        );
        assert_eq!(
            ok(r#"<target invSize="2"/>"#),
            Condition::TargetInvSize { size: 2 }
        );
        assert_eq!(
            ok(r#"<target siegeZone="2"/>"#),
            Condition::SiegeZone { value: 2, own: false }
        );
        assert_eq!(
            ok(r#"<target active_skill_id_lvl="10,2"/>"#),
            Condition::TargetActiveSkillId {
                skill_id: 10,
                skill_level: 2
            }
        );
        assert_eq!(
            ok(r#"<target active_effect_id="7"/>"#),
            Condition::TargetActiveEffectId {
                effect_id: 7,
                effect_lvl: -1
            }
        );
        let mask = WeaponType::Sword.mask() | WeaponType::Bow.mask() | ArmorType::Shield.mask();
        assert_eq!(
            ok(r#"<target using="SWORD, BOW,SHIELD,NOPE"/>"#),
            Condition::TargetUsesWeaponKind { weapon_mask: mask }
        );
    }

    #[test]
    fn using_and_game_leaves() {
        let mask = WeaponType::Dagger.mask() | ArmorType::Light.mask();
        assert_eq!(
            ok(r#"<using kind="DAGGER,LIGHT"/>"#),
            Condition::UsingItemType { mask }
        );
        // "NONE" is both a weapon type and an armor type.
        assert_eq!(
            ok(r#"<using kind="NONE"/>"#),
            Condition::UsingItemType {
                mask: WeaponType::None.mask() | ArmorType::None.mask()
            }
        );
        // Unknown names are only logged.
        assert_eq!(
            ok(r#"<using kind="NOPE"/>"#),
            Condition::UsingItemType { mask: 0 }
        );
        assert_eq!(
            ok(r#"<using slot="chest,legs"/>"#),
            Condition::UsingSlotType {
                mask: BodyPart::Chest.mask() | BodyPart::Legs.mask()
            }
        );
        assert_eq!(
            ok(r#"<using skill="1204"/>"#),
            Condition::UsingSkill { skill_id: 1204 }
        );
        assert_eq!(
            ok(r#"<using slotItem="5; 7;3"/>"#),
            Condition::SlotItemId {
                slot: 7,
                item_id: 5,
                enchant_level: 3
            }
        );
        assert_eq!(
            ok(r#"<using slotItem="5;7"/>"#),
            Condition::SlotItemId {
                slot: 7,
                item_id: 5,
                enchant_level: 0
            }
        );
        assert_eq!(
            ok(r#"<using weaponChange="true"/>"#),
            Condition::ChangeWeapon { required: true }
        );
        assert_eq!(
            ok(r#"<game skill="true"/>"#),
            Condition::WithSkill { skill: true }
        );
        assert_eq!(
            ok(r#"<game night="false"/>"#),
            Condition::GameTime { required: false }
        );
        assert_eq!(ok(r#"<game chance="30"/>"#), Condition::GameChance { chance: 30 });
    }

    #[test]
    fn other_player_forms() {
        assert_eq!(
            ok(r#"<player npcIdRadius="1;2,150,true"/>"#),
            Condition::PlayerRangeFromNpc {
                npc_ids: [1, 2].into_iter().collect(),
                radius: 150,
                value: true
            }
        );
        // Not exactly three tokens: ignored, and with nothing else, no condition.
        assert_eq!(parse(r#"<player npcIdRadius="1;2,150"/>"#).unwrap(), None);
        assert_eq!(
            ok(r#"<player checkAbnormal="STUN"/>"#),
            Condition::PlayerCheckAbnormal {
                abnormal_type: AbnormalType::Stun,
                level: -1
            }
        );
        assert_eq!(
            ok(r#"<player checkAbnormal="STUN,2"/>"#),
            Condition::PlayerCheckAbnormal {
                abnormal_type: AbnormalType::Stun,
                level: 2
            }
        );
        assert_eq!(
            ok(r#"<player active_effect_id_lvl="5,3"/>"#),
            Condition::PlayerActiveEffectId {
                effect_id: 5,
                effect_lvl: 3
            }
        );
        assert_eq!(
            ok(r#"<player active_skill_id="9"/>"#),
            Condition::PlayerActiveSkillId {
                skill_id: 9,
                skill_level: -1
            }
        );
        assert_eq!(
            ok(r#"<player agathionId="0x10"/>"#),
            Condition::PlayerAgathionId { agathion_id: 16 }
        );
        assert_eq!(
            ok(r#"<player transformationId="7"/>"#),
            Condition::PlayerTransformationId { id: 7 }
        );
        assert_eq!(
            ok(r#"<player canRefuelAirship="3"/>"#),
            Condition::PlayerCanRefuelAirship { value: 3 }
        );
        assert_eq!(
            ok(r#"<player haspet="1,2"/>"#),
            Condition::PlayerHasPet {
                control_item_ids: vec![1, 2]
            }
        );
        assert_eq!(
            ok(r#"<player servitorNpcId="3"/>"#),
            Condition::PlayerServitorNpcId { npc_ids: vec![3] }
        );
        assert_eq!(
            ok(r#"<player charges="2"/>"#),
            Condition::PlayerCharges { charges: 2 }
        );
        assert_eq!(ok(r#"<player souls="2"/>"#), Condition::PlayerSouls { souls: 2 });
        assert_eq!(ok(r#"<player grade="2"/>"#), Condition::PlayerGrade { value: 2 });
        assert_eq!(
            ok(r#"<player siegeSide="2"/>"#),
            Condition::PlayerSiegeSide { siege_side: 2 }
        );
        assert_eq!(
            ok(r#"<player landingZone="true"/>"#),
            Condition::PlayerLandingZone { value: true }
        );
    }

    #[test]
    fn bad_values_fail_like_java() {
        assert!(parse(r#"<player level="abc"/>"#).is_err());
        assert!(parse(r#"<player level=""/>"#).is_err());
        assert!(parse(r#"<player races="HUMAN, ELF"/>"#).is_err()); // not trimmed
        assert!(parse(r#"<player races="human"/>"#).is_err()); // valueOf is case sensitive
        assert!(parse(r#"<player races=""/>"#).is_err());
        assert!(parse(r#"<target race="NOPE"/>"#).is_err());
        assert!(parse(r#"<player checkAbnormal="NOPE"/>"#).is_err());
        assert!(parse(r#"<player checkAbnormal="STUN,"/>"#).is_err());
        assert!(parse(r#"<player categoryType="NOPE"/>"#).is_err());
        assert!(parse(r#"<target npcType="monster"/>"#).is_err());
        assert!(parse(r#"<using slotItem="5"/>"#).is_err());
        assert!(parse(r#"<using skill="x"/>"#).is_err());
        assert!(parse(r#"<player active_skill_id_lvl="5"/>"#).is_err());
        // transformationId is parseInt, not decode.
        assert!(parse(r#"<player transformationId="0x10"/>"#).is_err());
        // levelRange with the wrong number of parts is ignored, not an error.
        assert_eq!(parse(r#"<player levelRange="5"/>"#).unwrap(), None);
        assert_eq!(parse(r#"<player levelRange="1;2;3"/>"#).unwrap(), None);
        // A bad number in a levelRange part is an error.
        assert!(parse(r#"<player levelRange="1;x"/>"#).is_err());
    }

    #[test]
    fn table_references_use_the_resolver_only_where_java_passes_a_template() {
        let tables = |name: &str| match name {
            "#lvl" => Some("40".to_owned()),
            "#range" => Some("5;10".to_owned()),
            "#pair" => Some("7,2".to_owned()),
            _ => None,
        };
        let parser = ConditionParser::new("skill.xml", &tables);
        let run = |xml: &str| {
            let node = parse_xml(&format!("<c>{xml}</c>")).unwrap();
            parser.parse_condition(&node.children, 0)
        };
        // level, levelRange, active_*_lvl take the template.
        assert_eq!(
            run(r##"<player level="#lvl"/>"##).unwrap(),
            Some(Condition::PlayerLevel { level: 40 })
        );
        assert_eq!(
            run(r##"<target levelRange="#range"/>"##).unwrap(),
            Some(Condition::TargetLevelRange { levels: [5, 10] })
        );
        assert_eq!(
            run(r##"<player active_skill_id_lvl="#pair"/>"##).unwrap(),
            Some(Condition::PlayerActiveSkillId {
                skill_id: 7,
                skill_level: 2
            })
        );
        // hp, weight, ... use getValue(v, null): always an error for '#'.
        assert!(run(r##"<player hp="#lvl"/>"##).is_err());
        assert!(run(r##"<player weight="#lvl"/>"##).is_err());
        // An unknown table is an error.
        assert!(run(r##"<player level="#nope"/>"##).is_err());
        // Items: the resolver finds nothing, so any '#' value fails.
        assert!(parse(r##"<player level="#lvl"/>"##).is_err());
    }

    #[test]
    fn conditions_block_messages() {
        let parser = ConditionParser::new("t.xml", &no_tables);
        let block = |xml: &str| parse_xml(xml).unwrap();
        let m = parser
            .parse_conditions_block(&block(
                r#"<conditions msgId="113" addName="1"><player level="5"/></conditions>"#,
            ))
            .unwrap()
            .unwrap();
        assert_eq!(m.condition, Condition::PlayerLevel { level: 5 });
        assert_eq!(
            m.message,
            ConditionMessage {
                message: None,
                message_id: 113,
                add_name: true
            }
        );
        // msg wins over msgId.
        let m = parser
            .parse_conditions_block(&block(
                r#"<conditions msg="Hello" msgId="9"><player level="5"/></conditions>"#,
            ))
            .unwrap()
            .unwrap();
        assert_eq!(m.message.message.as_deref(), Some("Hello"));
        assert_eq!(m.message.message_id, 0);
        // addName needs a positive msgId and a msgId at all.
        let m = parser
            .parse_conditions_block(&block(
                r#"<conditions msgId="0" addName="1"><player level="5"/></conditions>"#,
            ))
            .unwrap()
            .unwrap();
        assert!(!m.message.add_name);
        let m = parser
            .parse_conditions_block(&block(
                r#"<conditions addName="1"><player level="5"/></conditions>"#,
            ))
            .unwrap()
            .unwrap();
        assert_eq!(m.message, ConditionMessage::default());
        // No message attributes, no condition.
        assert_eq!(
            parser.parse_conditions_block(&block("<conditions/>")).unwrap(),
            None
        );
        assert_eq!(
            parser
                .parse_conditions_block(&block(r#"<conditions msgId="5"><foo/></conditions>"#))
                .unwrap(),
            None
        );
        // A msgId that is not a number fails when there is a condition.
        assert!(
            parser
                .parse_conditions_block(&block(
                    r#"<conditions msgId="x"><player level="5"/></conditions>"#
                ))
                .is_err()
        );
    }
}
