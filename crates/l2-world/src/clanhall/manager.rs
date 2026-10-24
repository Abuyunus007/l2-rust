//! The `ClanHallManager` NPC (the chamberlain of a hall): the pages where a clan buys, changes and
//! stops the hall functions, the hall's buffs, teleport, item creation, doors and banishing.
// Java: entity/actor/instance/ClanHallManager.java
//
// # The functions
// The Java class repeats one block per function (HP, MP and exp recovery; teleport, support and
// item creation; curtains and front platform). They differ only in their names, texts, fee tables
// and the levels a hall grade offers, so a table (`SPECS`) describes them and one routine runs
// every page. The pages, messages, bypass commands and the order of the checks are Java's.
//
// NOTE(java-quirk): a purchase that fails for lack of adena sends the "low adena" page twice.
//
// # Who may use it
// Only the owning clan (or a GM) gets past the condition; everybody else gets nothing, not even the
// generic NPC commands (`condition <= COND_ALL_FALSE` returns first).

use l2_data::SkillTemplate;
use l2_protocol_game::server::{ActionFailed, NpcHtmlMessage};
use l2_util::ObjectId;
use tracing::warn;

use super::ClanHallSettings;
use super::format::date_time;
use crate::clan::ClanAccess;
use crate::effects::{EffectType, SkillKey};
use crate::residence::{
    ClanHallId, HALL_FUNC_DECO_CURTAINS, HALL_FUNC_DECO_FRONTPLATEFORM, HALL_FUNC_ITEM_CREATE,
    HALL_FUNC_RESTORE_EXP, HALL_FUNC_RESTORE_HP, HALL_FUNC_RESTORE_MP, HALL_FUNC_SUPPORT, HALL_FUNC_TELEPORT,
    ResidenceRef,
};
use crate::world::World;

/// `distance to the zone` Java asks for the nearest auctionable hall.
const NEARBY_HALL_DISTANCE: i32 = 500;
/// `CHSiegeManager.getNearbyClanHall(creature)`.
const NEARBY_SIEGABLE_HALL_DISTANCE: i32 = 10_000;

/// `ClanHallManager.COND_*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Condition {
    OwnerFalse = 0,
    AllFalse = 1,
    Owner = 3,
}

/// Which bypass group a function belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    /// `manage recovery ...`: the percent functions.
    Recovery,
    /// `manage other ...`: teleport, support, item creation.
    Other,
    /// `manage deco ...`: curtains and front platform (auctionable halls only).
    Deco,
}

impl Group {
    fn word(self) -> &'static str {
        match self {
            Self::Recovery => "recovery",
            Self::Other => "other",
            Self::Deco => "deco",
        }
    }
}

/// One hall function as the pages treat it.
struct Spec {
    function: i32,
    group: Group,
    /// The word of the apply command (`hp` in `manage recovery hp 20`), also the prefix of
    /// `<word>_cancel` and `edit_<word>`.
    word: &'static str,
    /// The `edit_<x>` word where it differs from the apply word (`edit_curtains`).
    edit: &'static str,
    cancel: &'static str,
    /// The name on the `functions-apply.htm` page.
    name: &'static str,
    /// The `%use%` text; `{}` is the level.
    use_text: &'static str,
    /// The level as the pages print it.
    percent: bool,
    /// The placeholders of the overview page: the value, the paid-until text and the links.
    value_key: &'static str,
    period_key: &'static str,
    change_key: &'static str,
    /// The fee slot of a level (`switch (level) { case ...: CH_*N_FEE }`).
    slot: fn(i32) -> usize,
    /// The days between two fees and the fee of a slot.
    fees: fn(&ClanHallSettings) -> (i64, &[i32]),
    /// The ratio in milliseconds (the `updateFunctions` rate).
    ratio: fn(&ClanHallSettings) -> i64,
    /// The levels a hall grade offers (`None`: the page keeps the placeholder).
    levels: fn(i32) -> Option<&'static [i32]>,
}

fn slot_hp(level: i32) -> usize {
    match level {
        20 => 1,
        40 => 2,
        80 => 3,
        100 => 4,
        120 => 5,
        140 => 6,
        160 => 7,
        180 => 8,
        200 => 9,
        220 => 10,
        240 => 11,
        260 => 12,
        _ => 13,
    }
}

fn slot_mp(level: i32) -> usize {
    match level {
        5 => 1,
        10 => 2,
        15 => 3,
        30 => 4,
        _ => 5,
    }
}

fn slot_exp(level: i32) -> usize {
    match level {
        5 => 1,
        10 => 2,
        15 => 3,
        25 => 4,
        35 => 5,
        40 => 6,
        _ => 7,
    }
}

