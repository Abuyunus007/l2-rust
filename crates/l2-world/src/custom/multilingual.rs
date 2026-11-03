//! `config/Custom/MultilingualSupport.ini`: the player's language, the `.lang` voiced command, the
//! html pages in the language of the player and the translated texts of `data/lang/<lang>/`.
// Java: dist/game/data/scripts/handlers/chat/commands/voiced/Lang.java
// Java: entity/actor/Player.java (getLang, setLang, getHtmlPrefix, sendMessage, sendDamageMessage)
// Java: data/xml/{NpcNameLocalisationData,SendMessageLocalisationData}.java
// Java: network/{SystemMessageId,NpcStringId}.java (loadLocalisations, getLocalisation)
// Java: mechanics/clientstrings/{Builder,BuilderContainer,BuilderObject,BuilderText}.java
// Java: network/serverpackets/{SystemMessage,NpcSay,ExShowScreenMessage,AbstractNpcInfo,
// Java: CharSelectionInfo}.java, entity/actor/status/PlayerStatus.java
//
// # Where the language is
// `Player._lang` is `PlayerProfile::language` (`characters.language`): read at the login by the
// session, normalised by [`World::set_player_lang`] when the player enters the world, written back
// with every store like any other column (Java `UPDATE_CHARACTER` writes `_lang`; it is NULL
// again with the switch off). The `CharSelectionInfo` part (the html prefix of the list entry) is
// `game-server/src/client/charlist.rs`.
//
// # How a text is localised
// Java does it in `writeImpl`, with the player of the client. The world encodes a packet once and
// sends the bytes ([`World::send_bytes`]), also to many players at a time, so the same step runs
// there: the bytes of a `SystemMessage`, `NpcSay` or `ExShowScreenMessage` are read back, the text
// of the player's language is put in and the packet is written again. `NpcInfo` is built per viewer
// (`World::npc_info`), and `Player.sendMessage` and the two damage messages ask this module for
// the text. With `MultiLangEnable = False` (or no language folder loaded) the only cost is one
// `bool` test per sent packet.

use std::collections::HashMap;

use l2_data::LangData;
use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::server::{
    DeleteObject, ExShowScreenMessage, NpcSay, SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{NpcStringId, ServerOpcode, SystemMessageId};
use l2_util::ObjectId;
use tracing::warn;

use super::misc::{CustomMiscTimer, tokens};
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// One piece of a [`Template`].
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    /// `BuilderText`.
    Text(String),
    /// `BuilderObject`: the 0-based index of `$s1` .. `$s9`.
    Param(usize),
}

/// Java `clientstrings.Builder`: a translation with its `$s1`, `$c1`, `$p1` (and upper case)
/// markers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    parts: Vec<Part>,
}

impl Template {
    /// `Builder.newBuilder(text)`. `None` is Java's `RuntimeException("Illegal Id: 0")` of a
    /// `$s0` marker.
    pub fn parse(text: &str) -> Option<Self> {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut parts = Vec::new();
        let (mut index1, mut index2) = (0_usize, 0_usize);
        while index1 < len {
            if chars[index1] == '$'
                && index1 + 2 < len
                && matches!(chars[index1 + 1], 'c' | 's' | 'p' | 'C' | 'S' | 'P')
                && let Some(digit) = chars[index1 + 2].to_digit(10)
            {
                if index1 > index2 {
                    parts.push(Part::Text(chars[index2..index1].iter().collect()));
                }
                // `BuilderObject`: ids 1 to 9 only.
                if digit == 0 {
                    return None;
                }
                parts.push(Part::Param(digit as usize - 1));
                index1 += 2;
                index2 = index1 + 1;
            }
            index1 += 1;
        }
        if index1 > index2 {
            parts.push(Part::Text(chars[index2..index1].iter().collect()));
        }
        Some(Self { parts })
    }

