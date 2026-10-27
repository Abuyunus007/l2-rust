//! NPC dialogs: the html window a click opens (`Npc.showChatWindow`), which file it shows, and the
//! bookkeeping that decides which `bypass` and `link` commands the client may send back.
// Java: entity/actor/Npc.java (showChatWindow, getHtmlPath, insertObjectIdAndShowChatWindow,
//       showPkDenyChatWindow, canInteract)
// Java: network/serverpackets/{AbstractHtmlPacket,NpcHtmlMessage}.java (setFile, replace, runImpl)
// Java: util/HtmlUtil.java (buildHtmlActionCache, buildHtmlBypassCache, buildHtmlLinkCache)
// Java: entity/actor/Player.java (addHtmlAction, clearHtmlActions, validateHtmlAction)
// Java: cache/HtmCache.java (getHtm)
//
// # The html action cache
// Every `NpcHtmlMessage` that reaches a player rebuilds that player's cache for its scope: the
// `="bypass ..."` and `="link ..."` commands found in the page, plus the id of the NPC the page
// belongs to (`runImpl`). `RequestBypassToServer` and `RequestLinkHtml` only get through when the
// command is in the cache ([`PlayerTalk::validate`]).
//
// # What is left out, with the system that brings it
// - Seven Signs state (`SevenSigns`, `SevenSignsFestival`): the npc ids that depend on the seal
//   owners show the file Java picks when `StrictSevenSigns` is off, and `%festivalMins%` stays in
//   the page. Olympiad state: `isHero`, `getOlympiadBuffCount` read as false / 0.
// - Cursed weapons (`isCursedWeaponEquipped`), `_isBusy` NPCs (nothing sets it), quest dialogs and
//   `ON_NPC_FIRST_TALK` listeners (scripts, D-05).
// - `GeneralConfig.HTM_CACHE = false` (lazy): the cache is always preloaded, see `l2_data::HtmCache`.

use std::collections::HashSet;
use std::sync::Arc;

use l2_data::{HtmCache, TeleporterData};
use l2_protocol_game::server::ChatType;
use l2_protocol_game::server::{
    ActionFailed, CreatureSay, NpcHtmlMessage, SayBody, SenderField, StatusUpdate,
};
use l2_util::ObjectId;
use tracing::warn;

use crate::world::World;

/// Java `Npc.INTERACTION_DISTANCE`.
pub const INTERACTION_DISTANCE: i32 = 250;

/// Java `AbstractHtmlPacket.VAR_PARAM_START_CHAR`.
const VAR_PARAM_START_CHAR: char = '$';

/// Java `HtmlActionScope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlActionScope {
    NpcHtml,
    NpcItemHtml,
    NpcQuestHtml,
    TutorialHtml,
    CommBoardHtml,
}

impl HtmlActionScope {
    const COUNT: usize = 5;

    fn index(self) -> usize {
        self as usize
    }
}

/// The configuration values NPC dialogs and gatekeepers read (Java `PlayerConfig`, `NpcConfig`,
/// `GeneralConfig`). `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct TalkSettings {
    /// `PlayerConfig.ALT_GAME_KARMA_PLAYER_CAN_SHOP` (`AltKarmaPlayerCanShop`).
    pub karma_player_can_shop: bool,
    /// `PlayerConfig.ALT_GAME_KARMA_PLAYER_CAN_USE_GK` (`AltKarmaPlayerCanUseGK`).
    pub karma_player_can_use_gk: bool,
    /// `PlayerConfig.ALT_GAME_KARMA_PLAYER_CAN_USE_WAREHOUSE` (`AltKarmaPlayerCanUseWareHouse`).
    pub karma_player_can_use_warehouse: bool,
    /// `PlayerConfig.MAX_FREE_TELEPORT_LEVEL` (`MaxFreeTeleportLevel`).
    pub max_free_teleport_level: i32,
    /// `PlayerConfig.TELEPORT_WHILE_SIEGE_IN_PROGRESS` (`TeleportWhileSiegeInProgress`).
    pub teleport_while_siege_in_progress: bool,
    /// `GeneralConfig.GM_DEBUG_HTML_PATHS` (`GMDebugHtmlPaths`): GMs are told which file is shown.
    pub gm_debug_html_paths: bool,
    /// `NpcConfig.LIST_PET_RENT_NPC` (`ListPetRentNpc`).
    pub list_pet_rent_npc: HashSet<i32>,
    /// Test hook: the local wall clock (milliseconds counted like Unix time, but in the server's
    /// local zone; see `l2_util::time::local_now_millis`) the Monday/Tuesday teleport discount
    /// reads. `None` = the system clock.
    pub clock_override_unix_ms: Option<i64>,
}

