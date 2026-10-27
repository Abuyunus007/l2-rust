//! The heroes: the choice at the period end, `isHero`, the hero skills and items, the hero list,
//! the diary and the history pages, the hero words and `//sethero`, `//givehero`.
// Java: mechanics/olympiad/Hero.java
// Java: entity/actor/Player.java (setHero, isHero)
// Java: network/clientpackets/{RequestWriteHeroWords,RequestBypassToServer}.java (`_match`,
//       `_diary`), network/serverpackets/ExHeroList.java
// Java: dist/game/data/scripts/handlers/bypass/npc/OlympiadManagerLink.java (`olympiad 4`, `5`)
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminAdmin.java (sethero, givehero)
//
// # What a hero is
// Java's `Hero` keeps `HEROES` (the heroes of the last period, claimed or not) and
// `COMPLETE_HEROS` (everybody who ever was one) as separate maps of `StatSet`s, which are the same
// object for a hero chosen after the start and separate ones for a hero read at the start. The
// only visible difference is a stale `claimed` flag in `COMPLETE_HEROS` that the next choice
// overwrites, so [`HeroState`] keeps one map and the list of the ids in `HEROES` (in Java's
// `ConcurrentHashMap` order, which the hero list and `getHeroByClass` follow).
// `Player._hero` is [`PlayerOlympiad::hero`](super::PlayerOlympiad::hero).
//
// # Side by side (D-39, D-53)
// The choice (`updateHeroes`, `deleteItemsInDb`) and the `heroes` rows are global: with
// `CycleDatabaseWrites = False` the Java game server runs them on the same database. This server
// then
// - reads the `heroes` rows at start and again before it shows or uses them (the hero list, the
//   history pages, the certification, `//givehero`: a [`OlympiadRereadRequest::Heroes`] through
//   the outbox, D-15, D-50; a failed read shows and does nothing),
// - runs the choice in memory (the online heroes of the last period lose their status and
//   items, the online players' own item rows go through the normal inventory writes) but writes
//   no `heroes` row and does not run `deleteItemsInDb`,
// - keeps a claim made on this server when a later read brings the same hero unclaimed (the Java
//   game server never sees it, nobody claims there),
// - still writes what Java writes per player: the `heroes_diary` rows of a hero's deeds
//   (`setDiaryData`) and the hero words (`UPDATE heroes SET message`) of the heroes whose words
//   this server changed. With the switch on it writes everything where Java does.

use std::collections::{BTreeMap, BTreeSet};

use l2_data::ClassId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ExHeroList, HeroEntry, InventoryUpdate, NpcHtmlMessage, SocialAction, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::{info, warn};

use super::period::format_local_date;
use super::{
    HeroToBe, OLYMPIAD_HTML_PATH, OlympiadRereadAction, OlympiadRereadRequest, OlympiadRereadRows,
    OlympiadRereadThen, OlympiadWrite,
};
use crate::admin::{AdminHandler, AdminHandlers, JavaException};
use crate::clan::model::java_map_order;
use crate::inventory::{LOC_INVENTORY, LOC_PAPERDOLL};
use crate::outbox::PersistCommand;
use crate::residence::CastleId;
use crate::world::World;

/// Java `Hero.ACTION_RAID_KILLED`: the diary entry of a raid boss kill (`param` is the NPC id).
pub const ACTION_RAID_KILLED: i32 = 1;
/// Java `Hero.ACTION_HERO_GAINED`.
pub const ACTION_HERO_GAINED: i32 = 2;
/// Java `Hero.ACTION_CASTLE_TAKEN`: the diary entry of a castle taken (`param` is the castle id).
pub const ACTION_CASTLE_TAKEN: i32 = 3;

/// The "Hero Animation" of `claimHero`.
const HERO_ANIMATION: i32 = 20016;
/// `RequestWriteHeroWords`: more than this many characters are ignored.
const MAX_HERO_WORDS: usize = 300;
/// Java `perpage` of `showHeroDiary` and `showHeroFights`.
const DIARY_PER_PAGE: i32 = 10;
const FIGHTS_PER_PAGE: i32 = 20;

/// A hero (Java's `StatSet`: `char_name`, `class_id`, `count`, `played`, `claimed` and the clan
/// keys).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeroData {
    pub char_name: String,
    pub class_id: i32,
    /// `Hero.COUNT`: how many times the character was chosen.
    pub count: i32,
    /// `Hero.PLAYED`: 1 for a hero of the last period.
    pub played: i32,
    /// `Hero.CLAIMED`.
    pub claimed: bool,
    pub clan_name: String,
    pub clan_crest: i32,
    pub ally_name: String,
    pub ally_crest: i32,
}

/// One `heroes` row (`GET_ALL_HEROES`) with the clan and alliance ids of the character
/// (`GET_CLAN_ALLY`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeroRow {
    pub char_id: i32,
    pub char_name: String,
    pub class_id: i32,
    pub count: i32,
    pub played: i32,
    pub claimed: bool,
    pub clan_id: i32,
    pub ally_id: i32,
}

/// One `heroes_diary` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeroDiaryRow {
    /// Unix milliseconds.
    pub time: i64,
    pub action: i32,
    pub param: i32,
}

/// One `olympiad_fights` row of a hero, with the names of both fighters (`None` for a character
/// `CharInfoTable` does not know).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroFightRow {
    pub char_one_id: i32,
    pub char_one_class: i32,
    pub char_one_name: Option<String>,
    pub char_two_id: i32,
    pub char_two_class: i32,
    pub char_two_name: Option<String>,
    pub winner: i32,
    pub start: i64,
    pub time: i64,
    pub classed: i32,
}

/// What `loadMessage`, `loadDiary` and `loadFights` read for one hero.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeroDetails {
    pub char_id: i32,
    /// `None` without a `heroes` row.
    pub message: Option<String>,
    pub diary: Vec<HeroDiaryRow>,
    pub fights: Vec<HeroFightRow>,
}

/// What the game server reads for [`World::load_heroes`] (`Hero.init`): every `heroes` row
/// (`GET_ALL_HEROES`; the rows with `played = 1` are the heroes) and the details of the played
/// heroes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeroLoad {
    pub rows: Vec<HeroRow>,
    pub details: Vec<HeroDetails>,
}