fn slot_item(level: i32) -> usize {
    match level {
        1 => 1,
        2 => 2,
        _ => 3,
    }
}

fn slot_two(level: i32) -> usize {
    if level == 1 { 1 } else { 2 }
}

fn slot_support(level: i32) -> usize {
    match level {
        1..=7 => level as usize,
        _ => 8,
    }
}

fn hp_levels(grade: i32) -> Option<&'static [i32]> {
    match grade {
        0 => Some(&[20, 40, 220]),
        1 => Some(&[40, 100, 160]),
        2 => Some(&[80, 140, 200, 260]),
        3 => Some(&[80, 120, 180, 240, 300]),
        _ => None,
    }
}

fn mp_levels(grade: i32) -> Option<&'static [i32]> {
    match grade {
        0 => Some(&[5, 10, 25]),
        1 => Some(&[5, 15, 25]),
        2 => Some(&[5, 15, 30]),
        3 => Some(&[5, 15, 30, 40]),
        _ => None,
    }
}

fn exp_levels(grade: i32) -> Option<&'static [i32]> {
    match grade {
        0 => Some(&[5, 10, 25]),
        1 => Some(&[5, 15, 30]),
        2 => Some(&[5, 15, 25, 40]),
        3 => Some(&[15, 25, 35, 50]),
        _ => None,
    }
}

fn support_levels(grade: i32) -> Option<&'static [i32]> {
    match grade {
        0 => Some(&[1, 2]),
        1 => Some(&[1, 2, 4]),
        2 => Some(&[3, 4, 5]),
        3 => Some(&[3, 5, 7, 8]),
        _ => None,
    }
}

fn two_levels(_grade: i32) -> Option<&'static [i32]> {
    Some(&[1, 2])
}

fn three_levels(_grade: i32) -> Option<&'static [i32]> {
    Some(&[1, 2, 3])
}

const SPECS: [Spec; 8] = [
    Spec {
        function: HALL_FUNC_RESTORE_HP,
        group: Group::Recovery,
        word: "hp",
        edit: "edit_hp",
        cancel: "hp_cancel",
        name: "Fireplace (HP Recovery Device)",
        use_text: "Provides additional HP recovery for clan members in the clan hall.<font color=\"00FFFF\">{}%</font>",
        percent: true,
        value_key: "hp_recovery",
        period_key: "hp_period",
        change_key: "change_hp",
        slot: slot_hp,
        fees: |s| (s.hp.days(), &s.hp.fees[..]),
        ratio: |s| s.hp.ratio,
        levels: hp_levels,
    },
    Spec {
        function: HALL_FUNC_RESTORE_MP,
        group: Group::Recovery,
        word: "mp",
        edit: "edit_mp",
        cancel: "mp_cancel",
        name: "Carpet (MP Recovery)",
        use_text: "Provides additional MP recovery for clan members in the clan hall.<font color=\"00FFFF\">{}%</font>",
        percent: true,
        value_key: "mp_recovery",
        period_key: "mp_period",
        change_key: "change_mp",
        slot: slot_mp,
        fees: |s| (s.mp.days(), &s.mp.fees[..]),
        ratio: |s| s.mp.ratio,
        levels: mp_levels,
    },
    Spec {
        function: HALL_FUNC_RESTORE_EXP,
        group: Group::Recovery,
        word: "exp",
        edit: "edit_exp",
        cancel: "exp_cancel",
        name: "Chandelier (EXP Recovery Device)",
        use_text: "Restores the Exp of any clan member who is resurrected in the clan hall.<font color=\"00FFFF\">{}%</font>",
        percent: true,
        value_key: "exp_recovery",
        period_key: "exp_period",
        change_key: "change_exp",
        slot: slot_exp,
        fees: |s| (s.exp.days(), &s.exp.fees[..]),
        ratio: |s| s.exp.ratio,
        levels: exp_levels,
    },
    Spec {
        function: HALL_FUNC_ITEM_CREATE,
        group: Group::Other,
        word: "item",
        edit: "edit_item",
        cancel: "item_cancel",
        name: "Magic Equipment (Item Production Facilities)",
        use_text: "Allow the purchase of special items at fixed intervals.",
        percent: false,
        value_key: "item",
        period_key: "item_period",
        change_key: "change_item",
        slot: slot_item,
        fees: |s| (s.item.days(), &s.item.fees[..]),
        ratio: |s| s.item.ratio,
        levels: three_levels,
    },
    Spec {
        function: HALL_FUNC_SUPPORT,
        group: Group::Other,
        word: "support",
        edit: "edit_support",
        cancel: "support_cancel",
        name: "Insignia (Supplementary Magic)",
        use_text: "Enables the use of supplementary magic.",
        percent: false,
        value_key: "support",
        period_key: "support_period",
        change_key: "change_support",
        slot: slot_support,
        fees: |s| (s.support.days(), &s.support.fees[..]),
        ratio: |s| s.support.ratio,
        levels: support_levels,
    },
    Spec {
        function: HALL_FUNC_TELEPORT,
        group: Group::Other,
        word: "tele",
        edit: "edit_tele",
        cancel: "tele_cancel",
        name: "Mirror (Teleportation Device)",
        use_text: "Teleports clan members in a clan hall to the target <font color=\"00FFFF\">Stage {}</font> staging area",
        percent: false,
        value_key: "tele",
        period_key: "tele_period",
        change_key: "change_tele",
        slot: slot_two,
        fees: |s| (s.tele.days(), &s.tele.fees[..]),
        ratio: |s| s.tele.ratio,
        levels: two_levels,
    },
    Spec {
        function: HALL_FUNC_DECO_CURTAINS,
        group: Group::Deco,
        word: "curtains",
        edit: "edit_curtains",
        cancel: "curtains_cancel",
        name: "Curtains (Decoration)",
        use_text: "These curtains can be used to decorate the clan hall.",
        percent: false,
        value_key: "curtain",
        period_key: "curtain_period",
        change_key: "change_curtain",
        slot: slot_two,
        fees: |s| (s.curtain.days(), &s.curtain.fees[..]),
        ratio: |s| s.curtain.ratio,
        levels: two_levels,
    },
    Spec {
        function: HALL_FUNC_DECO_FRONTPLATEFORM,
        group: Group::Deco,
        word: "fixtures",
        edit: "edit_fixtures",
        cancel: "fixtures_cancel",
        name: "Front Platform (Decoration)",
        use_text: "Used to decorate the clan hall.",
        percent: false,
        value_key: "fixture",
        period_key: "fixture_period",
        change_key: "change_fixture",
        slot: slot_two,
        fees: |s| (s.front.days(), &s.front.fees[..]),
        ratio: |s| s.front.ratio,
        levels: two_levels,
    },
];

