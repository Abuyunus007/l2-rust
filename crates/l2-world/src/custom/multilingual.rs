//! `config/Custom/MultilingualSupport.ini`: the player's language, the `.lang` voiced command and
//! the html pages in the language of the player.
// Java: dist/game/data/scripts/handlers/chat/commands/voiced/Lang.java
// Java: entity/actor/Player.java (getLang, setLang, getHtmlPrefix)
//
// Not ported (a follow-up): the translated texts of `data/lang/<lang>/` (NPC names, NPC strings,
// system messages, `sendMessage` texts), the `characters.language` column (the choice lasts until
// the player leaves the world) and the language in `CharSelectionInfo`.

use l2_protocol_game::server::NpcHtmlMessage;
use l2_util::ObjectId;

use super::misc::tokens;
use crate::world::World;

impl World {
    /// Java `Player.getLang()`: `None` until the player chose (Java: null).
    pub fn player_lang(&self, id: ObjectId) -> Option<&str> {
        self.custom_misc.lang.get(&id).map(String::as_str)
    }

    /// Java `Player.getHtmlPrefix()`: empty unless `MultiLangEnable`, and for English.
    pub(crate) fn html_prefix(&self, id: ObjectId) -> String {
        if !self.settings.custom_misc.multilang.enable {
            return String::new();
        }
        match self.custom_misc.lang.get(&id) {
            Some(lang) if lang != "en" => format!("data/lang/{lang}/"),
            _ => String::new(),
        }
    }

    /// Java `Player.setLang(lang)`: `true` when the language is allowed. A language that is not
    /// allowed sets the default one.
    pub(crate) fn set_player_lang(&mut self, id: ObjectId, lang: &str) -> bool {
        let m = &self.settings.custom_misc.multilang;
        if !m.enable {
            self.custom_misc.lang.remove(&id);
            return false;
        }
        let allowed = m.allowed.iter().any(|a| a == lang);
        let chosen = if allowed {
            lang.to_owned()
        } else {
            m.default_lang.clone()
        };
        self.custom_misc.lang.insert(id, chosen);
        allowed
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
        if self.set_player_lang(player, lang) {
            self.lang_page(player, "data/html/mods/Lang/Ok.htm", None);
            // NOTE: Java re-sends every NPC that has a name translation; no translation data is
            // loaded here, so there is none to send.
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
        if let Some(message) = NpcHtmlMessage::new(0, &html) {
            self.send_html(player, &message);
        }
    }
}