/// A write of the hero tables (`OlympiadWrite::Hero`), in Java's statements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeroWrite {
    /// `updateHeroes(true)`: `UPDATE heroes SET played = 0`. Only with `CycleDatabaseWrites`.
    ResetPlayed,
    /// `deleteItemsInDb`. Only with `CycleDatabaseWrites`.
    DeleteItems,
    /// `updateHeroes(false)` for a hero `COMPLETE_HEROS` did not know. Only with
    /// `CycleDatabaseWrites`.
    Insert {
        char_id: i32,
        class_id: i32,
        count: i32,
        played: i32,
        claimed: bool,
    },
    /// `updateHeroes(false)` for a known hero. Only with `CycleDatabaseWrites`.
    Update {
        char_id: i32,
        class_id: i32,
        count: i32,
        played: i32,
        claimed: bool,
    },
    /// `setDiaryData`: written as Java does, whatever the switch says (a hero's own deed).
    Diary {
        char_id: i32,
        time: i64,
        action: i32,
        param: i32,
    },
    /// `saveHeroMessage`.
    Message { char_id: i32, message: String },
}

/// What a read of the hero tables is for (the `then` of `OlympiadRereadAction::Hero`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeroAction {
    /// `olympiad 4`: `new ExHeroList()`.
    List,
    /// `olympiad 5`: the hero certification.
    Certification,
    /// `_diary?class=<class>&page=<page>`.
    Diary { class_id: i32, page: i32 },
    /// `_match?class=<class>&page=<page>`.
    Fights { class_id: i32, page: i32 },
    /// `//givehero` of `gm` on the player of the answer.
    GiveHero { gm: ObjectId },
    /// `loadFights` and `loadDiary` of the player of the answer (after `claimHero`).
    Details,
}

/// A line of a hero's diary (Java's `StatSet`: `date`, `action`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct DiaryEntry {
    date: String,
    /// `diaryEntry.getString("action", "")`.
    action: String,
}

/// A line of a hero's match history.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FightEntry {
    opponent: String,
    opponent_class: String,
    time: String,
    start: String,
    classed: i32,
    /// `None` for a `winner` that is not 0, 1 or 2: Java's `getString("result")` then throws.
    result: Option<&'static str>,
}

/// The `Hero` singleton: `HEROES`, `COMPLETE_HEROS`, `HERO_COUNTS`, `HERO_FIGHTS`, `HERO_DIARY`
/// and `HERO_MESSAGE`.
#[derive(Debug, Default)]
pub struct HeroState {
    /// `COMPLETE_HEROS` (and the values of `HEROES`), by char id.
    all: BTreeMap<i32, HeroData>,
    /// The keys of `HEROES` in insertion order, and the most it ever held (the table of Java's
    /// `ConcurrentHashMap` never shrinks).
    heroes: Vec<i32>,
    peak: usize,
    /// `HERO_DIARY`.
    diary: BTreeMap<i32, Vec<DiaryEntry>>,
    /// `HERO_FIGHTS` and `HERO_COUNTS` (victories, draws, losses).
    fights: BTreeMap<i32, Vec<FightEntry>>,
    counts: BTreeMap<i32, (i32, i32, i32)>,
    /// `HERO_MESSAGE`, and the heroes whose words this server changed.
    messages: BTreeMap<i32, String>,
    dirty_messages: BTreeSet<i32>,
}

impl HeroState {
    /// The heroes of the last period in Java's map order.
    fn ordered(&self) -> Vec<i32> {
        java_map_order(&self.heroes, self.peak)
            .into_iter()
            .filter_map(|i| self.heroes.get(i).copied())
            .collect()
    }

    /// `HEROES.size()`.
    pub fn len(&self) -> usize {
        self.heroes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heroes.is_empty()
    }

    /// `COMPLETE_HEROS.size()`.
    pub fn all_time_len(&self) -> usize {
        self.all.len()
    }

    /// `HERO_MESSAGE.get(charId)`.
    pub fn message(&self, char_id: i32) -> Option<&str> {
        self.messages.get(&char_id).map(String::as_str)
    }

    /// The number of lines of `HERO_DIARY.get(charId)`, `None` without a list.
    pub fn diary_len(&self, char_id: i32) -> Option<usize> {
        self.diary.get(&char_id).map(Vec::len)
    }
}

/// `String.replaceAll(pattern, value.replaceAll("\\$", "\\\\\\$"))` for a literal pattern, the
/// way `AbstractHtmlPacket.replace` does it: every `$` of the value is escaped, but a backslash
/// of the value still escapes the next character of the replacement. `None` where Java throws (a
/// backslash at the end, or a `$` that a backslash before it left unescaped).
fn java_html_replace(html: &mut String, pattern: &str, value: &str) -> Option<()> {
    let escaped = value.replace('$', "\\$");
    let mut replacement = String::with_capacity(escaped.len());
    let mut chars = escaped.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => replacement.push(chars.next()?),
            // "Illegal group reference"
            '$' => return None,
            other => replacement.push(other),
        }
    }
    *html = html.replace(pattern, &replacement);
    Some(())
}

impl World {
    // ------------------------------------------------------------------------------- queries

    /// The hero data of the world.
    pub fn hero_state(&self) -> &HeroState {
        &self.olympiad.heroes
    }

    /// Java `Player.isHero()`.
    pub fn is_hero(&self, id: ObjectId) -> bool {
        self.players.get(&id).is_some_and(|p| p.olympiad.hero)
    }

    /// Java `Hero.isHero(objectId)`: a claimed hero of the last period.
    pub fn is_hero_char(&self, char_id: i32) -> bool {
        let state = &self.olympiad.heroes;
        state.heroes.contains(&char_id) && state.all.get(&char_id).is_some_and(|h| h.claimed)
    }

    /// Java `Hero.isUnclaimedHero(objectId)`.
    pub fn is_unclaimed_hero(&self, char_id: i32) -> bool {
        let state = &self.olympiad.heroes;
        state.heroes.contains(&char_id) && state.all.get(&char_id).is_some_and(|h| !h.claimed)
    }