/// `html.replace(pattern, value)`.
fn rep(html: &mut NpcHtmlMessage, from: &str, to: impl std::fmt::Display) {
    html.html = html.html.replace(from, &to.to_string());
}

impl Spec {
    /// The links `[<a ...>20%</a>]...` of the levels a grade offers.
    fn edit_links(&self, grade: i32) -> Option<String> {
        let levels = (self.levels)(grade)?;
        let mut links = String::new();
        for level in levels {
            let text = if self.percent {
                format!("{level}%")
            } else {
                format!("Level {level}")
            };
            links.push_str(&format!(
                "[<a action=\"bypass -h npc_%objectId%_manage {} {} {level}\">{text}</a>]",
                self.group.word(),
                self.edit,
            ));
        }
        Some(links)
    }

    /// `[<a ...>Deactivate</a>]`.
    fn deactivate_link(&self) -> String {
        format!(
            "[<a action=\"bypass -h npc_%objectId%_manage {} {}\">Deactivate</a>]",
            self.group.word(),
            self.cancel
        )
    }

    /// The text `%used%` shows for a level.
    fn level_text(&self, level: &str) -> String {
        if self.percent {
            format!("{level}%")
        } else {
            format!("Stage {level}")
        }
    }
}

impl World {
    // ------------------------------------------------------------------ the hall of an NPC

    /// Java `ClanHallManager.getClanHall()`: the nearest auctionable hall within 500, else the
    /// nearest conquerable one within 10000, found at the first use and kept.
    fn manager_hall(&mut self, npc: ObjectId) -> Option<ClanHallId> {
        if let Some(Some(id)) = self.clanhall.npc_halls.get(&npc) {
            return Some(*id);
        }
        let (x, y, _) = self.position_of(npc)?;
        let hall = self
            .nearby_auctionable_hall(x, y, NEARBY_HALL_DISTANCE)
            .or_else(|| self.nearby_siegable_hall(x, y, NEARBY_SIEGABLE_HALL_DISTANCE));
        if hall.is_some() {
            self.clanhall.npc_halls.insert(npc, hall);
        }
        hall
    }

    /// Java `CHSiegeManager.getNearbyClanHall(x, y, maxDist)`: a conquerable hall whose zone is
    /// closer than `max_distance`.
    pub(crate) fn nearby_siegable_hall(&self, x: i32, y: i32, max_distance: i32) -> Option<ClanHallId> {
        self.clan_halls()
            .filter(|h| h.is_siegable_hall())
            .find(|h| {
                self.clan_hall_zone(h.id)
                    .is_some_and(|zone| zone.distance_to_zone(x, y) < f64::from(max_distance))
            })
            .map(|h| h.id)
    }

