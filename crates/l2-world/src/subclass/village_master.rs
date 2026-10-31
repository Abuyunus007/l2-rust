//! The `Subclass` bypass of the village masters: the menu, adding, switching and replacing a
//! subclass, and the race and class rules of each master.
// Java: entity/actor/instance/VillageMaster.java (onBypassFeedback's `Subclass` branch,
//       getSubClassMenu, getSubClassFail, checkQuests, getAvailableSubClasses, getSubclasses,
//       isValidNewSubClass, checkVillageMaster*, the subclass sets)
// Java: entity/actor/instance/VillageMaster{DElf,Dwarf,Fighter,Kamael,Mystic,Orc,Priest}.java
// Java: data/xml/CategoryData.java (THIRD_CLASS_GROUP, FOURTH_CLASS_GROUP, FIGHTER_GROUP,
//       MAGE_GROUP, CLERIC_GROUP of `categoryData.xml`)

use l2_data::{ClassId, Race};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use super::{AfterSwitch, SubClassHolder};
use crate::flood::FloodKind;
use crate::world::World;

/// `categoryData.xml` `THIRD_CLASS_GROUP`: the classes of the 2nd class transfer.
const THIRD_CLASS_GROUP: [u16; 36] = [
    2, 3, 5, 6, 8, 9, 12, 13, 14, 16, 17, 20, 21, 23, 24, 27, 28, 30, 33, 34, 36, 37, 40, 41, 43, 46, 48, 51,
    52, 55, 57, 127, 128, 129, 130, 135,
];
/// `FOURTH_CLASS_GROUP`: the classes of the 3rd class transfer.
const FOURTH_CLASS_GROUP: [u16; 36] = [
    88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110,
    111, 112, 113, 114, 115, 116, 117, 118, 131, 132, 133, 134, 136,
];
/// `FIGHTER_GROUP`.
const FIGHTER_GROUP: [u16; 66] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 18, 19, 20, 21, 22, 23, 24, 31, 32, 33, 34, 35, 36, 37, 44, 45, 46, 47, 48,
    53, 54, 55, 56, 57, 88, 89, 90, 91, 92, 93, 99, 100, 101, 102, 106, 107, 108, 109, 113, 114, 117, 118,
    123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136,
];
/// `MAGE_GROUP`.
const MAGE_GROUP: [u16; 37] = [
    10, 11, 12, 13, 14, 15, 16, 17, 25, 26, 27, 28, 29, 30, 38, 39, 40, 41, 42, 43, 49, 50, 51, 52, 94, 95,
    96, 97, 98, 103, 104, 105, 110, 111, 112, 115, 116,
];
/// `CLERIC_GROUP`.
const CLERIC_GROUP: [u16; 13] = [10, 15, 16, 17, 25, 29, 30, 38, 42, 43, 97, 98, 105];

/// Java `VillageMaster.neverSubclassed`: Overlord and Warsmith.
const NEVER_SUBCLASSED: [u16; 2] = [51, 57];
/// Java `subclasseSet1..5`: the classes too close to each other to be a subclass of one another.
const SUBCLASS_SETS: [&[u16]; 5] = [
    // Dark Avenger, Paladin, Temple Knight, Shillien Knight.
    &[6, 5, 20, 33],
    // Treasure Hunter, Abyss Walker, Plains Walker.
    &[8, 36, 23],
    // Hawkeye, Silver Ranger, Phantom Ranger.
    &[9, 24, 37],
    // Warlock, Elemental Summoner, Phantom Summoner.
    &[14, 28, 41],
    // Sorcerer, Spellsinger, Spellhowler.
    &[12, 27, 40],
];
/// `PlayerClass.MALE_SOULBREAKER`, `FEMALE_SOULBREAKER`, `INSPECTOR`.
const MALE_SOULBREAKER: u16 = 128;
const FEMALE_SOULBREAKER: u16 = 129;
const INSPECTOR: u16 = 135;

/// The village master pages of the subclass menu.
const HTML: &str = "data/html/villagemaster/";

/// Which `VillageMaster` class an NPC is (its `type` in the NPC data).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VillageMasterKind {
    /// `VillageMaster`: every race and class.
    Plain,
    DElf,
    Dwarf,
    Fighter,
    Kamael,
    Mystic,
    Orc,
    Priest,
}