    /// `Builder.toString(Object... params)`.
    pub fn render(&self, params: &[String]) -> String {
        // A text with exactly one piece is that piece itself, not a container.
        if let [only] = self.parts.as_slice() {
            return match only {
                Part::Text(text) => text.clone(),
                // NOTE(java-quirk): `BuilderObject.toString(Object...)` takes the first parameter
                // whatever its own index is.
                Part::Param(_) => params.first().cloned().unwrap_or_else(|| "null".to_owned()),
            };
        }
        let mut out = String::new();
        for part in &self.parts {
            match part {
                Part::Text(text) => out.push_str(text),
                Part::Param(index) => match params.get(*index) {
                    Some(value) => out.push_str(value),
                    // `BuilderObject.toString()`.
                    None => out.push_str(&format!("[PARAM-{}]", index + 1)),
                },
            }
        }
        out
    }
}

/// One `SendMessageLocalisation.xml` entry: the parts of the message and of its translation.
type SendMessagePair = (Vec<String>, Vec<String>);

/// Everything read from `data/lang/`, ready to use. Empty (and `active` false) with the switch
/// off.
#[derive(Debug, Default)]
pub struct LangTexts {
    /// Some language has some text: the packets are looked at only then.
    active: bool,
    /// `SystemMessageId._localisations`: language -> `SystemMessageId` id -> text.
    system_messages: HashMap<String, HashMap<i32, Template>>,
    /// `NpcStringId._localisations`.
    npc_strings: HashMap<String, HashMap<i32, Template>>,
    /// `NpcNameLocalisationData`: language -> NPC id -> (name, title).
    npc_names: HashMap<String, HashMap<i32, (String, String)>>,
    /// `SendMessageLocalisationData`: language -> (message parts, translation parts).
    send_messages: HashMap<String, Vec<SendMessagePair>>,
}

impl LangTexts {
    /// Builds the tables from the files. Ids the client tables do not know are skipped, and a
    /// `$s0` marker ends the rest of its file (Java's catch is around the whole file).
    pub fn build(data: &LangData) -> Self {
        let mut texts = Self::default();
        for (lang, tables) in &data.languages {
            let mut system = HashMap::new();
            for (id, text) in &tables.system_messages {
                if SystemMessageId::from_id(*id).is_none() {
                    warn!("SystemMessageId: Unknown SMID '{id}', lang '{lang}'.");
                    continue;
                }
                let Some(template) = Template::parse(text) else {
                    warn!("SystemMessageId: Failed loading the localisations of '{lang}'.");
                    break;
                };
                system.insert(*id, template);
            }
            let mut strings = HashMap::new();
            for (id, text) in &tables.npc_strings {
                if NpcStringId::from_id(*id).is_none() {
                    warn!("NpcStringId: Unknown NSID '{id}', lang '{lang}'.");
                    continue;
                }
                let Some(template) = Template::parse(text) else {
                    warn!("NpcStringId: Failed loading the localisations of '{lang}'.");
                    break;
                };
                strings.insert(*id, template);
            }
            texts.system_messages.insert(lang.clone(), system);
            texts.npc_strings.insert(lang.clone(), strings);
            if !tables.npc_names.is_empty() {
                texts.npc_names.insert(lang.clone(), tables.npc_names.clone());
            }
            if !tables.send_messages.is_empty() {
                texts
                    .send_messages
                    .insert(lang.clone(), tables.send_messages.clone());
            }
            texts.active = true;
        }
        texts
    }

    /// `NpcNameLocalisationData.getLocalisation(lang, id)`.
    pub fn npc_name(&self, lang: &str, npc_id: i32) -> Option<&(String, String)> {
        self.npc_names.get(lang)?.get(&npc_id)
    }

    /// `NpcNameLocalisationData.hasLocalisation(id)`: in any language.
    pub fn has_npc_name(&self, npc_id: i32) -> bool {
        self.npc_names.values().any(|names| names.contains_key(&npc_id))
    }