    /// Java `ClanHallManager.validateCondition(player)`.
    fn manager_condition(&self, hall: Option<ClanHallId>, player: ObjectId) -> Condition {
        let Some(hall) = hall.and_then(|id| self.clan_hall(id)) else {
            return Condition::AllFalse;
        };
        if self.is_gm(player) {
            return Condition::Owner;
        }
        match self.clan_of(player) {
            Some(clan) if hall.owner_id == clan.id => Condition::Owner,
            Some(_) => Condition::OwnerFalse,
            None => Condition::AllFalse,
        }
    }

    /// Java `ClanHallManager.sendHtmlMessage(player, html)`.
    fn manager_send(&mut self, npc: ObjectId, player: ObjectId, mut html: NpcHtmlMessage) {
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        rep(&mut html, "%objectId%", npc.0);
        rep(&mut html, "%npcId%", npc_id);
        self.send_html(player, &html);
    }

    /// `html.setFile(player, path)` and `sendHtmlMessage` for a page without replacements.
    fn manager_send_file(&mut self, npc: ObjectId, player: ObjectId, path: &str) {
        if let Some(html) = self.html_from_file(player, npc.0, path) {
            self.manager_send(npc, player, html);
        }
    }

    // ------------------------------------------------------------------ chat window

    /// Java `ClanHallManager.showChatWindow(player)`.
    pub(crate) fn clan_hall_manager_show_chat_window(&mut self, npc: ObjectId, player: ObjectId) {
        self.send(player, &ActionFailed);
        let hall = self.manager_hall(npc);
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        let path = match self.manager_condition(hall, player) {
            Condition::Owner => {
                let own = format!("data/html/clanHallManager/chamberlain-{npc_id}.htm");
                if self.talk.html.as_ref().is_some_and(|cache| cache.contains(&own)) {
                    own
                } else {
                    "data/html/clanHallManager/chamberlain.htm".to_owned()
                }
            }
            Condition::OwnerFalse => "data/html/clanHallManager/chamberlain-of.htm".to_owned(),
            Condition::AllFalse => "data/html/clanHallManager/chamberlain-no.htm".to_owned(),
        };
        if let Some(mut html) = self.html_from_file(player, npc.0, &path) {
            rep(&mut html, "%objectId%", npc.0);
            rep(&mut html, "%npcId%", npc_id);
            self.send_html(player, &html);
        }
    }

    // ------------------------------------------------------------------ bypass

    /// Java `ClanHallManager.onBypassFeedback(player, command)`.
    pub(crate) fn clan_hall_manager_bypass_feedback(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        command: &str,
    ) {
        // `getClanHall()` is null: the first line throws.
        let Some(hall_id) = self.manager_hall(npc) else {
            warn!("ClanHallManager: {npc} has no clan hall.");
            return;
        };
        // M7-CHSIEGE-01: `getClanHall().isSiegableHall() && ((SiegableHall) getClanHall()).isInSiege()`
        // returns here.
        let condition = self.manager_condition(Some(hall_id), player);
        if condition <= Condition::AllFalse {
            return;
        }
        let mut tokens = command.split(' ').filter(|t| !t.is_empty());
        let actual = tokens.next().unwrap_or("").to_lowercase();
        let val = tokens.next().unwrap_or("").to_owned();
        let rest: Vec<String> = tokens.map(str::to_owned).collect();
        let handled = match actual.as_str() {
            "banish_foreigner" => self.manager_banish(npc, player, hall_id, &val),
            "manage_vault" => self.manager_vault(npc, player, hall_id),
            "door" => self.manager_door(npc, player, hall_id, &val),
            "functions" => self.manager_functions(npc, player, hall_id, &val, &rest),
            "manage" => self.manager_manage(npc, player, hall_id, &val, &rest),
            "support" => self.manager_support(npc, player, hall_id, &val, &rest),
            "list_back" => {
                self.manager_list_back(npc, player);
                true
            }
            "support_back" => self.manager_support_back(npc, player, hall_id),
            "goto" => self.manager_goto(npc, player, hall_id, command),
            _ => false,
        };
        if !handled {
            // `super.onBypassFeedback(player, command)`.
            self.generic_npc_bypass_feedback(npc, player, command);
        }
    }