impl VillageMasterKind {
    /// The kind of an NPC `type`, `None` for an NPC that is no village master.
    pub fn from_npc_type(npc_type: &str) -> Option<Self> {
        Some(match npc_type {
            "VillageMaster" => Self::Plain,
            "VillageMasterDElf" => Self::DElf,
            "VillageMasterDwarf" => Self::Dwarf,
            "VillageMasterFighter" => Self::Fighter,
            "VillageMasterKamael" => Self::Kamael,
            "VillageMasterMystic" => Self::Mystic,
            "VillageMasterOrc" => Self::Orc,
            "VillageMasterPriest" => Self::Priest,
            _ => return None,
        })
    }

    /// `checkVillageMasterRace(pClass)`.
    fn check_race(self, class: Option<ClassId>) -> bool {
        let race = || class.and_then(ClassId::race);
        match self {
            Self::Plain => true,
            Self::DElf => race() == Some(Race::DarkElf),
            Self::Dwarf => race() == Some(Race::Dwarf),
            Self::Orc => race() == Some(Race::Orc),
            Self::Kamael => race() == Some(Race::Kamael),
            Self::Fighter | Self::Mystic | Self::Priest => {
                matches!(race(), Some(Race::Human | Race::Elf))
            }
        }
    }

    /// `checkVillageMasterTeachType(pClass)`.
    fn check_teach_type(self, class: Option<ClassId>) -> bool {
        let in_group = |group: &[u16]| class.is_some_and(|c| group.contains(&c.0));
        match self {
            Self::Fighter => in_group(&FIGHTER_GROUP),
            Self::Mystic => in_group(&MAGE_GROUP),
            Self::Priest => in_group(&CLERIC_GROUP),
            Self::Plain | Self::DElf | Self::Dwarf | Self::Orc | Self::Kamael => true,
        }
    }

    /// `getSubClassMenu(race)`.
    fn sub_class_menu(self, everywhere: bool, race: Option<Race>) -> &'static str {
        if self == Self::Kamael {
            if everywhere || race == Some(Race::Kamael) {
                return "data/html/villagemaster/SubClass.htm";
            }
            return "data/html/villagemaster/SubClass_NoKamael.htm";
        }
        if everywhere || race != Some(Race::Kamael) {
            return "data/html/villagemaster/SubClass.htm";
        }
        "data/html/villagemaster/SubClass_NoOther.htm"
    }

    /// `getSubClassFail()`.
    fn sub_class_fail(self) -> &'static str {
        if self == Self::Kamael {
            "data/html/villagemaster/SubClass_Fail_Kamael.htm"
        } else {
            "data/html/villagemaster/SubClass_Fail.htm"
        }
    }
}

/// The class rules of a village master for one player (pure, so the tests can call them).
#[derive(Debug, Clone)]
pub struct SubclassRules<'a> {
    pub kind: VillageMasterKind,
    /// `AltSubclassEverywhere`.
    pub everywhere: bool,
    /// Java `player.getRace()`: the race of the base class.
    pub race: Option<Race>,
    /// `getAppearance().isFemale()`.
    pub female: bool,
    /// `getBaseClass()`.
    pub base_class: i32,
    /// `getPlayerClass()`: the active class.
    pub active_class: i32,
    /// `getSubClasses().values()`.
    pub subs: &'a [SubClassHolder],
}

fn class_of(id: i32) -> Option<ClassId> {
    u16::try_from(id)
        .ok()
        .map(ClassId)
        .filter(|c| c.is_player_class())
}