    /// `SendMessageLocalisationData.getLocalisation(player, message)` for a language.
    pub fn send_message(&self, lang: &str, message: &str) -> String {
        let Some(list) = self.send_messages.get(lang) else {
            return message.to_owned();
        };
        let mut localisation = message.to_owned();
        // NOTE(java-quirk): Java keeps the entries in a `ConcurrentHashMap` keyed by array
        // identity, so the order they are tried in is arbitrary; they are tried in file order.
        for (search, replacement) in list {
            if let [only] = search.as_slice() {
                // Exact match.
                if *only == localisation {
                    return replacement.first().cloned().unwrap_or(localisation);
                }
            } else if search.iter().all(|part| localisation.contains(part.as_str())) {
                // Split match: every part is replaced by its own translation.
                for (i, part) in search.iter().enumerate() {
                    // NOTE(java-quirk): Java throws `ArrayIndexOutOfBoundsException` when the
                    // translation has fewer parts; here the replacing stops.
                    let Some(with) = replacement.get(i) else { break };
                    localisation = localisation.replace(part.as_str(), with);
                }
                break;
            }
        }
        localisation
    }
}

/// `SystemMessage.SMParam.getValue().toString()`: what a parameter reads like inside a
/// translation.
fn param_text(param: &SystemMessageParam) -> String {
    match param {
        SystemMessageParam::Text(text) | SystemMessageParam::PlayerName(text) => text.clone(),
        SystemMessageParam::CastleName(v)
        | SystemMessageParam::DoorName(v)
        | SystemMessageParam::ItemName(v)
        | SystemMessageParam::ElementName(v)
        | SystemMessageParam::SystemString(v)
        | SystemMessageParam::InstanceName(v)
        | SystemMessageParam::IntNumber(v) => v.to_string(),
        // `addNpcName(int)` stores `1000000 + id`.
        SystemMessageParam::NpcName(id) => id.wrapping_add(1_000_000).to_string(),
        SystemMessageParam::LongNumber(v) => v.to_string(),
        // NOTE(java-quirk): these values are `int[]`; Java prints the array identity
        // (`[I@1b6d3586`), which cannot be reproduced.
        SystemMessageParam::SkillName { .. } | SystemMessageParam::ZoneName { .. } => "[I@0".to_owned(),
    }
}

/// `List<String>.toString()`: how `NpcSay` and `ExShowScreenMessage` hand their parameters to a
/// translation.
// NOTE(java-quirk): `nsl.getLocalisation(_parameters)` passes the whole list as ONE parameter, so a
// `$s1` reads `[a, b]` and a `$s2` reads `[PARAM-2]`.
fn list_params(params: &[String]) -> Vec<String> {
    vec![format!("[{}]", params.join(", "))]
}

const SYSTEM_MESSAGE: u8 = ServerOpcode::SystemMessage.id();
const NPC_SAY: u8 = ServerOpcode::NpcSay.id();
const EX_SHOW_SCREEN_MESSAGE: u8 = ServerOpcode::ExShowScreenMessage.id();
/// The second opcode of `ExShowScreenMessage` (`write_opcode` writes it as a `u16`).
const EX_SHOW_SCREEN_MESSAGE_EX: u16 = match ServerOpcode::ExShowScreenMessage.ex_id() {
    Some(ex) => ex,
    None => 0,
};

impl World {
    /// Hands the world the translated texts (`SystemMessageId.loadLocalisations`,
    /// `NpcStringId.loadLocalisations`, `NpcNameLocalisationData`, `SendMessageLocalisationData`).
    /// The caller passes no language with `MultiLangEnable = False`.
    pub fn set_lang_texts(&mut self, data: &LangData) {
        self.custom_misc.texts = LangTexts::build(data);
    }

    /// True when a packet can need a translation: the switch is on and a language is loaded.
    pub(crate) fn multilang_on(&self) -> bool {
        self.custom_misc.texts.active && self.settings.custom_misc.multilang.enable
    }

    /// Java `Player.getLang()`: `None` until the player chose (Java: null).
    pub fn player_lang(&self, id: ObjectId) -> Option<&str> {
        self.players.get(&id)?.data.profile.language.as_deref()
    }

    /// The language of the client a packet for `to` goes to (a summon's packets go to its owner),
    /// when it is one other than `en`.
    fn foreign_lang_of(&self, to: ObjectId) -> Option<&str> {
        let player = match self.players.get(&to) {
            Some(_) => to,
            None => self.summons.summons.get(&to)?.owner,
        };
        self.player_lang(player).filter(|lang| *lang != "en")
    }