impl Default for TalkSettings {
    fn default() -> Self {
        Self {
            karma_player_can_shop: true,
            karma_player_can_use_gk: false,
            karma_player_can_use_warehouse: true,
            max_free_teleport_level: 40,
            teleport_while_siege_in_progress: true,
            gm_debug_html_paths: true,
            list_pet_rent_npc: HashSet::from([30827]),
            clock_override_unix_ms: None,
        }
    }
}

/// What a player remembers about the dialogs it was shown (Java `Player._htmlActionCaches`,
/// `_htmlActionOriginObjectIds`, `_lastHtmlActionOriginObjId`, `_lastFolkNpc`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerTalk {
    actions: [Vec<String>; HtmlActionScope::COUNT],
    origin: [i32; HtmlActionScope::COUNT],
    last_origin: i32,
    /// Java `getLastFolkNPC()`: the NPC the player clicked last.
    pub last_folk_npc: Option<ObjectId>,
    /// Java `getLastQuestNpcObject()`: the NPC a script (`Script <name>`) last talked through;
    /// 0 for none.
    pub last_quest_npc_object: i32,
    /// Java `getLearningClass()`: the class whose skill tree the player was last shown by a
    /// trainer. `None` is the player's own class (Java's field starts as `getPlayerClass()`).
    pub learning_class: Option<l2_data::ClassId>,
}

impl PlayerTalk {
    /// Java `clearHtmlActions(scope)`.
    pub fn clear(&mut self, scope: HtmlActionScope) {
        self.actions[scope.index()].clear();
    }

    /// Java `addHtmlAction(scope, action)`.
    pub fn add(&mut self, scope: HtmlActionScope, action: String) {
        self.actions[scope.index()].push(action);
    }

    /// Java `setHtmlActionOriginObjectId(scope, npcObjId)`.
    pub fn set_origin(&mut self, scope: HtmlActionScope, npc_obj_id: i32) {
        self.origin[scope.index()] = npc_obj_id;
    }

    /// Java `getLastHtmlActionOriginId()`.
    pub fn last_origin(&self) -> i32 {
        self.last_origin
    }

    /// Java `validateHtmlAction(action)`: -1 when no page the player was shown contains the
    /// action, else the object id of the NPC the page belongs to (0 = not bound to an NPC).
    pub fn validate(&mut self, action: &str) -> i32 {
        for scope in 0..HtmlActionScope::COUNT {
            if validate_in(&self.actions[scope], action) {
                self.last_origin = self.origin[scope];
                return self.last_origin;
            }
        }
        -1
    }
}

/// Java `String.trim()`: removes everything up to and including the space.
fn java_trim(text: &str) -> &str {
    text.trim_matches(|c: char| c <= ' ')
}

/// Java `Player.validateHtmlAction(Iterable<String>, String)`.
fn validate_in(cached: &[String], action: &str) -> bool {
    for entry in cached {
        // NOTE(java-quirk): `charAt(length - 1)` of an empty entry throws in Java, which drops the
        // whole packet; an empty entry (`bypass ""` in a page) simply never matches here.
        let Some(last) = entry.chars().last() else {
            continue;
        };
        if last == VAR_PARAM_START_CHAR {
            let prefix = java_trim(&entry[..entry.len() - last.len_utf8()]);
            if action.starts_with(prefix) {
                return true;
            }
        } else if entry == action {
            return true;
        }
    }
    false
}