    fn manager_banish(&mut self, npc: ObjectId, player: ObjectId, hall: ClanHallId, val: &str) -> bool {
        let path = if self.has_clan_access(player, ClanAccess::HallBanish) {
            if val.eq_ignore_ascii_case("list") {
                Some("data/html/clanHallManager/banish-list.htm")
            } else if val.eq_ignore_ascii_case("banish") {
                self.residence_banish_foreigners(ResidenceRef::ClanHall(hall));
                Some("data/html/clanHallManager/banish.htm")
            } else {
                None
            }
        } else {
            Some("data/html/clanHallManager/not_authorized.htm")
        };
        // A page without a file is sent empty in Java.
        match path {
            Some(path) => self.manager_send_file(npc, player, path),
            None => {
                if let Some(html) = NpcHtmlMessage::new(npc.0, "") {
                    self.manager_send(npc, player, html);
                }
            }
        }
        true
    }

    fn manager_vault(&mut self, npc: ObjectId, player: ObjectId, hall: ClanHallId) -> bool {
        if !self.has_clan_access(player, ClanAccess::AccessWarehouse) {
            self.manager_send_file(npc, player, "data/html/clanHallManager/not_authorized.htm");
            return true;
        }
        let (lease, paid_until) = self
            .clan_hall(hall)
            .map(|h| (h.lease(), h.paid_until()))
            .unwrap_or_default();
        if lease <= 0 {
            self.manager_send_file(npc, player, "data/html/clanHallManager/vault-chs.htm");
        } else if let Some(mut html) =
            self.html_from_file(player, npc.0, "data/html/clanHallManager/vault.htm")
        {
            rep(&mut html, "%rent%", lease);
            rep(&mut html, "%date%", date_time(paid_until));
            self.manager_send(npc, player, html);
        }
        true
    }

    fn manager_door(&mut self, npc: ObjectId, player: ObjectId, hall: ClanHallId, val: &str) -> bool {
        if !self.has_clan_access(player, ClanAccess::HallOpenDoor) {
            self.manager_send_file(npc, player, "data/html/clanHallManager/not_authorized.htm");
            return true;
        }
        let path = if val.eq_ignore_ascii_case("open") {
            self.manager_open_close_doors(hall, true);
            "data/html/clanHallManager/door-open.htm"
        } else if val.eq_ignore_ascii_case("close") {
            self.manager_open_close_doors(hall, false);
            "data/html/clanHallManager/door-close.htm"
        } else {
            "data/html/clanHallManager/door.htm"
        };
        self.manager_send_file(npc, player, path);
        true
    }

    /// Java `ClanHall.openCloseDoors(open)` (without a player: every door of the hall).
    pub(crate) fn manager_open_close_doors(&mut self, hall: ClanHallId, open: bool) {
        for door in self.residence_doors(ResidenceRef::ClanHall(hall)) {
            if open {
                self.open_door(door);
            } else {
                self.close_door(door);
            }
        }
    }

    fn manager_list_back(&mut self, npc: ObjectId, player: ObjectId) {
        let (npc_id, name) = self
            .npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .map(|t| (t.id, t.name.clone()))
            .unwrap_or_default();
        let own = format!("data/html/clanHallManager/chamberlain-{npc_id}.htm");
        let path = if self.talk.html.as_ref().is_some_and(|cache| cache.contains(&own)) {
            own
        } else {
            "data/html/clanHallManager/chamberlain.htm".to_owned()
        };
        if let Some(mut html) = self.html_from_file(player, npc.0, &path) {
            rep(&mut html, "%objectId%", npc.0);
            rep(&mut html, "%npcname%", name);
            self.manager_send(npc, player, html);
        }
    }

    // ------------------------------------------------------------------ functions (for use)

    fn function_level(&self, hall: ClanHallId, function: i32) -> Option<i32> {
        self.residence_function(ResidenceRef::ClanHall(hall), function)
            .map(|f| f.lvl)
    }