    /// Java `Hero.getHeroes()`: the heroes of the last period in Java's map order.
    pub fn heroes(&self) -> Vec<(i32, &HeroData)> {
        let state = &self.olympiad.heroes;
        state
            .ordered()
            .into_iter()
            .filter_map(|id| state.all.get(&id).map(|h| (id, h)))
            .collect()
    }

    /// Java `Hero.getHeroByClass(classId)`: the first hero of the class, 0 without one.
    pub fn hero_by_class(&self, class_id: i32) -> i32 {
        self.heroes()
            .into_iter()
            .find(|(_, h)| h.class_id == class_id)
            .map_or(0, |(id, _)| id)
    }

    /// Java `new ExHeroList()`: the heroes of the last period.
    pub fn hero_list_packet(&self) -> ExHeroList {
        ExHeroList {
            heroes: self
                .heroes()
                .into_iter()
                .map(|(_, h)| HeroEntry {
                    name: h.char_name.clone(),
                    class_id: h.class_id,
                    clan_name: h.clan_name.clone(),
                    clan_crest: h.clan_crest,
                    ally_name: h.ally_name.clone(),
                    ally_crest: h.ally_crest,
                    count: h.count,
                })
                .collect(),
        }
    }

    // ------------------------------------------------------------------------------- the load

    /// Java `new Hero()` (`init()`), after the Olympiad: the rows the game server read, see
    /// [`HeroLoad`]. Nothing when `OlympiadEnabled` is off.
    pub fn load_heroes(&mut self, load: HeroLoad) {
        if !self.olympiad.settings.enabled {
            return;
        }
        let state = &mut self.olympiad.heroes;
        state.diary.clear();
        state.fights.clear();
        state.counts.clear();
        state.messages.clear();
        state.dirty_messages.clear();
        state.heroes.clear();
        state.peak = 0;
        self.hero_replace_all(load, false);
        info!(
            "Hero System: Loaded {} Heroes.",
            self.olympiad.heroes.heroes.len()
        );
        info!(
            "Hero System: Loaded {} all time Heroes.",
            self.olympiad.heroes.all.len()
        );
    }

    /// The `StatSet` of a row, with the clan keys of `processHeros`.
    fn hero_data_of_row(&self, row: &HeroRow) -> HeroData {
        let (clan_name, clan_crest, ally_name, ally_crest) = self.hero_clan_info(row.clan_id, row.ally_id);
        HeroData {
            char_name: row.char_name.clone(),
            class_id: row.class_id,
            count: row.count,
            played: row.played,
            claimed: row.claimed,
            clan_name,
            clan_crest,
            ally_name,
            ally_crest,
        }
    }

    /// The clan keys of `processHeros` and `updateHeroes`: name and crest of the clan, and, with
    /// an alliance, the name and crest of the alliance.
    // NOTE(java-quirk): a clan `ClanTable` does not know is a `NullPointerException` in Java; here
    // the hero has no clan.
    fn hero_clan_info(&self, clan_id: i32, ally_id: i32) -> (String, i32, String, i32) {
        if clan_id > 0
            && let Some(clan) = self.clan(clan_id)
        {
            let (ally_name, ally_crest) = if ally_id > 0 {
                (clan.ally_name.clone().unwrap_or_default(), clan.ally_crest_id)
            } else {
                (String::new(), 0)
            };
            return (clan.name.clone(), clan.crest_id, ally_name, ally_crest);
        }
        (String::new(), 0, String::new(), 0)
    }

    /// The tables of a read: `rows` replace `COMPLETE_HEROS` and `HEROES` (`merge_local`: a claim
    /// made here survives), the details replace those of their heroes.
    fn hero_replace_all(&mut self, load: HeroLoad, merge_local: bool) {
        let HeroLoad { rows, details } = load;
        let mut all = BTreeMap::new();
        let mut order = Vec::new();
        for row in &rows {
            let mut data = self.hero_data_of_row(row);
            if merge_local
                && let Some(old) = self.olympiad.heroes.all.get(&row.char_id)
                && old.claimed
                && !data.claimed
                && old.played == 1
                && data.played == 1
                && old.count == data.count
            {
                data.claimed = true;
            }
            if row.played == 1 && !order.contains(&row.char_id) {
                order.push(row.char_id);
            }
            all.insert(row.char_id, data);
        }
        let state = &mut self.olympiad.heroes;
        state.all = all;
        state.heroes = order;
        state.peak = state.peak.max(state.heroes.len());
        self.hero_apply_details(details, true);
    }

    /// `loadMessage`, `loadDiary` and `loadFights` of the heroes (`with_messages`: the words
    /// replace the ones in memory, unless this server changed them).
    fn hero_apply_details(&mut self, details: Vec<HeroDetails>, with_messages: bool) {
        for detail in details {
            let char_id = detail.char_id;
            let diary: Vec<DiaryEntry> = detail
                .diary
                .iter()
                .map(|row| self.hero_diary_entry(row))
                .collect();
            let (fights, counts) = self.hero_fight_entries(char_id, &detail.fights);
            info!(
                "Hero System: Loaded {} diary entries for Hero: {char_id}",
                diary.len()
            );
            let state = &mut self.olympiad.heroes;
            state.diary.insert(char_id, diary);
            state.fights.insert(char_id, fights);
            state.counts.insert(char_id, counts);
            if with_messages
                && let Some(message) = detail.message
                && !state.dirty_messages.contains(&char_id)
            {
                state.messages.insert(char_id, message);
            }
        }
    }

    /// `loadDiary`'s text of one row.
    fn hero_diary_entry(&self, row: &HeroDiaryRow) -> DiaryEntry {
        let date = self.hero_diary_date(row.time);
        let action = match row.action {
            ACTION_RAID_KILLED => self
                .npc_world
                .template(row.param)
                .map(|t| format!("{} was defeated", t.name)),
            ACTION_HERO_GAINED => Some("Gained Hero status".to_owned()),
            // NOTE(java-quirk): "successfuly" is Java's spelling.
            ACTION_CASTLE_TAKEN => self
                .castle(CastleId(row.param))
                .map(|c| format!("{} Castle was successfuly taken", c.name)),
            _ => None,
        };
        DiaryEntry {
            date,
            action: action.unwrap_or_default(),
        }
    }