impl SubclassRules<'_> {
    /// `checkVillageMaster(pclass)`.
    pub fn check_village_master(&self, class: Option<ClassId>) -> bool {
        if self.everywhere {
            return true;
        }
        self.kind.check_race(class) && self.kind.check_teach_type(class)
    }

    /// The 2nd class of the base class (`baseCID.level() > 2 ? baseCID.getParent() : base`).
    fn second_class_of_base(&self) -> i32 {
        match class_of(self.base_class) {
            Some(base) if base.class_level() > 2 => {
                base.enum_parent().map_or(self.base_class, |p| i32::from(p.0))
            }
            _ => self.base_class,
        }
    }

    /// `getSubclasses(player, classId)`: the classes a character whose 2nd class is `class_id`
    /// may take, before the master's own rules. `None` when `class_id` is no 2nd or 3rd class.
    pub fn get_subclasses(&self, class_id: i32) -> Option<Vec<ClassId>> {
        let p_class = class_of(class_id);
        let id = u16::try_from(class_id).ok()?;
        if !THIRD_CLASS_GROUP.contains(&id) && !FOURTH_CLASS_GROUP.contains(&id) {
            return None;
        }
        // `EnumSet` order is the `PlayerClass` order, which is the id order.
        let mut subclasses: Vec<ClassId> = THIRD_CLASS_GROUP
            .iter()
            .filter(|c| !NEVER_SUBCLASSED.contains(c))
            .map(|&c| ClassId(c))
            .filter(|c| Some(*c) != p_class)
            .collect();
        if self.race == Some(Race::Kamael) {
            subclasses.retain(|c| c.race() == Some(Race::Kamael));
            if self.female {
                subclasses.retain(|c| c.0 != MALE_SOULBREAKER);
            } else {
                subclasses.retain(|c| c.0 != FEMALE_SOULBREAKER);
            }
            let second_sub_75 = self.subs.iter().any(|s| s.class_index == 2 && s.level >= 75);
            if !second_sub_75 {
                subclasses.retain(|c| c.0 != INSPECTOR);
            }
        } else {
            if self.race == Some(Race::Elf) {
                subclasses.retain(|c| c.race() != Some(Race::DarkElf));
            } else if self.race == Some(Race::DarkElf) {
                subclasses.retain(|c| c.race() != Some(Race::Elf));
            }
            subclasses.retain(|c| c.race() != Some(Race::Kamael));
        }
        if let Some(set) = SUBCLASS_SETS.iter().find(|set| set.contains(&id)) {
            subclasses.retain(|c| !set.contains(&c.0));
        }
        if let Some(current) = class_of(self.active_class) {
            subclasses.retain(|c| !current.equals_or_child_of(*c));
        }
        Some(subclasses)
    }

    /// `getAvailableSubClasses(player)`: the classes the master offers, without the ones the
    /// character already has (or a parent of one of them).
    pub fn available_sub_classes(&self) -> Option<Vec<ClassId>> {
        let mut avail = self.get_subclasses(self.second_class_of_base())?;
        avail.retain(|&pclass| {
            self.check_village_master(Some(pclass))
                && !self
                    .subs
                    .iter()
                    .any(|sub| class_of(sub.class_id).is_some_and(|s| s.equals_or_child_of(pclass)))
        });
        Some(avail)
    }

    /// `isValidNewSubClass(player, classId)`.
    pub fn is_valid_new_sub_class(&self, class_id: i32) -> bool {
        let cid = class_of(class_id);
        if !self.check_village_master(cid) {
            return false;
        }
        if let Some(cid) = cid
            && self
                .subs
                .iter()
                .any(|sub| class_of(sub.class_id).is_some_and(|s| s.equals_or_child_of(cid)))
        {
            return false;
        }
        self.get_subclasses(self.second_class_of_base())
            .is_some_and(|avail| avail.iter().any(|c| i32::from(c.0) == class_id))
    }
}

/// Java `Integer.parseInt(s.trim())`: `trim` drops everything up to the space character.
fn parse_int(text: &str) -> Option<i32> {
    text.trim_matches(|c: char| c <= ' ').parse::<i32>().ok()
}

/// Java's parsing of `Subclass <choice> <one> <two>`: `command.substring(9, 10)`, then the text
/// up to the next space after index 11 and the rest. An exception (a short command, a bad
/// number) is logged and keeps the values read so far.
fn parse_subclass_command(command: &str) -> (i32, i32, i32) {
    let units: Vec<u16> = command.encode_utf16().collect();
    let text = |from: usize, to: usize| -> Option<String> {
        (from <= to && to <= units.len()).then(|| String::from_utf16_lossy(&units[from..to]))
    };
    let mut choice = 0;
    let mut one = 0;
    let mut two = 0;
    let mut parse = || -> Option<()> {
        choice = parse_int(&text(9, 10)?)?;
        let end = units
            .iter()
            .skip(11)
            .position(|&u| u == u16::from(b' '))
            .map_or(units.len(), |p| p + 11);
        if units.len() > 11 {
            one = parse_int(&text(11, end)?)?;
            if units.len() > end {
                two = parse_int(&text(end, units.len())?)?;
            }
        }
        Some(())
    };
    if parse().is_none() {
        warn!("VillageMaster: Wrong numeric values for command {command}");
    }
    (choice, one, two)
}

impl World {
    /// The village master kind of an NPC.
    fn village_master_kind(&self, npc: ObjectId) -> VillageMasterKind {
        self.npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .and_then(|t| VillageMasterKind::from_npc_type(&t.npc_type))
            .unwrap_or(VillageMasterKind::Plain)
    }

    /// Java `player.getRace()`: the race of the base class.
    fn base_race(&self, id: ObjectId) -> Option<Race> {
        let p = self.players.get(&id)?;
        class_of(p.data.base_class)
            .and_then(ClassId::race)
            .or_else(|| u8::try_from(p.data.race).ok().and_then(race_of_ordinal))
    }