    fn manager_functions(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        hall: ClanHallId,
        val: &str,
        rest: &[String],
    ) -> bool {
        let not_active = "data/html/clanHallManager/chamberlain-nac.htm";
        if val.eq_ignore_ascii_case("tele") {
            match self.function_level(hall, HALL_FUNC_TELEPORT) {
                None => self.manager_send_file(npc, player, not_active),
                Some(level) => {
                    let location = self
                        .clan_hall(hall)
                        .map(|h| h.location.clone())
                        .unwrap_or_default();
                    self.manager_send_file(
                        npc,
                        player,
                        &format!("data/html/clanHallManager/tele{location}{level}.htm"),
                    );
                }
            }
        } else if val.eq_ignore_ascii_case("item_creation") {
            let Some(level) = self.function_level(hall, HALL_FUNC_ITEM_CREATE) else {
                self.manager_send_file(npc, player, not_active);
                return true;
            };
            let Some(token) = rest.first() else {
                return true;
            };
            let Ok(list) = token.parse::<i32>() else {
                warn!("ClanHallManager: bad buy list {token} from {player}");
                return true;
            };
            self.show_buy_window(npc, player, list.wrapping_add(level.wrapping_mul(100_000)));
        } else if val.eq_ignore_ascii_case("support") {
            match self.function_level(hall, HALL_FUNC_SUPPORT) {
                None => self.manager_send_file(npc, player, not_active),
                Some(level) => {
                    let path = format!("data/html/clanHallManager/support{level}.htm");
                    if let Some(mut html) = self.html_from_file(player, npc.0, &path) {
                        rep(&mut html, "%mp%", self.manager_mp(npc));
                        self.manager_send(npc, player, html);
                    }
                }
            }
        } else if val.eq_ignore_ascii_case("back") {
            self.clan_hall_manager_show_chat_window(npc, player);
        } else if let Some(mut html) =
            self.html_from_file(player, npc.0, "data/html/clanHallManager/functions.htm")
        {
            let level_or_zero =
                |world: &Self, function: i32| world.function_level(hall, function).unwrap_or(0);
            rep(
                &mut html,
                "%xp_regen%",
                level_or_zero(self, HALL_FUNC_RESTORE_EXP),
            );
            rep(&mut html, "%hp_regen%", level_or_zero(self, HALL_FUNC_RESTORE_HP));
            rep(&mut html, "%mp_regen%", level_or_zero(self, HALL_FUNC_RESTORE_MP));
            self.manager_send(npc, player, html);
        }
        true
    }

    /// The NPC's current MP as an int (`(int) getCurrentMp()`).
    fn manager_mp(&self, npc: ObjectId) -> i32 {
        self.core_of(npc).map_or(0, |c| c.cur_mp as i32)
    }

    fn manager_support(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        hall: ClanHallId,
        val: &str,
        rest: &[String],
    ) -> bool {
        // FOLLOW-UP(M8): `player.isCursedWeaponEquipped()` ("The wielder of a cursed weapon cannot
        // receive outside heals or buffs"); cursed weapons are not ported.
        self.set_npc_target(npc, Some(player));
        if val.is_empty() {
            return true;
        }
        let invalid = "Invalid skill level, contact your admin!";
        let Ok(skill_id) = val.parse::<i32>() else {
            self.send_message(player, invalid);
            return true;
        };
        let skill_level = match rest.first() {
            Some(token) => match token.parse::<i32>() {
                Ok(level) => level,
                Err(_) => {
                    self.send_message(player, invalid);
                    return true;
                }
            },
            None => 0,
        };
        let key = SkillKey::new(skill_id, skill_level);
        let Some(table) = self.skill_table() else {
            self.send_message(player, invalid);
            return true;
        };
        let Some(skill) = table.get(skill_id, skill_level) else {
            // `skill.hasEffectType` on null: the NPE is caught.
            self.send_message(player, invalid);
            return true;
        };
        if self.skill_has_effect_type(skill, &[EffectType::Summon]) {
            self.do_simultaneous_cast(player, key);
        } else {
            let mp_cost = mp_cost_of(skill);
            let current_mp = self.core_of(npc).map_or(0.0, |c| c.cur_mp);
            // If Clan Hall Buff are free or current MP is greater than MP cost, the skill should be casted.
            if current_mp >= f64::from(mp_cost) || self.clanhall.settings.buff_free {
                self.do_cast(npc, key);
            } else {
                if let Some(mut html) =
                    self.html_from_file(player, npc.0, "data/html/clanHallManager/support-no_mana.htm")
                {
                    rep(&mut html, "%mp%", self.manager_mp(npc));
                    self.manager_send(npc, player, html);
                }
                return true;
            }
        }
        match self.function_level(hall, HALL_FUNC_SUPPORT) {
            None | Some(0) => return true,
            Some(_) => {}
        }
        if let Some(mut html) =
            self.html_from_file(player, npc.0, "data/html/clanHallManager/support-done.htm")
        {
            rep(&mut html, "%mp%", self.manager_mp(npc));
            self.manager_send(npc, player, html);
        }
        true
    }

    fn manager_support_back(&mut self, npc: ObjectId, player: ObjectId, hall: ClanHallId) -> bool {
        // `getFunction(FUNC_SUPPORT).getLevel()` throws for a hall without the function.
        let Some(level) = self.function_level(hall, HALL_FUNC_SUPPORT) else {
            warn!("ClanHallManager: support_back without the support function.");
            return true;
        };
        if level == 0 {
            return true;
        }
        let path = format!("data/html/clanHallManager/support{level}.htm");
        if let Some(mut html) = self.html_from_file(player, npc.0, &path) {
            rep(&mut html, "%mp%", self.manager_mp(npc));
            self.manager_send(npc, player, html);
        }
        true
    }