/// Java `HtmlUtil.buildHtmlBypassCache`.
fn build_bypass_cache(talk: &mut PlayerTalk, scope: HtmlActionScope, html: &str) {
    // ASCII lowercasing keeps every index valid for `html` as well.
    let lower = html.to_ascii_lowercase();
    let mut bypass_end = 0;
    let mut next = lower[bypass_end..].find("=\"bypass ");
    while let Some(bypass_start) = next {
        let start_end = bypass_start + 9;
        let Some(quote) = lower[start_end..].find('"') else {
            break;
        };
        bypass_end = start_end + quote;

        let h_param = lower[start_end..].find("-h ").map(|i| i + start_end);
        let mut bypass = match h_param {
            Some(h) if h < bypass_end => java_trim(&html[h + 3..bypass_end]),
            _ => java_trim(&html[start_end..bypass_end]),
        }
        .to_owned();

        if let Some(first_parameter) = bypass.find(VAR_PARAM_START_CHAR) {
            bypass.truncate(first_parameter + 1);
        }
        talk.add(scope, bypass);
        next = lower[bypass_end..].find("=\"bypass ").map(|i| i + bypass_end);
    }
}

/// Java `HtmlUtil.buildHtmlLinkCache`.
fn build_link_cache(talk: &mut PlayerTalk, scope: HtmlActionScope, html: &str) {
    let lower = html.to_ascii_lowercase();
    let mut link_end = 0;
    let mut next = lower[link_end..].find("=\"link ");
    while let Some(link_start) = next {
        let start_end = link_start + 7;
        let Some(quote) = lower[start_end..].find('"') else {
            break;
        };
        link_end = start_end + quote;
        let link = java_trim(&html[start_end..link_end]);
        // NOTE(java-quirk): for an empty or `..` link Java `continue`s without moving on, which
        // spins forever on the same link; here the link is skipped and the scan goes on.
        if link.is_empty() {
            warn!("Html link path is empty!");
        } else if link.contains("..") {
            warn!("Html link path is invalid: {link}");
        } else {
            talk.add(scope, format!("link {link}"));
        }
        next = lower[link_end..].find("=\"link ").map(|i| i + link_end);
    }
}

/// Java `HtmlUtil.buildHtmlActionCache`.
pub fn build_html_action_cache(talk: &mut PlayerTalk, scope: HtmlActionScope, npc_obj_id: i32, html: &str) {
    talk.set_origin(scope, npc_obj_id);
    build_bypass_cache(talk, scope, html);
    build_link_cache(talk, scope, html);
}

/// Everything the dialog code keeps in the world besides the players.
#[derive(Default)]
pub(crate) struct TalkWorld {
    pub html: Option<Arc<HtmCache>>,
    pub teleporters: Option<Arc<TeleporterData>>,
    /// Java `SkillLearnData`: the classes each trainer teaches.
    pub skill_learn: Option<Arc<l2_data::SkillLearnData>>,
    pub bypass_handlers: crate::bypass::BypassHandlers,
    /// Java `Npc._isBusy` and `_busyMessage` by NPC (set by scripts).
    pub busy: std::collections::HashMap<ObjectId, (bool, String)>,
}

/// Java: which `entity/actor/instance/*` classes are a `Merchant`.
pub(crate) fn is_merchant(npc_type: &str) -> bool {
    matches!(
        npc_type,
        "Merchant"
            | "Teleporter"
            | "Fisherman"
            | "FortLogistics"
            | "FortManager"
            | "ClanHallManager"
            | "PetManager"
    )
}

/// The folder a type's own `getHtmlPath` override points to; `None` for the types that use
/// `Npc.getHtmlPath` (`data/html/default/`, with a fallback file).
fn html_folder(npc_type: &str) -> Option<&'static str> {
    Some(match npc_type {
        "Adventurer" => "adventurer_guildsman",
        "Fisherman" => "fisherman",
        "FortLogistics" => "fortress",
        "Guard" | "QuestGuard" => "guard",
        "KrateisCubeManager" | "KrateisMatchManager" => "krateisCube",
        "Merchant" | "ClanHallManager" | "FortManager" => "merchant",
        "PetManager" => "petmanager",
        "Teleporter" | "DungeonGatekeeper" => "teleporter",
        "SchemeBuffer" => "mods/SchemeBuffer",
        "Trainer" => "trainer",
        "UCHelper" | "UCManager" => "undergroundColiseum",
        "VillageMaster"
        | "VillageMasterDElf"
        | "VillageMasterDwarf"
        | "VillageMasterFighter"
        | "VillageMasterKamael"
        | "VillageMasterMystic"
        | "VillageMasterOrc"
        | "VillageMasterPriest" => "villagemaster",
        "Warehouse" => "warehouse",
        _ => return None,
    })
}