    /// Java `Player.getHtmlPrefix()`: empty unless `MultiLangEnable`, and for English.
    pub(crate) fn html_prefix(&self, id: ObjectId) -> String {
        if !self.settings.custom_misc.multilang.enable {
            return String::new();
        }
        match self.player_lang(id) {
            Some(lang) if lang != "en" => format!("data/lang/{lang}/"),
            _ => String::new(),
        }
    }

    /// Java `Player.setLang(lang)`: `true` when the language is allowed. A language that is not
    /// allowed sets the default one; with the switch off the language is null.
    pub(crate) fn set_player_lang(&mut self, id: ObjectId, lang: Option<&str>) -> bool {
        let m = &self.settings.custom_misc.multilang;
        let (chosen, result) = if !m.enable {
            (None, false)
        } else if let Some(lang) = lang.filter(|l| m.allowed.iter().any(|a| a == l)) {
            (Some(lang.to_owned()), true)
        } else {
            (Some(m.default_lang.clone()), false)
        };
        if let Some(player) = self.players.get_mut(&id) {
            player.data.profile.language = chosen;
        }
        result
    }

    /// Java `Player.restore`: `player.setLang(rset.getString("language"))`.
    pub(crate) fn restore_player_lang(&mut self, id: ObjectId) {
        let raw = self
            .players
            .get(&id)
            .and_then(|p| p.data.profile.language.clone());
        self.set_player_lang(id, raw.as_deref());
    }

    /// The name and title of an NPC in the language of `player`, when `NpcNameLocalisation.xml` has
    /// them (`NpcNameLocalisationData.getLocalisation(player.getLang(), id)`).
    pub(crate) fn npc_name_localisation(&self, player: ObjectId, npc_id: i32) -> Option<(String, String)> {
        if !self.multilang_on() {
            return None;
        }
        let lang = self.player_lang(player)?;
        self.custom_misc.texts.npc_name(lang, npc_id).cloned()
    }

    /// [`Self::npc_name_localisation`] for `AbstractNpcInfo.NpcInfo`, which leaves English alone.
    pub(crate) fn npc_name_localisation_for_info(
        &self,
        viewer: ObjectId,
        npc_id: i32,
    ) -> Option<(String, String)> {
        if !self.multilang_on() || self.player_lang(viewer) == Some("en") {
            return None;
        }
        self.npc_name_localisation(viewer, npc_id)
    }

    /// The name of `creature` in a damage message for `player`: an NPC shows its translated name
    /// (`Player.sendDamageMessage`, `PlayerStatus.reduceHp`).
    pub(crate) fn creature_name_for(&self, player: ObjectId, creature: ObjectId) -> String {
        let name = self.creature_name(creature);
        if self.multilang_on()
            && self.is_npc(creature)
            && let Some(npc) = self.npcs.get(&creature)
            && let Some((translated, _)) = self.npc_name_localisation(player, npc.npc_id)
        {
            return translated;
        }
        name
    }

    /// Java `SendMessageLocalisationData.getLocalisation(player, message)`.
    pub(crate) fn send_message_text(&self, to: ObjectId, message: &str) -> String {
        if self.multilang_on()
            && let Some(lang) = self.player_lang(to)
        {
            return self.custom_misc.texts.send_message(lang, message);
        }
        message.to_owned()
    }

    /// The `writeImpl` step of `SystemMessage`, `NpcSay` and `ExShowScreenMessage`: the bytes the
    /// player gets, in the player's language when there is a translation. Called by
    /// [`World::send_bytes`] for every packet while [`World::multilang_on`].
    pub(crate) fn localise_bytes(&self, to: ObjectId, bytes: Vec<u8>) -> Vec<u8> {
        if !self.multilang_on() {
            return bytes;
        }
        // The cheap test first: only these three packets carry the texts.
        let wanted = match bytes.first() {
            Some(&SYSTEM_MESSAGE | &NPC_SAY) => true,
            Some(&EX_SHOW_SCREEN_MESSAGE) => {
                bytes.get(1..3) == Some(&EX_SHOW_SCREEN_MESSAGE_EX.to_le_bytes()[..])
            }
            _ => false,
        };
        if !wanted {
            return bytes;
        }
        let Some(lang) = self.foreign_lang_of(to) else {
            return bytes;
        };
        self.localise_packet(lang, &bytes).unwrap_or(bytes)
    }