    /// The class rules of `kind` for `id`; `subs` is the caller's copy of the holders.
    fn subclass_rules<'a>(
        &self,
        id: ObjectId,
        kind: VillageMasterKind,
        subs: &'a [SubClassHolder],
    ) -> SubclassRules<'a> {
        let (female, base_class, active_class) = self.players.get(&id).map_or((false, 0, 0), |p| {
            (p.data.sex == 1, p.data.base_class, p.data.class_id)
        });
        SubclassRules {
            kind,
            everywhere: self.settings.subclass.alt_game_subclass_everywhere,
            race: self.base_race(id),
            female,
            base_class,
            active_class,
            subs,
        }
    }

    /// `ClassListData.getInstance().getClass(id).getClassName()`.
    fn vm_class_name(&self, class_id: i32) -> String {
        self.admin
            .class_list
            .get_by_id(class_id)
            .map(|c| c.name.clone())
            .unwrap_or_default()
    }

    /// `checkQuests(player)`: a noble may add a subclass without the quests; anybody else needs
    /// `Q00234_FatesWhisper` and `Q00235_MimirsElixir` completed, or `Q00236_SeedsOfChaos` instead
    /// of Mimir's Elixir at a Kamael master (`VillageMasterKamael.checkQuests`).
    fn vm_check_quests(&self, id: ObjectId, kind: VillageMasterKind) -> bool {
        // Noble players can add Sub-Classes without quests.
        if self.is_noble(id) {
            return true;
        }
        let completed = |name: &str| {
            self.scripts
                .players
                .get(&id)
                .and_then(|q| q.get(name))
                .is_some_and(|qs| qs.is_completed())
        };
        if !completed("Q00234_FatesWhisper") {
            return false;
        }
        let second = if kind == VillageMasterKind::Kamael {
            "Q00236_SeedsOfChaos"
        } else {
            "Q00235_MimirsElixir"
        };
        completed(second)
    }

    /// A page of the village master (`html.setFile(player, path)`).
    fn vm_page(&self, player: ObjectId, npc: ObjectId, path: &str) -> Option<NpcHtmlMessage> {
        self.html_from_file(player, npc.0, path)
    }

    /// Java `VillageMaster.onBypassFeedback` for a command that starts with `Subclass`.
    pub(crate) fn village_master_subclass_bypass(&mut self, npc: ObjectId, player: ObjectId, command: &str) {
        // Subclasses may not be changed while a skill is in use.
        if self.is_casting_now(player) || self.is_all_skills_disabled(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::SubclassesMayNotBeCreatedOrChangedWhileASkillIsInUse),
            );
            return;
        }
        if self.is_registered_in_comp(player) {
            self.olympiad_unregister_noble(player);
        }
        // Subclasses may not be changed while a transformated state.
        if self.is_transformed(player) {
            if let Some(html) =
                self.vm_page(player, npc, "data/html/villagemaster/SubClass_NoTransformed.htm")
            {
                self.send_html(player, &html);
            }
            return;
        }
        // Subclasses may not be changed while a summon is active.
        if self.summon_of(player).is_some() {
            if let Some(html) = self.vm_page(player, npc, "data/html/villagemaster/SubClass_NoSummon.htm") {
                self.send_html(player, &html);
            }
            return;
        }
        // Subclasses may not be changed while you have exceeded your inventory limit.
        if !self.is_inventory_under_90(player, true) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::ASubClassCannotBeCreatedOrChangedBecauseYouHaveExceededYourInventoryLimit,
                ),
            );
            return;
        }
        // Subclasses may not be changed while a you are over your weight limit.
        let weight_penalty = self
            .players
            .get(&player)
            .map_or(0, |p| if p.gm.diet { 0 } else { p.weight_penalty });
        if weight_penalty >= 2 {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::ASubClassCannotBeCreatedOrChangedWhileYouAreOverYourWeightLimit,
                ),
            );
            return;
        }

        let (choice, param_one, param_two) = parse_subclass_command(command);
        let kind = self.village_master_kind(npc);
        let subs = self.sub_classes(player);
        let max_subclass = self.settings.subclass.max_subclass;
        let total = i32::try_from(subs.len()).unwrap_or(i32::MAX);
        let html = match choice {
            // Subclass change menu.
            0 => {
                let race = self.base_race(player);
                let menu = kind.sub_class_menu(self.settings.subclass.alt_game_subclass_everywhere, race);
                self.vm_page(player, npc, menu)
            }
            // Add Subclass - Initial.
            1 => {
                // Avoid giving player an option to add a new sub class, if they have max
                // sub-classes already.
                if total >= max_subclass {
                    self.vm_page(player, npc, kind.sub_class_fail())
                } else {
                    let rules = self.subclass_rules(player, kind, &subs);
                    match rules.available_sub_classes().filter(|a| !a.is_empty()) {
                        Some(avail) => {
                            let mut list = String::new();
                            for class in avail {
                                let name = self.vm_class_name(i32::from(class.0));
                                list.push_str(&format!(
                                    "<a action=\"bypass npc_%objectId%_Subclass 4 {}\" msg=\"1268;{name}\">{name}</a><br>",
                                    class.0
                                ));
                            }
                            self.vm_page(player, npc, "data/html/villagemaster/SubClass_Add.htm")
                                .map(|mut h| {
                                    h.html = h.html.replace("%list%", &list);
                                    h
                                })
                        }
                        None => {
                            // NOTE(java-quirk): these pages go out before `%objectId%` is replaced.
                            let race = self.base_race(player);
                            let fail = match race {
                                Some(Race::Elf | Race::DarkElf) => Some("SubClass_Fail_Elves.htm"),
                                Some(Race::Kamael) => Some("SubClass_Fail_Kamael.htm"),
                                _ => None,
                            };
                            match fail {
                                Some(page) => {
                                    if let Some(html) = self.vm_page(player, npc, &format!("{HTML}{page}")) {
                                        self.send_html(player, &html);
                                    }
                                }
                                // TODO in Java: retail message.
                                None => self
                                    .send_message(player, "There are no sub classes available at this time."),
                            }
                            return;
                        }
                    }
                }
            }
            // Change Class - Initial.
            2 => {
                if subs.is_empty() {
                    self.vm_page(player, npc, "data/html/villagemaster/SubClass_ChangeNo.htm")
                } else {
                    let rules = self.subclass_rules(player, kind, &subs);
                    let base_class = rules.base_class;
                    let mut list = String::new();
                    if rules.check_village_master(class_of(base_class)) {
                        list.push_str(&format!(
                            "<a action=\"bypass -h npc_%objectId%_Subclass 5 0\">{}</a><br>",
                            self.vm_class_name(base_class)
                        ));
                    }
                    for sub in &subs {
                        if rules.check_village_master(class_of(sub.class_id)) {
                            list.push_str(&format!(
                                "<a action=\"bypass -h npc_%objectId%_Subclass 5 {}\">{}</a><br>",
                                sub.class_index,
                                self.vm_class_name(sub.class_id)
                            ));
                        }
                    }
                    if list.is_empty() {
                        self.vm_page(player, npc, "data/html/villagemaster/SubClass_ChangeNotFound.htm")
                    } else {
                        self.vm_page(player, npc, "data/html/villagemaster/SubClass_Change.htm")
                            .map(|mut h| {
                                h.html = h.html.replace("%list%", &list);
                                h
                            })
                    }
                }
            }
            // Change/Cancel Subclass - Initial.
            3 => {
                if subs.is_empty() {
                    self.vm_page(player, npc, "data/html/villagemaster/SubClass_ModifyEmpty.htm")
                } else if total > 3 {
                    // custom value
                    let mut list = String::new();
                    for (n, sub) in subs.iter().enumerate() {
                        list.push_str(&format!(
                            "Sub-class {}<br><a action=\"bypass -h npc_%objectId%_Subclass 6 {}\">{}</a><br>",
                            n + 1,
                            sub.class_index,
                            self.vm_class_name(sub.class_id)
                        ));
                    }
                    self.vm_page(player, npc, "data/html/villagemaster/SubClass_ModifyCustom.htm")
                        .map(|mut h| {
                            h.html = h.html.replace("%list%", &list);
                            h
                        })
                } else {
                    // retail html contain only 3 subclasses
                    self.vm_page(player, npc, "data/html/villagemaster/SubClass_Modify.htm")
                        .map(|mut h| {
                            for index in 1..=3 {
                                match subs.iter().find(|s| s.class_index == index) {
                                    Some(sub) => {
                                        h.html = h
                                            .html
                                            .replace(&format!("%sub{index}%"), &self.vm_class_name(sub.class_id));
                                    }
                                    None => {
                                        h.html = h.html.replace(
                                            &format!(
                                                "<a action=\"bypass npc_%objectId%_Subclass 6 {index}\">%sub{index}%</a><br>"
                                            ),
                                            "",
                                        );
                                    }
                                }
                            }
                            h
                        })
                }
            }
            // Add Subclass - Action (Subclass 4 x[x]).
            4 => {
                if !self.flood_allows(player, FloodKind::Subclass) {
                    return;
                }
                let level = self.core_of(player).map_or(0, |c| c.level);
                let mut allow_addition = total < max_subclass && level >= 75;
                // If the character is less than level 75 on any of their previously chosen
                // classes then disallow them to change to their most recently added sub-class.
                if allow_addition && subs.iter().any(|s| s.level < 75) {
                    allow_addition = false;
                }
                if allow_addition && !self.settings.subclass.alt_game_subclass_without_quests {
                    allow_addition = self.vm_check_quests(player, kind);
                }
                let valid = allow_addition
                    && self
                        .subclass_rules(player, kind, &subs)
                        .is_valid_new_sub_class(param_one);
                if valid {
                    if !self.add_sub_class(player, param_one, total + 1) {
                        return;
                    }
                    let page = self.vm_page(player, npc, "data/html/villagemaster/SubClass_AddOk.htm");
                    let mut after = vec![AfterSwitch::Message(SystemMessageId::TheNewSubclassHasBeenAdded)];
                    after.extend(page.map(|h| AfterSwitch::Html(with_object_id(h, npc))));
                    let new_total = self.total_sub_classes(player);
                    self.set_active_class_then(player, new_total, after);
                    return;
                }
                self.vm_page(player, npc, kind.sub_class_fail())
            }
            // Change Class - Action.
            5 => {
                if !self.flood_allows(player, FloodKind::Subclass) {
                    return;
                }
                let class_index = self
                    .players
                    .get(&player)
                    .map_or(0, |p| p.data.profile.class_index);
                if class_index == param_one {
                    self.vm_page(player, npc, "data/html/villagemaster/SubClass_Current.htm")
                } else {
                    let rules = self.subclass_rules(player, kind, &subs);
                    let class = if param_one == 0 {
                        Some(rules.base_class)
                    } else {
                        subs.iter()
                            .find(|s| s.class_index == param_one)
                            .map(|s| s.class_id)
                    };
                    let Some(class) = class else {
                        return;
                    };
                    if !rules.check_village_master(class_of(class)) {
                        return;
                    }
                    self.set_active_class_then(
                        player,
                        param_one,
                        vec![AfterSwitch::Message(
                            SystemMessageId::YouHaveSuccessfullySwitchedToYourSubclass,
                        )],
                    );
                    return;
                }
            }
            // Change/Cancel Subclass - Choice.
            6 => {
                // validity check
                if param_one < 1 || param_one > max_subclass {
                    return;
                }
                let rules = self.subclass_rules(player, kind, &subs);
                let Some(avail) = rules.available_sub_classes().filter(|a| !a.is_empty()) else {
                    // TODO in Java: retail message.
                    self.send_message(player, "There are no sub classes available at this time.");
                    return;
                };
                let mut list = String::new();
                for class in avail {
                    list.push_str(&format!(
                        "<a action=\"bypass npc_%objectId%_Subclass 7 {param_one} {}\" msg=\"1445;\">{}</a><br>",
                        class.0,
                        self.vm_class_name(i32::from(class.0))
                    ));
                }
                let page = match param_one {
                    1 => "SubClass_ModifyChoice1.htm",
                    2 => "SubClass_ModifyChoice2.htm",
                    3 => "SubClass_ModifyChoice3.htm",
                    _ => "SubClass_ModifyChoice.htm",
                };
                self.vm_page(player, npc, &format!("{HTML}{page}")).map(|mut h| {
                    h.html = h.html.replace("%list%", &list);
                    h
                })
            }
            // Change Subclass - Action.
            7 => {
                // Warning: the information about this subclass will be removed from the subclass
                // list even if false!
                if !self.flood_allows(player, FloodKind::Subclass) {
                    return;
                }
                if !self
                    .subclass_rules(player, kind, &subs)
                    .is_valid_new_sub_class(param_two)
                {
                    return;
                }
                if self.modify_sub_class(player, param_one, param_two) {
                    self.abort_cast(player);
                    // all effects from old subclass stopped!
                    self.stop_all_effects_except_those_that_last_through_death(player);
                    self.stop_all_effects_not_stay_on_subclass_change(player);
                    self.stop_cubics(player);
                    let name = self.vm_class_name(param_two);
                    let page = self
                        .vm_page(player, npc, "data/html/villagemaster/SubClass_ModifyOk.htm")
                        .map(|mut h| {
                            h.html = h.html.replace("%name%", &name);
                            with_object_id(h, npc)
                        });
                    let mut after = vec![AfterSwitch::Message(SystemMessageId::TheNewSubclassHasBeenAdded)];
                    after.extend(page.map(AfterSwitch::Html));
                    self.set_active_class_then(player, param_one, after);
                } else {
                    // This isn't good! modifySubClass() removed subclass from memory we must update
                    // _classIndex! Also updates _classIndex plus switching _classid to baseclass.
                    self.set_active_class_then(
                        player,
                        0,
                        vec![AfterSwitch::Text(
                            "The sub class could not be added, you have been reverted to your base class."
                                .to_owned(),
                        )],
                    );
                }
                return;
            }
            // NOTE(java-quirk): any other choice leaves the page without content, and Java's
            // `replace` on it throws, so nothing is sent.
            _ => return,
        };
        if let Some(html) = html {
            let html = with_object_id(html, npc);
            self.send_html(player, &html);
        }
    }
}