impl World {
    /// Hands the world the html pages (Java `HtmCache`). Call before players enter.
    pub fn set_html_cache(&mut self, cache: Arc<HtmCache>) {
        self.talk.html = Some(cache);
    }

    /// Hands the world the gatekeeper lists (Java `TeleporterData`). Call before players enter.
    pub fn set_teleporter_data(&mut self, data: Arc<TeleporterData>) {
        self.talk.teleporters = Some(data);
    }

    /// Hands the world the trainer lists (Java `SkillLearnData`). Call before players enter.
    pub fn set_skill_learn_data(&mut self, data: Arc<l2_data::SkillLearnData>) {
        self.talk.skill_learn = Some(data);
    }

    /// Registers a bypass command handler (Java `BypassHandler.registerHandler`). The command is
    /// matched on its first word, case-insensitively. A later registration replaces an earlier
    /// one for the same word.
    pub fn register_bypass_handler(&mut self, command: &str, handler: crate::bypass::BypassHandler) {
        self.talk.bypass_handlers.register(command, handler);
    }

    /// Java `Player.sendPacket(NpcHtmlMessage)`: the packet's `runImpl` rebuilds the action cache
    /// of its scope, then the page goes out.
    pub fn send_html(&mut self, to: ObjectId, message: &NpcHtmlMessage) -> bool {
        let scope = if message.item_id == 0 {
            HtmlActionScope::NpcHtml
        } else {
            HtmlActionScope::NpcItemHtml
        };
        if let Some(player) = self.players.get_mut(&to) {
            player.talk.clear(scope);
            build_html_action_cache(&mut player.talk, scope, message.npc_obj_id, &message.html);
        }
        self.send(to, message)
    }

    /// An `NpcHtmlMessage` after `disableValidation()`: the old actions of the scope are cleared
    /// and nothing new is allowed.
    pub fn send_html_unvalidated(&mut self, to: ObjectId, message: &NpcHtmlMessage) -> bool {
        let scope = if message.item_id == 0 {
            HtmlActionScope::NpcHtml
        } else {
            HtmlActionScope::NpcItemHtml
        };
        if let Some(player) = self.players.get_mut(&to) {
            player.talk.clear(scope);
        }
        self.send(to, message)
    }

    /// Java `HtmCache.getHtm(player, path)`. A GM is told the path when `GMDebugHtmlPaths` is on.
    pub fn get_htm(&self, player: ObjectId, path: &str) -> Option<String> {
        // `Player.getHtmlPrefix()` is empty unless multi-language is on (not ported).
        let content = self
            .talk
            .html
            .as_ref()?
            .get_localized("", path)
            .map(str::to_owned);
        if self.settings.talk.gm_debug_html_paths && self.is_gm(player) {
            let shown = path.get(5..).unwrap_or(path);
            self.send(
                player,
                &CreatureSay {
                    sender_object_id: 0,
                    chat_type: ChatType::General,
                    sender: SenderField::Name("HTML".to_owned()),
                    message_id: -1,
                    body: SayBody::Text(shown.to_owned()),
                },
            );
        }
        content
    }

    /// Java `AbstractHtmlPacket.setFile(player, path)` on a new `NpcHtmlMessage(npcObjId)`: the
    /// page, or the "missing" page Java shows (and warns about) when the file is not loaded.
    pub fn html_from_file(&self, player: ObjectId, npc_obj_id: i32, path: &str) -> Option<NpcHtmlMessage> {
        match self.get_htm(player, path) {
            Some(content) => NpcHtmlMessage::new(npc_obj_id, &content),
            None => {
                warn!("NpcHtmlMessage: Missing html page {path}");
                NpcHtmlMessage::new(
                    npc_obj_id,
                    &format!("<html><body>My Text is missing:<br>{path}</body></html>"),
                )
            }
        }
    }

    /// Java `Npc.insertObjectIdAndShowChatWindow(player, content)`.
    pub(crate) fn insert_object_id_and_show_chat_window(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        content: &str,
    ) {
        let replaced = content.replace("%objectId%", &npc.0.to_string());
        if let Some(message) = NpcHtmlMessage::new(npc.0, &replaced) {
            self.send_html(player, &message);
        }
    }