    /// `goto <locId>`: the hall's mirror teleports the clan member.
    fn manager_goto(&mut self, npc: ObjectId, player: ObjectId, hall: ClanHallId, command: &str) -> bool {
        if let Some(level) = self.function_level(hall, HALL_FUNC_TELEPORT).filter(|l| *l > 0) {
            let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
            let data = self.talk.teleporters.clone();
            if let Some(holder) = data
                .as_ref()
                .and_then(|d| d.get_holder(npc_id, &format!("tel{level}")))
            {
                // `Integer.parseInt(command.split(" ")[1])`.
                let Some(location) = command.split(' ').nth(1).and_then(|t| t.parse::<i32>().ok()) else {
                    warn!("ClanHallManager: bad goto command {command:?} from {player}");
                    return true;
                };
                self.do_teleport(npc, player, holder, location);
                self.send(player, &ActionFailed);
            }
        }
        true
    }

    // ------------------------------------------------------------------ functions (to buy)

    /// Java `revalidateDeco(player)`: `AgitDecoInfo` of the hall the player's clan owns.
    fn manager_revalidate_deco(&mut self, player: ObjectId) {
        let Some(hall) = self
            .clan_of(player)
            .and_then(|c| self.auctionable_hall_by_owner(c.id))
        else {
            return;
        };
        if let Some(packet) = self.agit_deco_info(hall) {
            self.send(player, &packet);
        }
    }

    fn manager_manage(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        hall: ClanHallId,
        val: &str,
        rest: &[String],
    ) -> bool {
        if !self.has_clan_access(player, ClanAccess::HallManageFunctions) {
            if let Some(html) = self.html_from_file(player, 1, "data/html/clanHallManager/not_authorized.htm")
            {
                self.manager_send(npc, player, html);
            }
            return true;
        }
        let siegable = self.clan_hall(hall).is_some_and(|h| h.is_siegable_hall());
        let group = if val.eq_ignore_ascii_case("recovery") {
            Some(Group::Recovery)
        } else if val.eq_ignore_ascii_case("other") {
            Some(Group::Other)
        } else if val.eq_ignore_ascii_case("deco") && !siegable {
            Some(Group::Deco)
        } else {
            None
        };
        match group {
            Some(group) => self.manager_manage_group(npc, player, hall, group, rest),
            None if val.eq_ignore_ascii_case("back") => {
                self.clan_hall_manager_show_chat_window(npc, player);
            }
            None => {
                let path = if siegable {
                    "data/html/clanHallManager/manage_siegable.htm"
                } else {
                    "data/html/clanHallManager/manage.htm"
                };
                self.manager_send_file(npc, player, path);
            }
        }
        true
    }

    /// One of `manage recovery`, `manage other` and `manage deco`.
    fn manager_manage_group(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        hall: ClanHallId,
        group: Group,
        rest: &[String],
    ) {
        if let Some(sub) = rest.first() {
            if self.clan_hall(hall).is_none_or(|h| h.owner_id == 0) {
                self.send_message(
                    player,
                    "This clan hall has no owner, you cannot change the configuration.",
                );
                return;
            }
            let sub = sub.to_lowercase();
            for spec in SPECS.iter().filter(|s| s.group == group) {
                if sub == spec.cancel {
                    if let Some(mut html) =
                        self.html_from_file(player, npc.0, "data/html/clanHallManager/functions-cancel.htm")
                    {
                        rep(&mut html, "%apply%", format!("{} {} 0", group.word(), spec.word));
                        self.manager_send(npc, player, html);
                    }
                    return;
                }
                if sub == spec.edit {
                    // `st.nextToken()` throws without a level.
                    let Some(level) = rest.get(1).and_then(|t| t.parse::<i32>().ok()) else {
                        warn!("ClanHallManager: {sub} without a level from {player}");
                        return;
                    };
                    self.manager_edit_page(npc, player, spec, level);
                    return;
                }
                if sub == spec.word {
                    if let Some(token) = rest.get(1) {
                        self.manager_apply(npc, player, hall, spec, token);
                    }
                    return;
                }
            }
        }
        self.manager_overview(npc, player, hall, group);
    }

    /// `edit_<x> <level>`: the page that shows the cost of a level.
    fn manager_edit_page(&mut self, npc: ObjectId, player: ObjectId, spec: &Spec, level: i32) {
        let Some(mut html) =
            self.html_from_file(player, npc.0, "data/html/clanHallManager/functions-apply.htm")
        else {
            return;
        };
        let settings = &self.clanhall.settings;
        let (days, fees) = (spec.fees)(settings);
        let cost = fees[(spec.slot)(level).clamp(1, fees.len()) - 1];
        rep(&mut html, "%name%", spec.name);
        rep(
            &mut html,
            "%cost%",
            format!("{cost}</font>Adena /{days} Day</font>)"),
        );
        rep(
            &mut html,
            "%use%",
            spec.use_text.replace("{}", &level.to_string()),
        );
        rep(
            &mut html,
            "%apply%",
            format!("{} {} {level}", spec.group.word(), spec.word),
        );
        self.manager_send(npc, player, html);
    }