/// `html.replace("%objectId%", String.valueOf(getObjectId()))`.
fn with_object_id(mut html: NpcHtmlMessage, npc: ObjectId) -> NpcHtmlMessage {
    html.html = html.html.replace("%objectId%", &npc.0.to_string());
    html
}

/// The playable race of a `Race` ordinal (`characters.race`).
fn race_of_ordinal(ordinal: u8) -> Option<Race> {
    Some(match ordinal {
        0 => Race::Human,
        1 => Race::Elf,
        2 => Race::DarkElf,
        3 => Race::Orc,
        4 => Race::Dwarf,
        5 => Race::Kamael,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules<'a>(
        kind: VillageMasterKind,
        base: i32,
        active: i32,
        subs: &'a [SubClassHolder],
    ) -> SubclassRules<'a> {
        SubclassRules {
            kind,
            everywhere: false,
            race: class_of(base).and_then(ClassId::race),
            female: false,
            base_class: base,
            active_class: active,
            subs,
        }
    }

    fn sub(class_id: i32, class_index: i32, level: i32) -> SubClassHolder {
        SubClassHolder {
            class_id,
            exp: 0,
            sp: 0,
            level,
            class_index,
        }
    }

    fn ids(classes: &[ClassId]) -> Vec<u16> {
        classes.iter().map(|c| c.0).collect()
    }

    /// The category lists match `categoryData.xml` of the reference datapack; skipped without it.
    #[test]
    fn category_lists_match_the_datapack() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/game/data/categoryData.xml"
        ));
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("skipped: {} does not exist", path.display());
            return;
        };
        let listed = |name: &str| -> Vec<u16> {
            let start = text.find(&format!("<category name=\"{name}\">")).unwrap();
            let block = &text[start..start + text[start..].find("</category>").unwrap()];
            block
                .split("<id>")
                .skip(1)
                .map(|part| part.split("</id>").next().unwrap().trim().parse().unwrap())
                .collect()
        };
        assert_eq!(listed("THIRD_CLASS_GROUP"), THIRD_CLASS_GROUP.to_vec());
        assert_eq!(listed("FOURTH_CLASS_GROUP"), FOURTH_CLASS_GROUP.to_vec());
        assert_eq!(listed("FIGHTER_GROUP"), FIGHTER_GROUP.to_vec());
        assert_eq!(listed("MAGE_GROUP"), MAGE_GROUP.to_vec());
        assert_eq!(listed("CLERIC_GROUP"), CLERIC_GROUP.to_vec());
    }

    #[test]
    fn a_gladiator_may_take_every_2nd_class_but_its_own_overlord_warsmith_and_kamael() {
        // Duelist (88) on the base class: its 2nd class is Gladiator (2).
        let r = rules(VillageMasterKind::Plain, 88, 88, &[]);
        let avail = ids(&r.available_sub_classes().unwrap());
        assert!(!avail.contains(&2), "own 2nd class");
        assert!(!avail.contains(&51) && !avail.contains(&57), "never subclassed");
        assert!(avail.iter().all(|c| ClassId(*c).race() != Some(Race::Kamael)));
        // A human may take both elf and dark elf classes.
        assert!(avail.contains(&20) && avail.contains(&33));
        assert_eq!(avail.first(), Some(&3));
    }

    #[test]
    fn similar_classes_and_the_other_elves_are_left_out() {
        // Paladin's set: Dark Avenger, Temple Knight, Shillien Knight.
        let r = rules(VillageMasterKind::Plain, 5, 5, &[]);
        let avail = ids(&r.available_sub_classes().unwrap());
        for c in [5, 6, 20, 33] {
            assert!(!avail.contains(&c), "{c}");
        }
        // An elf (Silver Ranger 24) may take no dark elf class, nor Hawkeye / Phantom Ranger.
        let r = rules(VillageMasterKind::Plain, 24, 24, &[]);
        let avail = ids(&r.available_sub_classes().unwrap());
        assert!(avail.iter().all(|c| ClassId(*c).race() != Some(Race::DarkElf)));
        assert!(!avail.contains(&9) && !avail.contains(&37));
    }

    #[test]
    fn kamael_rules_gender_and_the_inspector() {
        // Berserker (127), male: only Kamael, no female Soul Breaker, no Inspector before the
        // second subclass reaches 75.
        let r = rules(VillageMasterKind::Kamael, 127, 127, &[]);
        let avail = ids(&r.available_sub_classes().unwrap());
        assert_eq!(avail, vec![128, 130]);
        let subs = [sub(130, 1, 80), sub(128, 2, 75)];
        let r = rules(VillageMasterKind::Kamael, 127, 127, &subs);
        // The subclasses taken drop out, Inspector comes in.
        assert_eq!(ids(&r.available_sub_classes().unwrap()), vec![135]);
        // A non-Kamael master teaches no Kamael class, even with `AltSubclassEverywhere` off.
        let r = rules(VillageMasterKind::Fighter, 127, 127, &[]);
        assert!(r.available_sub_classes().unwrap().is_empty());
    }

    #[test]
    fn the_masters_teach_their_race_and_type() {
        let none: [SubClassHolder; 0] = [];
        let r = rules(VillageMasterKind::Mystic, 88, 88, &none);
        let avail = ids(&r.available_sub_classes().unwrap());
        assert!(avail.iter().all(|c| MAGE_GROUP.contains(c)));
        assert!(
            avail
                .iter()
                .all(|c| matches!(ClassId(*c).race(), Some(Race::Human | Race::Elf)))
        );
        assert!(avail.contains(&12) && avail.contains(&27));
        let r = rules(VillageMasterKind::Orc, 88, 88, &none);
        assert_eq!(ids(&r.available_sub_classes().unwrap()), vec![46, 48, 52]);
        let r = rules(VillageMasterKind::Dwarf, 88, 88, &none);
        assert_eq!(ids(&r.available_sub_classes().unwrap()), vec![55]);
        // Everywhere: every master teaches every class.
        let r = SubclassRules {
            everywhere: true,
            ..rules(VillageMasterKind::Dwarf, 88, 88, &none)
        };
        assert!(ids(&r.available_sub_classes().unwrap()).len() > 20);
    }

    #[test]
    fn a_taken_subclass_or_its_parent_is_not_valid_again() {
        // Gladiator base with Bishop (16) as subclass that became Cardinal (97).
        let subs = [sub(97, 1, 76)];
        let r = rules(VillageMasterKind::Plain, 2, 2, &subs);
        assert!(!r.is_valid_new_sub_class(16));
        assert!(r.is_valid_new_sub_class(17));
        assert!(!r.is_valid_new_sub_class(2));
        assert!(!r.is_valid_new_sub_class(999));
        // A 1st class is no 2nd class: `getSubclasses` gives null.
        let r = rules(VillageMasterKind::Plain, 1, 1, &[]);
        assert!(r.available_sub_classes().is_none());
    }

    #[test]
    fn the_active_class_and_its_parents_are_left_out() {
        // On a Cardinal (97) subclass: Bishop (16) is its parent and drops out.
        let r = rules(VillageMasterKind::Plain, 2, 97, &[]);
        assert!(!ids(&r.available_sub_classes().unwrap()).contains(&16));
    }

    #[test]
    fn the_command_is_read_like_java() {
        assert_eq!(parse_subclass_command("Subclass 0"), (0, 0, 0));
        assert_eq!(parse_subclass_command("Subclass 4 12"), (4, 12, 0));
        assert_eq!(parse_subclass_command("Subclass 7 2 103"), (7, 2, 103));
        assert_eq!(parse_subclass_command("Subclass"), (0, 0, 0));
        assert_eq!(parse_subclass_command("Subclass 5 x"), (5, 0, 0));
    }

    #[test]
    fn the_kamael_master_has_its_own_menu_and_fail_page() {
        assert_eq!(
            VillageMasterKind::Kamael.sub_class_menu(false, Some(Race::Human)),
            "data/html/villagemaster/SubClass_NoKamael.htm"
        );
        assert_eq!(
            VillageMasterKind::Plain.sub_class_menu(false, Some(Race::Kamael)),
            "data/html/villagemaster/SubClass_NoOther.htm"
        );
        assert_eq!(
            VillageMasterKind::Plain.sub_class_menu(true, Some(Race::Kamael)),
            "data/html/villagemaster/SubClass.htm"
        );
        assert_eq!(
            VillageMasterKind::Kamael.sub_class_fail(),
            "data/html/villagemaster/SubClass_Fail_Kamael.htm"
        );
    }
}