    /// Java `Npc.canInteract(player)`.
    pub fn can_interact(&self, npc: ObjectId, player: ObjectId) -> bool {
        let (Some(p), Some(n)) = (self.players.get(&player), self.npcs.get(&npc)) else {
            return false;
        };
        // Casting and sitting are not modelled.
        if p.core.cur_hp < 0.5 || p.trade.store_type != crate::trade::PrivateStoreType::None {
            return false;
        }
        if crate::target::distance_3d((n.core.x, n.core.y, n.core.z), (p.core.x, p.core.y, p.core.z))
            >= f64::from(INTERACTION_DISTANCE)
        {
            return false;
        }
        let npc_instance = self.objects.get(&npc).map_or(0, |e| e.instance_id);
        let player_instance = self.objects.get(&player).map_or(0, |e| e.instance_id);
        npc_instance == player_instance
    }

    /// Java `Npc.getHtmlPath(npcId, value)`, with the overrides of the NPC's class.
    pub fn npc_html_path(&self, npc: ObjectId, value: i32) -> Option<String> {
        let n = self.npcs.get(&npc)?;
        let template = self.npc_world.template(n.npc_id)?;
        let pom = if value == 0 {
            n.npc_id.to_string()
        } else {
            format!("{}-{value}", n.npc_id)
        };
        // `FortLogistics.getHtmlPath`: `logistics.htm` / `logistics-<value>.htm`.
        if template.npc_type == "FortLogistics" {
            return Some(if value == 0 {
                "data/html/fortress/logistics.htm".to_owned()
            } else {
                format!("data/html/fortress/logistics-{value}.htm")
            });
        }
        if let Some(folder) = html_folder(&template.npc_type) {
            return Some(format!("data/html/{folder}/{pom}.htm"));
        }
        let path = format!("data/html/default/{pom}.htm");
        let exists = self.talk.html.as_ref().is_some_and(|cache| cache.contains(&path));
        Some(if exists {
            path
        } else {
            // "I have nothing to say to you."
            "data/html/npcdefault.htm".to_owned()
        })
    }

    /// Java `Npc.showPkDenyChatWindow(player, type)`.
    fn show_pk_deny_chat_window(&mut self, npc: ObjectId, npc_id: i32, player: ObjectId, kind: &str) -> bool {
        let path = format!("data/html/{kind}/{npc_id}-pk.htm");
        let Some(content) = self.get_htm(player, &path) else {
            return false;
        };
        self.insert_object_id_and_show_chat_window(npc, player, &content);
        self.send(player, &ActionFailed);
        true
    }

    /// Java `Npc.showChatWindow(player)`: the click on an NPC. Gatekeepers on castle ground have
    /// their own windows in Java; no castle is loaded, so none is on castle ground.
    pub fn show_chat_window_default(&mut self, npc: ObjectId, player: ObjectId) {
        // The fortress NPCs override `showChatWindow(player)` (M7-FORT-01, `fort/npc.rs`).
        if self.fort_show_chat_window(npc, player) {
            return;
        }
        // The clan hall NPCs override `showChatWindow(player)` (`clanhall/`).
        if self.clan_hall_npc_show_chat_window(npc, player) {
            return;
        }
        self.show_chat_window(npc, player, 0);
    }