    /// `None`: nothing to translate, send the packet as it is.
    fn localise_packet(&self, lang: &str, bytes: &[u8]) -> Option<Vec<u8>> {
        let texts = &self.custom_misc.texts;
        match *bytes.first()? {
            SYSTEM_MESSAGE => {
                let message = SystemMessage::decode_added(&mut PacketReader::new(&bytes[1..])).ok()?;
                let template = texts.system_messages.get(lang)?.get(&message.id.id())?;
                let params: Vec<String> = message.params.iter().map(param_text).collect();
                // `S1_2` with one text parameter.
                Some(SystemMessage::text(&template.render(&params)).to_bytes())
            }
            NPC_SAY => {
                let mut say = NpcSay::decode(&mut PacketReader::new(&bytes[1..])).ok()?;
                let template = texts.npc_strings.get(lang)?.get(&say.npc_string)?;
                say.text = template.render(&list_params(&say.parameters));
                say.npc_string = -1;
                say.parameters.clear();
                Some(say.to_bytes())
            }
            _ => {
                let mut screen = ExShowScreenMessage::decode(&mut PacketReader::new(&bytes[3..])).ok()?;
                // `if (_sysMessageId > -1) {..} else if (_npcString > -1) {..}`.
                let template = if screen.sys_message_id > -1 {
                    texts.system_messages.get(lang)?.get(&screen.sys_message_id)?
                } else if screen.npc_string_id > -1 {
                    texts.npc_strings.get(lang)?.get(&screen.npc_string_id)?
                } else {
                    return None;
                };
                screen.text = template.render(&list_params(&screen.params));
                screen.sys_message_id = -1;
                screen.npc_string_id = -1;
                screen.params.clear();
                Some(screen.to_bytes())
            }
        }
    }

    /// `Lang.onCommand`.
    pub(crate) fn lang_voiced(&mut self, player: ObjectId, params: Option<&str>) -> bool {
        let m = &self.settings.custom_misc.multilang;
        if !m.enable || !m.voiced_allow {
            return false;
        }
        let Some(params) = params else {
            let mut list = String::new();
            for lang in &m.allowed {
                list.push_str(&format!(
                    "<button value=\"{}\" action=\"bypass -h voice .lang {lang}\" width=60 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"><br>",
                    lang.to_uppercase()
                ));
            }
            self.lang_page(player, "data/html/mods/Lang/LanguageSelect.htm", Some(&list));
            return true;
        };

        let Some(first) = tokens(params).into_iter().next() else {
            return false;
        };
        // `st.nextToken().trim()`: a token has no whitespace, `trim` also drops control chars.
        let lang = first.trim_matches(|c: char| c <= ' ');
        if self.set_player_lang(player, Some(lang)) {
            self.lang_page(player, "data/html/mods/Lang/Ok.htm", None);
            self.lang_resend_npcs(player);
            self.set_target(player, None);
            return true;
        }
        self.lang_page(player, "data/html/mods/Lang/Error.htm", None);
        true
    }

    /// `msg.setFile(activeChar, path)` (+ `replace("%list%", ..)`), sent to the player.
    fn lang_page(&mut self, player: ObjectId, path: &str, list: Option<&str>) {
        let html = self
            .get_htm(player, path)
            .unwrap_or_else(|| format!("<html><body>My Text is missing:<br>{path}</body></html>"));
        let html = match list {
            Some(list) => html.replace("%list%", list),
            None => html,
        };
        if let Some(message) = l2_protocol_game::server::NpcHtmlMessage::new(0, &html) {
            self.send_html(player, &message);
        }
    }