    /// `DIARY_DATE_FORMAT` in the JVM's zone (D-48).
    fn hero_diary_date(&self, time_ms: i64) -> String {
        format_local_date(time_ms + self.olympiad_offset_at(time_ms), false)
    }

    /// `loadFights`' entries and the count of victories, draws and losses.
    fn hero_fight_entries(&self, char_id: i32, rows: &[HeroFightRow]) -> (Vec<FightEntry>, (i32, i32, i32)) {
        let (mut victories, mut draws, mut losses) = (0, 0, 0);
        let mut fights = Vec::new();
        for row in rows {
            // `charId == charOneId`: the hero is side 1, else side 2.
            let (opponent, opponent_class, own_side) = if char_id == row.char_one_id {
                (&row.char_two_name, row.char_two_class, 1)
            } else if char_id == row.char_two_id {
                (&row.char_one_name, row.char_one_class, 2)
            } else {
                continue;
            };
            // `name != null && cls != null`
            let Some(name) = opponent else {
                continue;
            };
            let Some(class_name) = self
                .admin
                .class_list
                .get(ClassId(u16::try_from(opponent_class).unwrap_or(u16::MAX)))
                .map(|c| c.name.clone())
            else {
                continue;
            };
            let result = if row.winner == own_side {
                victories += 1;
                Some("<font color=\"00ff00\">victory</font>")
            } else if row.winner == 3 - own_side {
                losses += 1;
                Some("<font color=\"ff0000\">loss</font>")
            } else if row.winner == 0 {
                draws += 1;
                Some("<font color=\"ffff00\">draw</font>")
            } else {
                None
            };
            fights.push(FightEntry {
                opponent: name.clone(),
                opponent_class: class_name,
                time: calc_fight_time(row.time),
                start: format_local_date(row.start + self.olympiad_offset_at(row.start), true),
                classed: row.classed,
                result,
            });
        }
        (fights, (victories, draws, losses))
    }

    // ------------------------------------------------------------------------------- Player.setHero

    /// Java `Player.setHero(hero)`: the hero skills are given (to a player on its base class) or
    /// taken, the flag is set and the skill list sent. The skills are not stored.
    pub fn set_hero(&mut self, id: ObjectId, hero: bool) {
        if !self.players.contains_key(&id) {
            return;
        }
        self.hero_skills_for(id, hero);
        if let Some(p) = self.players.get_mut(&id) {
            p.olympiad.hero = hero;
        }
        self.send_skill_list(id);
    }

    /// The skill part of `setHero`.
    fn hero_skills_for(&mut self, id: ObjectId, hero: bool) {
        let Some(p) = self.players.get(&id) else {
            return;
        };
        let on_base_class = p.data.base_class == p.data.class_id;
        let trees = std::sync::Arc::clone(&self.rewards.skill_trees);
        for skill in trees.hero_skills() {
            if hero && on_base_class {
                self.add_skill(id, skill.skill_id, skill.skill_level);
            } else {
                // `removeSkill(skill, false, true)`: "Just remove skills from non-hero players."
                self.remove_skill(id, skill.skill_id, true);
            }
        }
    }

    /// `Player.restoreCharacter`'s `player.setHero(Hero.getInstance().isHero(objectId))` for a
    /// player who is entering: the flag and the skills, no skill list (the enter-world packets
    /// carry it).
    pub(crate) fn hero_on_restore(&mut self, id: ObjectId) {
        let hero = self.is_hero_char(id.0);
        if hero {
            self.hero_skills_for(id, true);
        }
        if let Some(p) = self.players.get_mut(&id) {
            p.olympiad.hero = hero;
        }
    }

    /// The hero skills `(skill id, level)` of a hero who is on its base class: the enter-world
    /// `SkillList` carries them next to the stored ones.
    pub(crate) fn hero_skill_levels(&self, id: ObjectId) -> Vec<(i32, i32)> {
        let on_base_class = self
            .players
            .get(&id)
            .is_some_and(|p| p.olympiad.hero && p.data.base_class == p.data.class_id);
        if !on_base_class {
            return Vec::new();
        }
        self.rewards
            .skill_trees
            .hero_skills()
            .iter()
            .map(|l| (l.skill_id, l.skill_level))
            .collect()
    }

    // ------------------------------------------------------------------------------- the choice

    /// Java `resetData()`: the diary, the history, the counts and the words go.
    pub(crate) fn hero_reset_data(&mut self) {
        let state = &mut self.olympiad.heroes;
        state.diary.clear();
        state.fights.clear();
        state.counts.clear();
        state.messages.clear();
        state.dirty_messages.clear();
    }

    /// Java `computeNewHeroes(newHeroes)`.
    pub(crate) fn hero_compute_new_heroes(&mut self, new_heroes: &[HeroToBe]) {
        for hero in new_heroes {
            info!(
                "Olympiad System: hero to be {} ({}), class {}",
                hero.char_name, hero.char_id, hero.class_id
            );
        }
        // `updateHeroes(true)`.
        if self.olympiad_writes_cycles() {
            self.hero_write(HeroWrite::ResetPlayed);
        }

        // The heroes of the last period who are online lose the status and the items.
        for char_id in self.olympiad.heroes.ordered() {
            let player = ObjectId(char_id);
            if !self.players.contains_key(&player) {
                continue;
            }
            self.set_hero(player, false);
            self.hero_take_items(player);
            self.broadcast_user_info(player);
        }

        // `deleteItemsInDb`: every offline hero's items (D-53: the Java game server runs it).
        if self.olympiad_writes_cycles() {
            self.hero_write(HeroWrite::DeleteItems);
        }
        self.olympiad.heroes.heroes.clear();

        if new_heroes.is_empty() {
            return;
        }

        let mut fresh: BTreeMap<i32, HeroData> = BTreeMap::new();
        let mut order: Vec<i32> = Vec::new();
        for hero in new_heroes {
            let char_id = hero.char_id;
            if let Some(old) = self.olympiad.heroes.all.get_mut(&char_id) {
                old.count = old.count.wrapping_add(1);
                old.played = 1;
                old.claimed = false;
            } else {
                // NOTE(java-quirk): a character listed twice (`HEROS_TO_BE` is never cleared) is
                // a new hero twice; the second `StatSet` replaces the first.
                fresh.insert(
                    char_id,
                    HeroData {
                        char_name: hero.char_name.clone(),
                        class_id: hero.class_id,
                        count: 1,
                        played: 1,
                        claimed: false,
                        ..HeroData::default()
                    },
                );
            }
            if !order.contains(&char_id) {
                order.push(char_id);
            }
        }
        {
            let state = &mut self.olympiad.heroes;
            state.heroes = order.clone();
            state.peak = state.peak.max(state.heroes.len());
        }

        // `updateHeroes(false)`: INSERT for a hero `COMPLETE_HEROS` does not know, else UPDATE.
        for char_id in order {
            if let Some(mut data) = fresh.remove(&char_id) {
                let (clan_id, ally_id) = self.hero_clan_ids(char_id);
                (data.clan_name, data.clan_crest, data.ally_name, data.ally_crest) =
                    self.hero_clan_info(clan_id, ally_id);
                if self.olympiad_writes_cycles() {
                    self.hero_write(HeroWrite::Insert {
                        char_id,
                        class_id: data.class_id,
                        count: data.count,
                        played: data.played,
                        claimed: data.claimed,
                    });
                }
                self.olympiad.heroes.all.insert(char_id, data);
            } else if let Some(data) = self.olympiad.heroes.all.get(&char_id)
                && self.olympiad_writes_cycles()
            {
                let write = HeroWrite::Update {
                    char_id,
                    class_id: data.class_id,
                    count: data.count,
                    played: data.played,
                    claimed: data.claimed,
                };
                self.hero_write(write);
            }
        }
    }