    /// `<x> <level>`: buys, changes or stops the function.
    fn manager_apply(&mut self, npc: ObjectId, player: ObjectId, hall: ClanHallId, spec: &Spec, token: &str) {
        let Some(mut html) = self.html_from_file(
            player,
            npc.0,
            "data/html/clanHallManager/functions-apply_confirmed.htm",
        ) else {
            return;
        };
        // `Integer.parseInt(val)` throws for anything else.
        let Ok(level) = token.parse::<i32>() else {
            warn!("ClanHallManager: bad level {token} from {player}");
            return;
        };
        if self.function_level(hall, spec.function) == Some(level) {
            if let Some(mut used) =
                self.html_from_file(player, npc.0, "data/html/clanHallManager/functions-used.htm")
            {
                rep(&mut used, "%val%", spec.level_text(token));
                self.manager_send(npc, player, used);
            }
            return;
        }
        let fee = if level == 0 {
            if let Some(cancelled) = self.html_from_file(
                player,
                npc.0,
                "data/html/clanHallManager/functions-cancel_confirmed.htm",
            ) {
                html = cancelled;
            }
            0
        } else {
            let (_, fees) = (spec.fees)(&self.clanhall.settings);
            fees[(spec.slot)(level).clamp(1, fees.len()) - 1]
        };
        let rate = (spec.ratio)(&self.clanhall.settings);
        let add_new = self.function_level(hall, spec.function).is_none();
        if self.residence_update_functions(
            player,
            ResidenceRef::ClanHall(hall),
            spec.function,
            level,
            fee,
            rate,
            add_new,
        ) {
            self.manager_revalidate_deco(player);
        } else if let Some(low) =
            self.html_from_file(player, npc.0, "data/html/clanHallManager/low_adena.htm")
        {
            html = low;
            // NOTE(java-quirk): the page goes out here and once more below.
            self.manager_send(npc, player, html.clone());
        }
        self.manager_send(npc, player, html);
    }

    /// The overview pages: `edit_recovery.htm`, `edit_other.htm` and `deco.htm`.
    fn manager_overview(&mut self, npc: ObjectId, player: ObjectId, hall: ClanHallId, group: Group) {
        let path = match group {
            Group::Recovery => "data/html/clanHallManager/edit_recovery.htm",
            Group::Other => "data/html/clanHallManager/edit_other.htm",
            Group::Deco => "data/html/clanHallManager/deco.htm",
        };
        let Some(mut html) = self.html_from_file(player, npc.0, path) else {
            return;
        };
        let grade = self.clan_hall(hall).map_or(0, |h| h.grade());
        for spec in SPECS.iter().filter(|s| s.group == group) {
            let current = self
                .residence_function(ResidenceRef::ClanHall(hall), spec.function)
                .cloned();
            let links = spec.edit_links(grade);
            let (days, _) = (spec.fees)(&self.clanhall.settings);
            match current {
                Some(function) => {
                    let stage = if spec.percent {
                        format!("{}%", function.lvl)
                    } else {
                        format!("Stage {}", function.lvl)
                    };
                    rep(
                        &mut html,
                        &format!("%{}%", spec.value_key),
                        format!(
                            "{stage}</font> (<font color=\"FFAABB\">{}</font>Adena /{days} Day)",
                            function.lease
                        ),
                    );
                    rep(
                        &mut html,
                        &format!("%{}%", spec.period_key),
                        format!(
                            "Withdraw the fee for the next time at {}",
                            date_time(function.end_time)
                        ),
                    );
                    if let Some(links) = links {
                        rep(
                            &mut html,
                            &format!("%{}%", spec.change_key),
                            format!("{}{links}", spec.deactivate_link()),
                        );
                    }
                }
                None => {
                    rep(&mut html, &format!("%{}%", spec.value_key), "none");
                    rep(&mut html, &format!("%{}%", spec.period_key), "none");
                    if let Some(links) = links {
                        rep(&mut html, &format!("%{}%", spec.change_key), links);
                    }
                }
            }
        }
        self.manager_send(npc, player, html);
    }
}

/// `skill.getMpConsume() + skill.getMpInitialConsume()`.
fn mp_cost_of(skill: &SkillTemplate) -> i32 {
    skill.mp_consume.wrapping_add(skill.mp_initial_consume)
}