    /// Java `Npc.showChatWindow(player, value)`.
    pub fn show_chat_window(&mut self, npc: ObjectId, player: ObjectId, value: i32) {
        let Some((npc_id, npc_type, talkable, name)) = self.npcs.get(&npc).and_then(|n| {
            let template = self.npc_world.template(n.npc_id)?;
            Some((
                n.npc_id,
                template.npc_type.clone(),
                template.talkable,
                template.name.clone(),
            ))
        }) else {
            return;
        };
        let Some(karma) = self.players.get(&player).map(|p| p.data.karma) else {
            return;
        };
        if !talkable {
            self.send(player, &ActionFailed);
            return;
        }

        // FOLLOW-UP(M5): `player.isCursedWeaponEquipped()` -> `setTarget(player)`; return.

        if karma > 0 {
            let settings = self.settings.talk.clone();
            // NOTE(java-quirk): a `Teleporter` is a `Merchant`, so with `AltKarmaPlayerCanShop`
            // off a karma player is shown the merchant page of a gatekeeper.
            if !settings.karma_player_can_shop && is_merchant(&npc_type) {
                if self.show_pk_deny_chat_window(npc, npc_id, player, "merchant") {
                    return;
                }
            } else if !settings.karma_player_can_use_gk && npc_type == "Teleporter" {
                if self.show_pk_deny_chat_window(npc, npc_id, player, "teleporter") {
                    return;
                }
            } else if !settings.karma_player_can_use_warehouse && npc_type == "Warehouse" {
                if self.show_pk_deny_chat_window(npc, npc_id, player, "warehouse") {
                    return;
                }
            } else if !settings.karma_player_can_shop
                && npc_type == "Fisherman"
                && self.show_pk_deny_chat_window(npc, npc_id, player, "fisherman")
            {
                return;
            }
        }

        if npc_type == "Auctioneer" && value == 0 {
            return;
        }

        let Some(filename) = self.chat_window_file(npc, npc_id, player, value) else {
            return;
        };

        // Send a Server->Client NpcHtmlMessage containing the text of the Npc to the Player.
        let Some(mut html) = self.html_from_file(player, npc.0, &filename) else {
            return;
        };
        if is_merchant(&npc_type) && self.settings.talk.list_pet_rent_npc.contains(&npc_id) {
            html.html = html
                .html
                .replace("_Quest", "_RentPet\">Rent Pet</a><br><a action=\"bypass Script");
        }
        html.html = html.html.replace("%npcname%", &name);
        html.html = html.html.replace("%objectId%", &npc.0.to_string());
        // FOLLOW-UP(Seven Signs): `%festivalMins%` is `SevenSignsFestival.getTimeToNextFestivalStr()`.
        self.send_html(player, &html);

        // ActionFailed, "to avoid that the client wait another packet".
        self.send(player, &ActionFailed);
    }

    /// The `switch (npcId)` of `Npc.showChatWindow`: which file the page comes from. `None` where
    /// Java returns without a window.
    fn chat_window_file(&self, npc: ObjectId, npc_id: i32, player: ObjectId, value: i32) -> Option<String> {
        const SEVEN_SIGNS: &str = "data/html/seven_signs/";
        const OLYMPIAD: &str = "data/html/olympiad/";
        let noble = self.players.get(&player).is_some_and(|p| p.data.profile.noble);
        Some(match npc_id {
            // Dawn Festival Guides
            31127..=31131 => format!("{SEVEN_SIGNS}festival/dawn_guide.htm"),
            // Dusk Festival Guides
            31137..=31141 => format!("{SEVEN_SIGNS}festival/dusk_guide.htm"),
            // Black Marketeer of Mammon
            31092 => format!("{SEVEN_SIGNS}blkmrkt_1.htm"),
            // Merchant of Mammon / Blacksmith of Mammon: the `StrictSevenSigns` checks need the
            // Seven Signs state (FOLLOW-UP(Seven Signs)); with the option off Java shows these.
            31113 => format!("{SEVEN_SIGNS}mammmerch_1.htm"),
            31126 => format!("{SEVEN_SIGNS}mammblack_1.htm"),
            // Festival Witches
            31132..=31136 | 31142..=31146 => format!("{SEVEN_SIGNS}festival/festival_witch.htm"),
            31688 => {
                if noble {
                    format!("{OLYMPIAD}noble_main.htm")
                } else {
                    self.npc_html_path(npc, value)?
                }
            }
            31690 | 31769..=31772 => {
                // FOLLOW-UP(M5): `player.isHero()`.
                if noble {
                    format!("{OLYMPIAD}hero_main.htm")
                } else {
                    self.npc_html_path(npc, value)?
                }
            }
            // FOLLOW-UP(Olympiad): `getOlympiadBuffCount()` is always 0 here.
            36402 => format!("{OLYMPIAD}olympiad_nobuffs.htm"),
            30298 => {
                // Blacksmith Pinter: `isAcademyMember()` needs clans (M5).
                self.npc_html_path(npc, value)?
            }
            31865..=31918 => {
                if value == 0 {
                    format!("{SEVEN_SIGNS}rift/GuardianOfBorder.htm")
                } else {
                    format!("{SEVEN_SIGNS}rift/GuardianOfBorder-{value}.htm")
                }
            }
            31093..=31094 | 31172..=31201 | 31239..=31254 => return None,
            _ => self.npc_html_path(npc, value)?,
        })
    }