    /// The loop of `Lang.onCommand`: every NPC of the world that has a name translation is deleted
    /// from the client and shown again a second later, so the client takes the new name.
    // NOTE(java-quirk): Java walks `World.getVisibleObjects()`, every object of the world and not
    // only the ones near the player.
    fn lang_resend_npcs(&mut self, player: ObjectId) {
        let mut npcs: Vec<ObjectId> = self
            .npcs
            .iter()
            .filter(|(id, npc)| self.is_npc(**id) && self.custom_misc.texts.has_npc_name(npc.npc_id))
            .map(|(id, _)| *id)
            .collect();
        // In object id order, so the same input gives the same output (D-13).
        npcs.sort_unstable();
        for npc in npcs {
            self.send(player, &DeleteObject { object_id: npc.0 });
            let due = self.tick + TimerWheel::ms_to_ticks(1000);
            self.timers.schedule_at(
                due,
                TimerKind::CustomMisc(CustomMiscTimer::LangNpcInfo(player, npc)),
            );
        }
    }

    /// `activeChar.sendPacket(new NpcInfo(obj.asNpc(), activeChar))`, a second after the delete.
    pub(crate) fn on_lang_npc_info(&mut self, player: ObjectId, npc: ObjectId) {
        if !self.players.contains_key(&player) {
            return;
        }
        if let Some(info) = self.npc_info(npc, player) {
            self.send(player, &info);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(text: &str, params: &[&str]) -> String {
        let params: Vec<String> = params.iter().map(|p| (*p).to_owned()).collect();
        Template::parse(text).unwrap().render(&params)
    }

    #[test]
    fn markers_are_replaced_by_their_parameter() {
        assert_eq!(
            render("Hello $c1, you got $s2 of $s1.", &["Adena", "5"]),
            "Hello Adena, you got 5 of Adena."
        );
        assert_eq!(render("$s2 $S1 $p1 $C2", &["a", "b"]), "b a a b");
    }

    #[test]
    fn a_missing_parameter_reads_param_n() {
        assert_eq!(render("a $s1 b $s2", &["x"]), "a x b [PARAM-2]");
        assert_eq!(render("a $s1 b", &[]), "a [PARAM-1] b");
    }

    #[test]
    fn a_text_with_one_piece_is_not_a_container() {
        assert_eq!(render("plain", &["x"]), "plain");
        // The only marker takes the first parameter, whatever its index is.
        assert_eq!(render("$s2", &["first", "second"]), "first");
        assert_eq!(render("$s1", &[]), "null");
    }

    #[test]
    fn marker_edges() {
        // Not a marker: no digit, a wrong letter, too short.
        assert_eq!(render("$sx $x1 $s", &["a"]), "$sx $x1 $s");
        assert_eq!(render("end$s1", &["a"]), "enda");
        assert!(Template::parse("$s0").is_none());
        assert_eq!(render("$$s1", &["a"]), "$a");
    }

    #[test]
    fn list_parameters_are_one_parameter() {
        let params = list_params(&["a".to_owned(), "b".to_owned()]);
        assert_eq!(
            Template::parse("$s1 and $s2").unwrap().render(&params),
            "[a, b] and [PARAM-2]"
        );
        assert_eq!(Template::parse("$s1").unwrap().render(&list_params(&[])), "[]");
    }

    fn texts() -> LangTexts {
        let mut texts = LangTexts::default();
        texts.send_messages.insert(
            "el".to_owned(),
            vec![
                (vec!["exact".to_owned()], vec!["EXACT".to_owned()]),
                (
                    vec!["You have learned ".to_owned(), " new skills.".to_owned()],
                    vec!["Learned ".to_owned(), " skills.".to_owned()],
                ),
            ],
        );
        texts
    }

    #[test]
    fn send_messages_match_whole_or_in_parts() {
        let texts = texts();
        assert_eq!(texts.send_message("el", "exact"), "EXACT");
        assert_eq!(
            texts.send_message("el", "You have learned 3 new skills."),
            "Learned 3 skills."
        );
        assert_eq!(texts.send_message("el", "unknown"), "unknown");
        assert_eq!(texts.send_message("fr", "exact"), "exact");
    }
}