    /// The clan and alliance id `GET_CLAN_ALLY` reads for a new hero (`characters.clanid`): the
    /// clan the character is a member of.
    fn hero_clan_ids(&self, char_id: i32) -> (i32, i32) {
        self.clans()
            .find(|c| c.is_member(char_id))
            .map_or((0, 0), |c| (c.id, c.ally_id))
    }

    /// `updateHeroes(false)` for the heroes of the last period (`claimHero`): one UPDATE each.
    fn hero_update_heroes(&mut self) {
        if !self.olympiad_writes_cycles() {
            return;
        }
        let writes: Vec<HeroWrite> = self
            .olympiad
            .heroes
            .ordered()
            .into_iter()
            .filter_map(|char_id| {
                self.olympiad
                    .heroes
                    .all
                    .get(&char_id)
                    .map(|data| HeroWrite::Update {
                        char_id,
                        class_id: data.class_id,
                        count: data.count,
                        played: data.played,
                        claimed: data.claimed,
                    })
            })
            .collect();
        for write in writes {
            self.hero_write(write);
        }
    }

    /// The equipped hero items come off, then every hero item is destroyed
    /// (`computeNewHeroes`'s loops).
    fn hero_take_items(&mut self, player: ObjectId) {
        let is_hero_item = |world: &Self, item_id: i32| {
            world
                .item_templates()
                .and_then(|t| t.get(item_id).map(|t| t.is_hero_item()))
                .unwrap_or(false)
        };
        let mut scratch = Vec::new();
        for slot in 0..l2_protocol_game::server::charscreen::paperdoll::TOTAL_SLOTS {
            let Some(object_id) = self.paperdoll_object(player, slot) else {
                continue;
            };
            let Some(item_id) = self.inventory_item(player, object_id).map(|i| i.item_id) else {
                continue;
            };
            if is_hero_item(self, item_id) {
                self.set_paperdoll_item(player, slot, None, &mut scratch);
            }
        }
        let items: Vec<_> = self
            .players
            .get(&player)
            .map(|p| {
                p.data
                    .items
                    .iter()
                    .filter(|i| i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL)
                    .filter(|i| is_hero_item(self, i.item_id))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let mut removed = Vec::new();
        for item in items {
            if self.destroy_item_by_object_id(player, item.object_id, item.count, true) {
                removed.push(self.update_entry(&item, InventoryUpdate::REMOVED));
            }
        }
        for entry in removed {
            self.send_inventory_update(player, entry);
        }
    }

    // ------------------------------------------------------------------------------- the writes

    fn hero_write(&mut self, write: HeroWrite) {
        self.olympiad_write(OlympiadWrite::Hero(write));
    }

    /// Java `setDiaryData(charId, action, param)`: the row is written as Java does (the deed of
    /// the hero, D-53).
    pub fn hero_set_diary_data(&mut self, char_id: i32, action: i32, param: i32) {
        let time = self.olympiad_now_ms();
        self.hero_write(HeroWrite::Diary {
            char_id,
            time,
            action,
            param,
        });
    }

    /// Java `setHeroGained(charId)`.
    pub fn hero_set_hero_gained(&mut self, char_id: i32) {
        self.hero_set_diary_data(char_id, ACTION_HERO_GAINED, 0);
    }

    /// Java `setRBkilled(charId, npcId)`: the row, and the line in the diary the hero has.
    pub fn hero_set_rb_killed(&mut self, char_id: i32, npc_id: i32) {
        self.hero_set_diary_data(char_id, ACTION_RAID_KILLED, npc_id);
        let Some(name) = self.npc_world.template(npc_id).map(|t| t.name.clone()) else {
            return;
        };
        let date = self.hero_diary_date(self.olympiad_now_ms());
        if let Some(list) = self.olympiad.heroes.diary.get_mut(&char_id) {
            list.push(DiaryEntry {
                date,
                action: format!("{name} was defeated"),
            });
        }
    }

    /// Java `setCastleTaken(charId, castleId)`.
    pub fn hero_set_castle_taken(&mut self, char_id: i32, castle_id: i32) {
        self.hero_set_diary_data(char_id, ACTION_CASTLE_TAKEN, castle_id);
        let Some(name) = self.castle(CastleId(castle_id)).map(|c| c.name.clone()) else {
            return;
        };
        let date = self.hero_diary_date(self.olympiad_now_ms());
        if let Some(list) = self.olympiad.heroes.diary.get_mut(&char_id) {
            list.push(DiaryEntry {
                date,
                action: format!("{name} Castle was successfuly taken"),
            });
        }
    }

    /// `Siege.endSiege`: `Hero.setCastleTaken` for every online noble member of the new owner.
    pub(crate) fn hero_castle_taken_by_clan(&mut self, clan_id: i32, castle_id: i32) {
        for member in self.clan_online_members(clan_id, 0) {
            if self.is_noble(member) {
                self.hero_set_castle_taken(member.0, castle_id);
            }
        }
    }

    /// Java `Hero.shutdown()`: `saveHeroMessage` of every message. With `CycleDatabaseWrites`
    /// off only the words this server changed go out (D-53).
    pub(crate) fn hero_shutdown(&mut self) {
        let all = self.olympiad_writes_cycles();
        let state = &mut self.olympiad.heroes;
        let writes: Vec<HeroWrite> = state
            .messages
            .iter()
            .filter(|(char_id, _)| all || state.dirty_messages.contains(char_id))
            .map(|(char_id, message)| HeroWrite::Message {
                char_id: *char_id,
                message: message.clone(),
            })
            .collect();
        state.dirty_messages.clear();
        for write in writes {
            self.hero_write(write);
        }
    }

    // ------------------------------------------------------------------------------- hero words

    /// Java `RequestWriteHeroWords.runImpl`.
    pub(crate) fn on_write_hero_words(&mut self, id: ObjectId, words: &str) {
        if !self.is_hero(id) {
            return;
        }
        if words.encode_utf16().count() > MAX_HERO_WORDS {
            return;
        }
        let state = &mut self.olympiad.heroes;
        state.messages.insert(id.0, words.to_owned());
        state.dirty_messages.insert(id.0);
    }

    // ------------------------------------------------------------------------------- the reads

    /// Asks the persistence task for the hero tables (the answer is `on_hero_reread`).
    fn hero_reread(&mut self, only: Option<i32>, player: ObjectId, npc: ObjectId, action: HeroAction) {
        let before = self.hero_fights_cutoff();
        self.outbox.persist(PersistCommand::OlympiadRereadRequest {
            request: OlympiadRereadRequest::Heroes { before, only },
            then: OlympiadRereadThen {
                player,
                npc,
                action: OlympiadRereadAction::Hero(action),
            },
        });
    }

    /// Runs `action` now (the switch is on: the memory is the truth) or after a fresh read of the
    /// hero tables (D-50).
    fn hero_fresh(&mut self, player: ObjectId, npc: ObjectId, action: HeroAction) {
        if self.olympiad_writes_cycles() {
            self.hero_do(player, npc, action);
        } else {
            self.hero_reread(None, player, npc, action);
        }
    }

    /// The answer of a read of the hero tables: a failed read shows and does nothing (D-50).
    pub(crate) fn on_hero_reread(
        &mut self,
        result: Result<OlympiadRereadRows, ()>,
        player: ObjectId,
        npc: ObjectId,
        action: HeroAction,
    ) {
        let Ok(OlympiadRereadRows::Heroes(load)) = result else {
            return;
        };
        if action == HeroAction::Details {
            self.hero_apply_details(load.details, false);
            return;
        }
        self.hero_replace_all(*load, true);
        self.hero_do(player, npc, action);
    }

    /// What a read was for.
    fn hero_do(&mut self, player: ObjectId, npc: ObjectId, action: HeroAction) {
        if !self.players.contains_key(&player) {
            return;
        }
        match action {
            HeroAction::List => {
                let list = self.hero_list_packet();
                self.send(player, &list);
            }
            HeroAction::Certification => self.hero_certify(player, npc),
            HeroAction::Diary { class_id, page } => {
                let char_id = self.hero_by_class(class_id);
                if char_id > 0 {
                    self.show_hero_diary(player, class_id, char_id, page);
                }
            }
            HeroAction::Fights { class_id, page } => {
                let char_id = self.hero_by_class(class_id);
                if char_id > 0 {
                    self.show_hero_fights(player, class_id, char_id, page);
                }
            }
            HeroAction::GiveHero { gm } => self.hero_give_hero(gm, player),
            HeroAction::Details => {}
        }
    }

    // ------------------------------------------------------------------------------- the NPC

    /// `olympiad 4`: `player.sendPacket(new ExHeroList())`.
    pub(crate) fn hero_send_list(&mut self, player: ObjectId, npc: ObjectId) {
        self.hero_fresh(player, npc, HeroAction::List);
    }

    /// `olympiad 5`: the hero certification.
    pub(crate) fn hero_certification(&mut self, player: ObjectId, npc: ObjectId) {
        self.hero_fresh(player, npc, HeroAction::Certification);
    }

    fn hero_certify(&mut self, player: ObjectId, npc: ObjectId) {
        if self.is_unclaimed_hero(player.0) {
            self.claim_hero(player);
            self.olympiad_page(player, npc, "hero_receive.htm", &[]);
        } else {
            self.olympiad_page(player, npc, "hero_notreceive.htm", &[]);
        }
    }

    /// Java `claimHero(player)`.
    pub fn claim_hero(&mut self, player: ObjectId) {
        let char_id = player.0;
        // NOTE: Java makes an empty `StatSet` for a player who is no hero and then throws in
        // `updateHeroes`; both callers check `isUnclaimedHero` first.
        let Some(hero) = self.olympiad.heroes.all.get_mut(&char_id) else {
            warn!("Hero System: {player} cannot claim the hero status (not a hero).");
            return;
        };
        hero.claimed = true;

        let clan = self.clan_of(player).filter(|c| c.level >= 5).map(|c| c.id);
        if let Some(clan_id) = clan {
            let points = self.olympiad.settings.hero_reputation_points;
            self.add_reputation_score(clan_id, points);
            let name = self.object_name(player).unwrap_or_default();
            let message = SystemMessage::new(
                SystemMessageId::ClanMemberC1WasNamedAHeroS2PointsHaveBeenAddedToYourClanSReputationScore,
            )
            .with(SystemMessageParam::Text(name))
            .with(SystemMessageParam::IntNumber(points));
            self.broadcast_to_clan(clan_id, &message);
        }

        self.set_hero(player, true);
        self.broadcast_creature_packet(
            player,
            &SocialAction {
                object_id: char_id,
                action_id: HERO_ANIMATION,
            },
        );
        self.broadcast_user_info(player);

        // "Set Gained hero and reload data."
        self.hero_set_hero_gained(char_id);
        // `loadFights` and `loadDiary` (after the row above, the outbox keeps the order).
        self.hero_reread(Some(char_id), player, ObjectId(0), HeroAction::Details);
        let state = &mut self.olympiad.heroes;
        state.messages.insert(char_id, String::new());
        state.dirty_messages.insert(char_id);

        self.hero_update_heroes();
    }

    // ------------------------------------------------------------------------------- the pages

    /// `RequestBypassToServer`'s `_match` and `_diary`: `<command>?class=<id>&page=<n>`.
    pub(crate) fn hero_history_bypass(&mut self, player: ObjectId, command: &str) {
        // `_command.substring(_command.indexOf('?') + 1)`
        let params = command.find('?').map_or(command, |at| &command[at + 1..]);
        // `StringTokenizer(params, "&")` skips empty tokens; `token.split("=")[1]`.
        let mut tokens = params.split('&').filter(|t| !t.is_empty());
        let value = |token: Option<&str>| -> Option<i32> {
            let text = token?.split('=').nth(1)?;
            text.parse::<i32>().ok()
        };
        let (Some(class_id), Some(page)) = (value(tokens.next()), value(tokens.next())) else {
            warn!("Exception processing bypass from {player}: {command}");
            return;
        };
        let action = if command.starts_with("_match") {
            HeroAction::Fights { class_id, page }
        } else {
            HeroAction::Diary { class_id, page }
        };
        self.hero_fresh(player, ObjectId(0), action);
    }

    /// Java `showHeroDiary(player, heroclass, charid, page)`.
    fn show_hero_diary(&mut self, player: ObjectId, hero_class: i32, char_id: i32, page: i32) {
        let state = &self.olympiad.heroes;
        let Some(main_list) = state.diary.get(&char_id) else {
            return;
        };
        let Some(hero_message) = state.messages.get(&char_id) else {
            return;
        };
        let Some(mut html) = self.get_htm(player, &format!("{OLYMPIAD_HTML_PATH}herodiary.htm")) else {
            return;
        };
        let name = state
            .all
            .get(&char_id)
            .map(|h| h.char_name.clone())
            .unwrap_or_default();
        if java_html_replace(&mut html, "%heroname%", &name).is_none()
            || java_html_replace(&mut html, "%message%", hero_message).is_none()
        {
            warn!("Hero System: the message of hero {char_id} cannot be shown.");
            return;
        }
        if !main_list.is_empty() {
            let list: Vec<&DiaryEntry> = main_list.iter().rev().collect();
            let mut color = true;
            let mut f_list = String::new();
            let mut counter = 0;
            let mut break_at: usize = 0;
            // `list.get(negative)` throws.
            let Ok(first) = usize::try_from((i64::from(page) - 1) * i64::from(DIARY_PER_PAGE)) else {
                warn!("Exception in showHeroDiary: page {page}");
                return;
            };
            for (i, entry) in list.iter().enumerate().skip(first) {
                break_at = i;
                f_list.push_str("<tr><td>");
                f_list.push_str(if color {
                    "<table width=270 bgcolor=\"131210\">"
                } else {
                    "<table width=270>"
                });
                f_list.push_str(&format!(
                    "<tr><td width=270><font color=\"LEVEL\">{}:xx</font></td></tr>",
                    entry.date
                ));
                f_list.push_str(&format!("<tr><td width=270>{}</td></tr>", entry.action));
                f_list.push_str("<tr><td>&nbsp;</td></tr></table>");
                f_list.push_str("</td></tr>");
                color = !color;
                counter += 1;
                if counter >= DIARY_PER_PAGE {
                    break;
                }
            }
            let prev = if break_at + 1 < list.len() {
                format!(
                    "<button value=\"Prev\" action=\"bypass _diary?class={hero_class}&page={}\" width=60 height=25 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">",
                    page.wrapping_add(1)
                )
            } else {
                String::new()
            };
            let next = if page > 1 {
                format!(
                    "<button value=\"Next\" action=\"bypass _diary?class={hero_class}&page={}\" width=60 height=25 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">",
                    page - 1
                )
            } else {
                String::new()
            };
            let _ = java_html_replace(&mut html, "%buttprev%", &prev);
            let _ = java_html_replace(&mut html, "%buttnext%", &next);
            let _ = java_html_replace(&mut html, "%list%", &f_list);
        } else {
            let _ = java_html_replace(&mut html, "%list%", "");
            let _ = java_html_replace(&mut html, "%buttprev%", "");
            let _ = java_html_replace(&mut html, "%buttnext%", "");
        }
        // `new NpcHtmlMessage()` and `disableValidation()`.
        if let Some(message) = NpcHtmlMessage::new(0, &html) {
            self.send_html_unvalidated(player, &message);
        }
    }

    /// Java `showHeroFights(player, heroclass, charid, page)`.
    fn show_hero_fights(&mut self, player: ObjectId, hero_class: i32, char_id: i32, page: i32) {
        let state = &self.olympiad.heroes;
        let Some(hero_fights) = state.fights.get(&char_id) else {
            return;
        };
        let Some(mut html) = self.get_htm(player, &format!("{OLYMPIAD_HTML_PATH}herohistory.htm")) else {
            return;
        };
        let name = state
            .all
            .get(&char_id)
            .map(|h| h.char_name.clone())
            .unwrap_or_default();
        let _ = java_html_replace(&mut html, "%heroname%", &name);
        let (mut win, mut loss, mut draw) = (0, 0, 0);
        if !hero_fights.is_empty() {
            if let Some(&(victories, draws, losses)) = state.counts.get(&char_id) {
                (win, draw, loss) = (victories, draws, losses);
            }
            let mut color = true;
            let mut f_list = String::new();
            let mut counter = 0;
            let mut break_at: usize = 0;
            let Ok(first) = usize::try_from((i64::from(page) - 1) * i64::from(FIGHTS_PER_PAGE)) else {
                warn!("Exception in showHeroFights: page {page}");
                return;
            };
            for (i, fight) in hero_fights.iter().enumerate().skip(first) {
                break_at = i;
                let Some(result) = fight.result else {
                    // `fight.getString("result")` of a missing key throws.
                    warn!("Exception in showHeroFights: no result");
                    return;
                };
                f_list.push_str("<tr><td>");
                f_list.push_str(if color {
                    "<table width=270 bgcolor=\"131210\">"
                } else {
                    "<table width=270>"
                });
                // NOTE(java-quirk): the "non-cls" font is never closed (`<font>` again).
                f_list.push_str(&format!(
                    "<tr><td width=220><font color=\"LEVEL\">{}</font>&nbsp;&nbsp;{}</td><td width=50 align=right>{}</td></tr>",
                    fight.start,
                    result,
                    if fight.classed > 0 {
                        "<font color=\"FFFF99\">cls</font>"
                    } else {
                        "<font color=\"999999\">non-cls<font>"
                    }
                ));
                f_list.push_str(&format!(
                    "<tr><td width=220>vs {} ({})</td><td width=50 align=right>({})</td></tr>",
                    fight.opponent, fight.opponent_class, fight.time
                ));
                f_list.push_str("<tr><td colspan=2>&nbsp;</td></tr></table>");
                f_list.push_str("</td></tr>");
                color = !color;
                counter += 1;
                if counter >= FIGHTS_PER_PAGE {
                    break;
                }
            }
            let prev = if break_at + 1 < hero_fights.len() {
                format!(
                    "<button value=\"Prev\" action=\"bypass _match?class={hero_class}&page={}\" width=60 height=25 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">",
                    page.wrapping_add(1)
                )
            } else {
                String::new()
            };
            let next = if page > 1 {
                format!(
                    "<button value=\"Next\" action=\"bypass _match?class={hero_class}&page={}\" width=60 height=25 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\">",
                    page - 1
                )
            } else {
                String::new()
            };
            let _ = java_html_replace(&mut html, "%buttprev%", &prev);
            let _ = java_html_replace(&mut html, "%buttnext%", &next);
            let _ = java_html_replace(&mut html, "%list%", &f_list);
        } else {
            let _ = java_html_replace(&mut html, "%list%", "");
            let _ = java_html_replace(&mut html, "%buttprev%", "");
            let _ = java_html_replace(&mut html, "%buttnext%", "");
        }
        let _ = java_html_replace(&mut html, "%win%", &win.to_string());
        let _ = java_html_replace(&mut html, "%draw%", &draw.to_string());
        let _ = java_html_replace(&mut html, "%loos%", &loss.to_string());
        if let Some(message) = NpcHtmlMessage::new(0, &html) {
            self.send_html(player, &message);
        }
    }

    // ------------------------------------------------------------------------------- admin

    /// `admin_givehero`'s checks and the claim, after the read.
    fn hero_give_hero(&mut self, gm: ObjectId, target: ObjectId) {
        if self.is_hero_char(target.0) {
            self.send_sys_message(gm, "This player has already claimed the hero status.");
            return;
        }
        if !self.is_unclaimed_hero(target.0) {
            self.send_sys_message(gm, "This player cannot claim the hero status.");
            return;
        }
        self.claim_hero(target);
    }
}

/// Java `calcFightTime(fightTimeValue)`: `mm:ss` of the fight, the minutes within the hour.
fn calc_fight_time(fight_time_value: i64) -> String {
    let fight_time = fight_time_value / 1000;
    format!("{:02}:{:02}", (fight_time % 3600) / 60, fight_time % 60)
}

// ------------------------------------------------------------------------------- admin words

/// `admin_sethero` and `admin_givehero` (`AdminAdmin`).
pub fn register_hero_admin(handlers: &mut AdminHandlers) {
    let set_hero: AdminHandler = admin_sethero;
    handlers.register("admin_sethero", set_hero);
    let give_hero: AdminHandler = admin_givehero;
    handlers.register("admin_givehero", give_hero);
}

/// The target when it is a player, else the GM (`getTarget().isPlayer() ? target : activeChar`);
/// `None` without a target (the GM gets "Invalid target").
fn admin_target(world: &World, gm: ObjectId) -> Option<ObjectId> {
    match world.target_of(gm) {
        None => {
            world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
            None
        }
        Some(t) if world.is_player(t) => Some(t),
        Some(_) => Some(gm),
    }
}

fn admin_sethero(world: &mut World, gm: ObjectId, _command: &str) -> Result<(), JavaException> {
    let Some(target) = admin_target(world, gm) else {
        return Ok(());
    };
    let hero = world.is_hero(target);
    world.set_hero(target, !hero);
    world.broadcast_user_info(target);
    Ok(())
}

fn admin_givehero(world: &mut World, gm: ObjectId, _command: &str) -> Result<(), JavaException> {
    let Some(target) = admin_target(world, gm) else {
        return Ok(());
    };
    world.hero_fresh(target, ObjectId(0), HeroAction::GiveHero { gm });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fight_time_is_minutes_and_seconds_within_the_hour() {
        assert_eq!(calc_fight_time(0), "00:00");
        assert_eq!(calc_fight_time(61_000), "01:01");
        // 1 h 5 min 9 s: the hours are dropped.
        assert_eq!(calc_fight_time(3_909_000), "05:09");
    }

    #[test]
    fn html_replace_follows_java() {
        let mut html = "a %x% b %x%".to_owned();
        assert!(java_html_replace(&mut html, "%x%", "cost $5").is_some());
        assert_eq!(html, "a cost $5 b cost $5");
        // A backslash escapes the next character.
        let mut html = "%x%".to_owned();
        assert!(java_html_replace(&mut html, "%x%", "a\\nb").is_some());
        assert_eq!(html, "anb");
        // A backslash at the end throws.
        let mut html = "%x%".to_owned();
        assert!(java_html_replace(&mut html, "%x%", "end\\").is_none());
        assert_eq!(html, "%x%");
        // Two backslashes are one, then the escaped `$`; a single one before `$` leaves a bare
        // `$` for Java to reject.
        let mut html = "%x%".to_owned();
        assert!(java_html_replace(&mut html, "%x%", "\\\\$").is_some());
        assert_eq!(html, "\\$");
        let mut html = "%x%".to_owned();
        assert!(java_html_replace(&mut html, "%x%", "\\$").is_none());
    }
}