    /// Replaces `%objectId%` and `%npcname%` of a page, as the gatekeeper's `sendHtmlMessage` does.
    pub(crate) fn send_npc_file(&mut self, npc: ObjectId, player: ObjectId, filename: &str) {
        let name = self
            .npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .map(|t| t.name.clone())
            .unwrap_or_default();
        let Some(mut html) = self.html_from_file(player, npc.0, filename) else {
            return;
        };
        html.html = html.html.replace("%objectId%", &npc.0.to_string());
        html.html = html.html.replace("%npcname%", &name);
        self.send_html(player, &html);
    }

    /// Debug aid used by the tests: how many bypasses the player may send for `scope`.
    #[doc(hidden)]
    pub fn html_action_count(&self, player: ObjectId, scope: HtmlActionScope) -> usize {
        self.players
            .get(&player)
            .map_or(0, |p| p.talk.actions[scope.index()].len())
    }

    /// Sends a `StatusUpdate` with the target's hit points (the part of `Player.setTarget` that
    /// the client's target window needs).
    pub(crate) fn send_target_hp(&self, to: ObjectId, target: ObjectId, max_hp: i32, cur_hp: i32) {
        let mut update = StatusUpdate::new(target.0);
        update.add_attribute(l2_protocol_game::server::visibility::status::MAX_HP, max_hp);
        update.add_attribute(l2_protocol_game::server::visibility::status::CUR_HP, cur_hp);
        self.send(to, &update);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache(html: &str) -> PlayerTalk {
        let mut talk = PlayerTalk::default();
        build_html_action_cache(&mut talk, HtmlActionScope::NpcHtml, 77, html);
        talk
    }

    #[test]
    fn bypass_cache_takes_the_text_after_h() {
        let mut talk =
            cache(r#"<a action="bypass -h npc_1_Chat 3">x</a><a action="bypass npc_1_showTeleports">y</a>"#);
        assert_eq!(talk.validate("npc_1_Chat 3"), 77);
        assert_eq!(talk.validate("npc_1_showTeleports"), 77);
        assert_eq!(talk.validate("npc_1_Chat 4"), -1);
        assert_eq!(talk.validate("npc_1_Chat"), -1);
    }

    #[test]
    fn a_dollar_entry_matches_every_command_that_starts_the_same() {
        let mut talk = cache(r#"<edit var="a"><a action="bypass -h npc_1_Quest $a $b">x</a>"#);
        assert_eq!(talk.validate("npc_1_Quest hello world"), 77);
        assert_eq!(talk.validate("npc_1_Other"), -1);
    }

    #[test]
    fn link_cache_keeps_a_lower_case_link_literal() {
        let mut talk = cache(r#"<a action="link teleporter/separatedsoul.htm">x</a>"#);
        assert_eq!(talk.validate("link teleporter/separatedsoul.htm"), 77);
        assert_eq!(talk.validate("link teleporter/other.htm"), -1);
    }

    #[test]
    fn a_bad_link_does_not_hang_the_scan() {
        // Java loops forever on this page; the scan must go on to the next link.
        let mut talk =
            cache(r#"<a action="link ../x.htm">a</a><a action="link ">b</a><a action="link ok.htm">c</a>"#);
        assert_eq!(talk.validate("link ok.htm"), 77);
        assert_eq!(talk.validate("link ../x.htm"), -1);
    }

    #[test]
    fn matching_is_case_insensitive_for_the_markers_only() {
        let mut talk = cache(r#"<A ACTION="BYPASS -h npc_1_Chat 1">x</A>"#);
        // `-h ` is searched in the lower-cased page, the text is taken from the original.
        assert_eq!(talk.validate("npc_1_Chat 1"), 77);
    }

    #[test]
    fn an_empty_entry_never_matches() {
        let mut talk = cache(r#"<a action="bypass ">x</a>"#);
        assert_eq!(talk.validate(""), -1);
        assert_eq!(talk.validate("anything"), -1);
    }

    #[test]
    fn the_first_scope_that_knows_the_action_decides_the_origin() {
        let mut talk = PlayerTalk::default();
        build_html_action_cache(
            &mut talk,
            HtmlActionScope::NpcItemHtml,
            5,
            r#"<a action="bypass item_1_x">"#,
        );
        build_html_action_cache(
            &mut talk,
            HtmlActionScope::NpcHtml,
            9,
            r#"<a action="bypass item_1_x">"#,
        );
        assert_eq!(talk.validate("item_1_x"), 9);
        assert_eq!(talk.last_origin(), 9);
    }
}
