//! The buff shop (`SellBuffs.ini`): a player sits down and sells his buffs for a price.
// Off by default (`SellBuffEnable = False`): then neither the bypass words nor the voiced commands
// are registered (Java registers them in the constructor of the handler only when the switch is
// on), and the sell list stays empty.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use l2_data::SellBuffData;
use l2_protocol_game::server::ExPrivateStoreSetWholeMsg;
use l2_util::ObjectId;

use crate::dialog::INTERACTION_DISTANCE;
use crate::effects::SkillKey;
use crate::script::{ScriptCtx, ScriptId};
use crate::trade::PrivateStoreType;
use crate::world::World;
use crate::zones::ZoneId;

/// `SellBuffsConfig`; `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SellBuffsSettings {
    /// `SELLBUFF_ENABLED` (`SellBuffEnable`).
    pub enabled: bool,
    /// `SELLBUFF_MP_MULTIPLER` (`MpCostMultipler`).
    pub mp_multipler: i32,
    /// `SELLBUFF_PAYMENT_ID` (`PaymentID`).
    pub payment_id: i32,
    /// `SELLBUFF_MIN_PRICE` (`MinimumPrice`).
    pub min_price: i64,
    /// `SELLBUFF_MAX_PRICE` (`MaximumPrice`).
    pub max_price: i64,
    /// `SELLBUFF_MAX_BUFFS` (`MaxBuffs`).
    pub max_buffs: i32,
}

impl Default for SellBuffsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            mp_multipler: 1,
            payment_id: 57,
            min_price: 100_000,
            max_price: 100_000_000,
            max_buffs: 15,
        }
    }
}

/// Java `SellBuffHolder`: a skill of a seller and its price.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SellBuffHolder {
    pub skill_id: i32,
    pub price: i64,
}

/// What the buff shop keeps in the world.
#[derive(Debug, Default)]
pub(crate) struct SellBuffsState {
    pub(crate) settings: SellBuffsSettings,
    /// `SellBuffsManager.ALLOWED_BUFFS`.
    pub(crate) data: Option<Arc<SellBuffData>>,
    /// `Player.isSellingBuffs()`.
    selling: BTreeSet<ObjectId>,
    /// `Player.getSellingBuffs()`: kept while the player is online.
    lists: BTreeMap<ObjectId, Vec<SellBuffHolder>>,
}

/// `SellBuffBypassHandler.COMMANDS`.
const COMMANDS: [&str; 10] = [
    "sellbuffadd",
    "sellbuffaddskill",
    "sellbuffedit",
    "sellbuffchangeprice",
    "sellbuffremove",
    "sellbuffbuymenu",
    "sellbuffbuyskill",
    "sellbuffbuyskillPet",
    "sellbuffstart",
    "sellbuffstop",
];

/// `SellBuffsManager.HTML_FOLDER`.
const HTML_FOLDER: &str = "data/html/mods/SellBuffs/";

/// Java `Integer.parseInt`: an optional sign and digits.
fn parse_int(text: &str) -> Option<i32> {
    text.parse::<i32>().ok()
}

/// `new StringTokenizer(text, " ")`.
fn space_tokens(text: &str) -> Vec<&str> {
    text.split(' ').filter(|t| !t.is_empty()).collect()
}

/// Java `skill.getLevel() > 100 ? " + level % 100" : ""` after the name, and the level column.
fn enchant_suffix(level: i32) -> String {
    if level > 100 {
        format!("<font color=\"LEVEL\"> + {}</font></td>", level % 100)
    } else {
        "</td>".to_owned()
    }
}

impl World {
    /// Hands the world the buff shop settings and its skill list (Java `SellBuffsConfig`,
    /// `SellBuffsManager.load`). With the switch on, the `sellbuff*` bypass words are registered
    /// (Java `SellBuffBypassHandler` constructor).
    pub fn set_sell_buffs(&mut self, settings: SellBuffsSettings, data: Arc<SellBuffData>) {
        let enabled = settings.enabled;
        self.custom_buffers.sell.settings = settings;
        self.custom_buffers.sell.data = Some(data);
        if enabled {
            for word in COMMANDS {
                self.register_bypass_handler(word, sell_buff_bypass_handler);
            }
        }
    }

    /// The buff shop's bypass words, for the registry audit.
    pub fn sell_buffs_enabled(&self) -> bool {
        self.custom_buffers.sell.settings.enabled
    }

    /// Java `Player.isSellingBuffs()`.
    pub fn is_selling_buffs(&self, id: ObjectId) -> bool {
        self.custom_buffers.sell.selling.contains(&id)
    }

    /// Java `Player.getSellingBuffs()`.
    pub fn selling_buffs(&self, id: ObjectId) -> &[SellBuffHolder] {
        self.custom_buffers.sell.lists.get(&id).map_or(&[], Vec::as_slice)
    }

    /// `OfflineTraderTable.restoreOfflineTraders` of a `SELL_BUFFS` row: `setSellingBuffs(true)` and
    /// the skills with their prices (the caller then sits the player down).
    pub(crate) fn sell_buffs_restore(&mut self, id: ObjectId, holders: Vec<SellBuffHolder>) {
        self.custom_buffers.sell.selling.insert(id);
        self.custom_buffers
            .sell
            .lists
            .entry(id)
            .or_default()
            .extend(holders);
    }

    /// The player left: his buff list goes with him (Java keeps it in the `Player` object).
    pub(crate) fn sell_buffs_player_left(&mut self, id: ObjectId) {
        self.custom_buffers.sell.selling.remove(&id);
        self.custom_buffers.sell.lists.remove(&id);
    }

    /// The voiced commands of `SellBuffCommandHandler`. `None`: not its word (or the switch is
    /// off, so Java has not registered it).
    pub(crate) fn sell_buffs_voiced_handler(&mut self, player: ObjectId, word: &str) -> Option<bool> {
        if !self.custom_buffers.sell.settings.enabled || !matches!(word, "sellbuff" | "sellbuffs") {
            return None;
        }
        self.sell_buffs_send_sell_menu(player);
        Some(true)
    }

    fn sell_page(&self, player: ObjectId, name: &str) -> Option<String> {
        self.get_htm(player, &format!("{HTML_FOLDER}{name}"))
    }

    /// The skill a player knows, as its template.
    fn sell_known_skill(&self, player: ObjectId, skill_id: i32) -> Option<l2_data::skill::SkillTemplate> {
        let key = self.known_skill(player, skill_id)?;
        self.skill_table()?.get(key.id, key.level).cloned()
    }

    fn sell_in_list(&self, player: ObjectId, skill_id: i32) -> bool {
        self.selling_buffs(player).iter().any(|h| h.skill_id == skill_id)
    }

    /// Java `SellBuffsManager.isInSellList(player, skill)`.
    pub fn is_in_sell_list(&self, player: ObjectId, skill_id: i32) -> bool {
        self.sell_in_list(player, skill_id)
    }

    /// Java `SellBuffsManager.sendSellMenu(player)`.
    pub fn sell_buffs_send_sell_menu(&mut self, player: ObjectId) {
        let name = if self.is_selling_buffs(player) {
            "BuffMenu_already.html"
        } else {
            "BuffMenu.html"
        };
        // `separateAndSend(null, player)` has nothing to send.
        if let Some(html) = self.sell_page(player, name) {
            self.send_cb_html(player, &html);
        }
    }

    /// Java `sendBuffChoiceMenu(player, index)`.
    fn sell_send_buff_choice_menu(&mut self, player: ObjectId, index: i32) {
        let Some(html) = self.sell_page(player, "BuffChoice.html") else {
            return;
        };
        let html = html.replace("%list%", &self.sell_build_skill_menu(player, index));
        self.send_cb_html(player, &html);
    }

    /// Java `sendBuffEditMenu(player)`.
    fn sell_send_buff_edit_menu(&mut self, player: ObjectId) {
        let Some(html) = self.sell_page(player, "BuffChoice.html") else {
            return;
        };
        let html = html.replace("%list%", &self.sell_build_edit_menu(player));
        self.send_cb_html(player, &html);
    }

    /// Java `sendBuffMenu(player, seller, index)`.
    pub(crate) fn sell_send_buff_menu(&mut self, player: ObjectId, seller: ObjectId, index: i32) {
        if !self.is_selling_buffs(seller) || self.selling_buffs(seller).is_empty() {
            return;
        }
        let Some(html) = self.sell_page(player, "BuffBuyMenu.html") else {
            return;
        };
        let html = html.replace("%list%", &self.sell_build_buff_menu(seller, index));
        self.send_cb_html(player, &html);
    }

    /// Java `startSellBuffs(player, title)`.
    fn sell_start(&mut self, player: ObjectId, title: &str) {
        self.sit_down(player, true);
        self.custom_buffers.sell.selling.insert(player);
        self.set_private_store_type(player, PrivateStoreType::PackageSell);
        if let Some(list) = self.sell_list_mut(player) {
            list.title = title.to_owned();
            list.packaged = true;
        }
        self.broadcast_user_info(player);
        let message = self.sell_list_copy(player).title;
        self.broadcast_creature_packet(
            player,
            &ExPrivateStoreSetWholeMsg {
                object_id: player.0,
                message,
            },
        );
        self.sell_buffs_send_sell_menu(player);
    }

    /// Java `stopSellBuffs(player)`.
    fn sell_stop(&mut self, player: ObjectId) {
        self.custom_buffers.sell.selling.remove(&player);
        self.set_private_store_type(player, PrivateStoreType::None);
        self.stand_up(player);
        self.broadcast_user_info(player);
        self.sell_buffs_send_sell_menu(player);
    }

    /// Java `canStartSellBuffs(player)`. Nothing in Java calls it (the start command does not
    /// ask), it is kept for the pages of other mods.
    pub fn can_start_sell_buffs(&mut self, player: ObjectId) -> bool {
        let refusal = if self.is_alike_dead(player) {
            Some("You can't sell buffs in fake death!")
        } else if self.is_in_olympiad_mode(player) || self.olympiad_is_registered(player) {
            Some("You can't sell buffs with Olympiad status!")
        } else if self.is_registered_on_event(player) {
            Some("You can't sell buffs while registered in an event!")
        } else if self.is_cursed_weapon_equipped(player) || self.karma_of_player(player) > 0 {
            Some("You can't sell buffs in Chaotic state!")
        } else if self.is_in_duel(player) {
            Some("You can't sell buffs in Duel state!")
        } else if self.is_fishing(player) {
            Some("You can't sell buffs while fishing.")
        } else if self.is_mounted(player) || self.is_flying(player) {
            Some("You can't sell buffs in Mount state!")
        } else if self.is_transformed(player) {
            Some("You can't sell buffs in Transform state!")
        } else if self.is_inside_zone(player, ZoneId::NoStore)
            || !self.is_inside_zone(player, ZoneId::Peace)
            || self.is_jailed(player)
        {
            Some("You can't sell buffs here!")
        } else {
            None
        };
        match refusal {
            Some(text) => {
                self.send_message(player, text);
                false
            }
            None => true,
        }
    }

    fn sell_payment_name(&self) -> Option<String> {
        let id = self.custom_buffers.sell.settings.payment_id;
        self.item_templates()?.get(id).map(|t| t.name.clone())
    }

    /// The icon, name and level cells both skill tables start a row with.
    fn sell_skill_cells(&self, skill: &l2_data::skill::SkillTemplate) -> String {
        let max_level = self.skill_table().map_or(0, |t| t.max_level(skill.id));
        let mut html = format!(
            "<td align=center><img src=\"{}\" width=\"32\" height=\"32\"></td>",
            skill.icon
        );
        html.push_str(&format!(
            "<td align=center>{}{}",
            skill.name,
            enchant_suffix(skill.level)
        ));
        html.push_str(&format!(
            "<td align=center>{}</td>",
            if skill.level > 100 { max_level } else { skill.level }
        ));
        html
    }

    /// Java `buildBuffMenu(seller, index)`.
    fn sell_build_buff_menu(&self, seller: ObjectId, index: i32) -> String {
        let ceiling = 10;
        let mut next_index = -1;
        let mut previous_index = -1;
        let mut sell_list: Vec<SellBuffHolder> = Vec::new();
        let mut count = 0;
        for holder in self.selling_buffs(seller) {
            count += 1;
            if count > index && count <= ceiling + index {
                sell_list.push(*holder);
            }
        }
        if count > 10 && count > index + 10 {
            next_index = index + 10;
        }
        if index >= 10 {
            previous_index = index - 10;
        }
        let mut empty_fields = ceiling - i32::try_from(sell_list.len()).unwrap_or(ceiling);

        let (mp, max_mp) = self.core_of(seller).map_or((0, 0), |c| {
            (
                c.cur_mp as i64,
                self.stats(seller).map_or(0, |v| i64::from(v.max_mp())),
            )
        });
        let mut sb = String::from("<br>");
        sb.push_str(&crate::script_handlers::npc_view::gauge(
            250,
            mp,
            max_mp,
            "L2UI_CT1.Gauges.Gauge_DF_Large_MP_bg_Center",
            "L2UI_CT1.Gauges.Gauge_DF_Large_MP_Center",
        ));
        sb.push_str("<br>");

        sb.push_str(
            "<table border=0 cellpadding=0 cellspacing=0 background=\"L2UI_CH3.refinewnd_back_Pattern\">",
        );
        sb.push_str("<tr><td><br><br><br></td></tr>");
        sb.push_str("<tr>");
        sb.push_str("<td fixwidth=\"20\"></td>");
        for (label, width) in [
            ("Icon", 75),
            ("Name", 155),
            ("Level", 85),
            ("MP Cost", 100),
            ("Price", 140),
            ("Player", 90),
            ("Pet", 90),
        ] {
            sb.push_str(&format!(
                "<td> <button action=\"\" value=\"{label}\" width={width} height=23 back=\"L2UI_CT1.OlympiadWnd_DF_Watch_Down\" fore=\"L2UI_CT1.OlympiadWnd_DF_Watch\"> </td>"
            ));
        }
        sb.push_str("<td fixwidth=\"20\"></td>");
        sb.push_str("<tr><td><br></td></tr>");
        sb.push_str("</tr>");

        let payment_name = self.sell_payment_name();
        let settings = &self.custom_buffers.sell.settings;
        for holder in &sell_list {
            let Some(skill) = self.sell_known_skill(seller, holder.skill_id) else {
                empty_fields += 1;
                continue;
            };
            sb.push_str("<tr>");
            sb.push_str("<td fixwidth=\"20\"></td>");
            sb.push_str(&self.sell_skill_cells(&skill));
            sb.push_str(&format!(
                "<td align=center> <font color=\"1E90FF\">{}</font></td>",
                i64::from(skill.mp_consume) * i64::from(settings.mp_multipler)
            ));
            sb.push_str(&format!(
                "<td align=center> {} <font color=\"LEVEL\"> {}</font> </td>",
                crate::residence::npc::format_adena(holder.price),
                payment_name.as_deref().unwrap_or("")
            ));
            for word in ["sellbuffbuyskill", "sellbuffbuyskillPet"] {
                sb.push_str(&format!(
                    "<td align=center fixwidth=\"50\"><button value=\"Buy Buff\" action=\"bypass -h {word} {} {} {index}\" width=\"85\" height=\"26\" back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td>",
                    seller.0, skill.id
                ));
            }
            sb.push_str("</tr>");
            sb.push_str("<tr><td><br><br></td></tr>");
        }
        for _ in 0..empty_fields {
            sb.push_str("<tr>");
            sb.push_str("<td fixwidth=\"20\" height=\"32\"></td>");
            sb.push_str("<td align=center></td>");
            sb.push_str("<td align=left></td>");
            sb.push_str("<td align=center></td>");
            sb.push_str("<td align=center></font></td>");
            sb.push_str("<td align=center></td>");
            sb.push_str("<td align=center fixwidth=\"50\"></td>");
            sb.push_str("</tr>");
            sb.push_str("<tr><td><br><br></td></tr>");
        }
        sb.push_str("</table>");

        sb.push_str("<table width=\"250\" border=\"0\">");
        sb.push_str("<tr>");
        if previous_index > -1 {
            sb.push_str(&format!(
                "<td align=left><button value=\"Previous Page\" action=\"bypass -h sellbuffbuymenu {} {previous_index}\" width=\"100\" height=\"30\" back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td>",
                seller.0
            ));
        }
        if next_index > -1 {
            sb.push_str(&format!(
                "<td align=right><button value=\"Next Page\" action=\"bypass -h sellbuffbuymenu {} {next_index}\" width=\"100\" height=\"30\" back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td>",
                seller.0
            ));
        }
        sb.push_str("</tr>");
        sb.push_str("</table>");
        sb
    }

    /// Java `buildEditMenu(player)`.
    fn sell_build_edit_menu(&self, player: ObjectId) -> String {
        let mut sb = String::new();
        sb.push_str(
            "<table border=0 cellpadding=0 cellspacing=0 background=\"L2UI_CH3.refinewnd_back_Pattern\">",
        );
        sb.push_str("<tr><td><br><br><br></td></tr>");
        sb.push_str("<tr>");
        sb.push_str("<td fixwidth=\"10\"></td>");
        for (label, width) in [
            ("Icon", 75),
            ("Name", 150),
            ("Level", 75),
            ("Old Price", 100),
            ("New Price", 125),
            ("Action", 125),
            ("Remove", 85),
        ] {
            sb.push_str(&format!(
                "<td> <button action=\"\" value=\"{label}\" width={width} height=23 back=\"L2UI_CT1.OlympiadWnd_DF_Watch_Down\" fore=\"L2UI_CT1.OlympiadWnd_DF_Watch\"> </td>"
            ));
        }
        sb.push_str("<td fixwidth=\"20\"></td>");
        sb.push_str("</tr>");

        if self.selling_buffs(player).is_empty() {
            sb.push_str("</table>");
            sb.push_str("<br><br><br>");
            sb.push_str("You don't have added any buffs yet!");
        } else {
            for holder in self.selling_buffs(player) {
                let Some(skill) = self.sell_known_skill(player, holder.skill_id) else {
                    continue;
                };
                let max_level = self.skill_table().map_or(0, |t| t.max_level(skill.id));
                sb.push_str("<tr>");
                sb.push_str("<td fixwidth=\"20\"></td>");
                sb.push_str(&format!(
                    "<td align=center><img src=\"{}\" width=\"32\" height=\"32\"></td>",
                    skill.icon
                ));
                sb.push_str(&format!(
                    "<td align=left>{}{}",
                    skill.name,
                    enchant_suffix(skill.level)
                ));
                sb.push_str(&format!(
                    "<td align=center>{}</td>",
                    if skill.level > 100 { max_level } else { skill.level }
                ));
                sb.push_str(&format!(
                    "<td align=center> {} </td>",
                    crate::residence::npc::format_adena(holder.price)
                ));
                sb.push_str(&format!(
                    "<td align=center><edit var=\"price_{}\" width=120 type=\"number\"></td>",
                    skill.id
                ));
                sb.push_str(&format!(
                    "<td align=center><button value=\"Edit\" action=\"bypass -h sellbuffchangeprice {} $price_{}\" width=\"85\" height=\"26\" back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td>",
                    skill.id, skill.id
                ));
                sb.push_str(&format!(
                    "<td align=center><button value=\" X \" action=\"bypass -h sellbuffremove {}\" width=\"26\" height=\"26\" back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td>",
                    skill.id
                ));
                sb.push_str("</tr>");
                sb.push_str("<tr><td><br><br></td></tr>");
            }
            sb.push_str("</table>");
        }
        sb
    }

    /// Java `buildSkillMenu(player, index)`.
    fn sell_build_skill_menu(&self, player: ObjectId, index: i32) -> String {
        let ceiling = index + 10;
        let mut next_index = -1;
        let mut previous_index = -1;
        let mut skill_list: Vec<l2_data::skill::SkillTemplate> = Vec::new();
        let data = self.custom_buffers.sell.data.clone().unwrap_or_default();
        let table = self.skill_table();

        let mut count = 0;
        let known: Vec<(i32, i32)> = self.core_of(player).map_or_else(Vec::new, |c| c.skills.clone());
        for (skill_id, level) in known {
            if data.is_allowed(skill_id) && !self.sell_in_list(player, skill_id) {
                count += 1;
                if count > index
                    && count <= ceiling
                    && let Some(skill) = table.as_ref().and_then(|t| t.get(skill_id, level))
                {
                    skill_list.push(skill.clone());
                }
            }
        }
        if count > 10 && count > index + 10 {
            next_index = index + 10;
        }
        if index >= 10 {
            previous_index = index - 10;
        }

        let mut sb = String::new();
        sb.push_str(
            "<table border=0 cellpadding=0 cellspacing=0 background=\"L2UI_CH3.refinewnd_back_Pattern\">",
        );
        sb.push_str("<tr><td><br><br><br></td></tr>");
        sb.push_str("<tr>");
        sb.push_str("<td fixwidth=\"10\"></td>");
        for (label, width) in [
            ("Icon", 100),
            ("Name", 175),
            ("Level", 150),
            ("Price", 150),
            ("Action", 125),
        ] {
            sb.push_str(&format!(
                "<td> <button action=\"\" value=\"{label}\" width={width} height=23 back=\"L2UI_CT1.OlympiadWnd_DF_Watch_Down\" fore=\"L2UI_CT1.OlympiadWnd_DF_Watch\"> </td>"
            ));
        }
        sb.push_str("<td fixwidth=\"20\"></td>");
        sb.push_str("</tr>");

        if skill_list.is_empty() {
            sb.push_str("</table>");
            sb.push_str("<br><br><br>");
            sb.push_str("At this moment you cannot add any buffs!");
        } else {
            for skill in &skill_list {
                sb.push_str("<tr>");
                sb.push_str("<td fixwidth=\"20\"></td>");
                let max_level = table.as_ref().map_or(0, |t| t.max_level(skill.id));
                sb.push_str(&format!(
                    "<td align=center><img src=\"{}\" width=\"32\" height=\"32\"></td>",
                    skill.icon
                ));
                sb.push_str(&format!(
                    "<td align=left>{}{}",
                    skill.name,
                    enchant_suffix(skill.level)
                ));
                sb.push_str(&format!(
                    "<td align=center>{}</td>",
                    if skill.level > 100 { max_level } else { skill.level }
                ));
                sb.push_str(&format!(
                    "<td align=center><edit var=\"price_{}\" width=120 type=\"number\"></td>",
                    skill.id
                ));
                sb.push_str(&format!(
                    "<td align=center fixwidth=\"50\"><button value=\"Add Buff\" action=\"bypass -h sellbuffaddskill {} $price_{}\" width=\"85\" height=\"26\" back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td>",
                    skill.id, skill.id
                ));
                sb.push_str("</tr>");
                sb.push_str("<tr><td><br><br></td></tr>");
            }
            sb.push_str("</table>");
        }

        sb.push_str("<table width=\"250\" border=\"0\">");
        sb.push_str("<tr>");
        if previous_index > -1 {
            sb.push_str(&format!(
                "<td align=left><button value=\"Previous Page\" action=\"bypass -h sellbuffadd {previous_index}\" width=\"100\" height=\"30\" back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td>"
            ));
        }
        if next_index > -1 {
            sb.push_str(&format!(
                "<td align=right><button value=\"Next Page\" action=\"bypass -h sellbuffadd {next_index}\" width=\"100\" height=\"30\" back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td>"
            ));
        }
        sb.push_str("</tr>");
        sb.push_str("</table>");
        sb
    }

    /// `SellBuffBypassHandler.onCommand(command, player, params)`: always `true` for a known
    /// word, `false` where Java returns it. A `NumberFormatException` of Java ends the command.
    fn sell_buff_command(&mut self, player: ObjectId, command: &str, params: &str) -> bool {
        if !self.custom_buffers.sell.settings.enabled {
            return false;
        }
        match command {
            "sellbuffstart" => {
                if self.is_selling_buffs(player) || params.is_empty() {
                    return false;
                }
                if self.selling_buffs(player).is_empty() {
                    self.send_message(
                        player,
                        "Your list of buffs is empty, please add some buffs first!",
                    );
                    return false;
                }
                let mut title = String::from("BUFF SELL: ");
                for token in space_tokens(params) {
                    title.push_str(token);
                    title.push(' ');
                }
                // Java `StringBuilder.length()` counts UTF-16 units.
                if title.encode_utf16().count() > 40 {
                    self.send_message(
                        player,
                        "Your title cannot exceed 29 characters in length. Please try again.",
                    );
                    return false;
                }
                self.sell_start(player, &title);
            }
            "sellbuffstop" => {
                if self.is_selling_buffs(player) {
                    self.sell_stop(player);
                }
            }
            "sellbuffadd" => {
                if !self.is_selling_buffs(player) {
                    let mut index = 0;
                    if !params.is_empty() && params.chars().all(|c| c.is_ascii_digit()) {
                        let Some(parsed) = parse_int(params) else {
                            return true;
                        };
                        index = parsed;
                    }
                    self.sell_send_buff_choice_menu(player, index);
                }
            }
            "sellbuffedit" => {
                if !self.is_selling_buffs(player) {
                    self.sell_send_buff_edit_menu(player);
                }
            }
            "sellbuffchangeprice" => return self.sell_change_price(player, params),
            "sellbuffremove" => return self.sell_remove(player, params),
            "sellbuffaddskill" => return self.sell_add_skill(player, params),
            "sellbuffbuymenu" => self.sell_buy_menu(player, params),
            "sellbuffbuyskill" => return self.sell_buy_skill(player, params, false),
            "sellbuffbuyskillPet" => return self.sell_buy_skill(player, params, true),
            _ => {}
        }
        true
    }

    fn sell_change_price(&mut self, player: ObjectId, params: &str) -> bool {
        if self.is_selling_buffs(player) || params.is_empty() {
            return true;
        }
        let mut st = space_tokens(params).into_iter();
        let mut skill_id = -1;
        let mut price: i32 = -1;
        if let Some(token) = st.next() {
            let Some(parsed) = parse_int(token) else {
                return true;
            };
            skill_id = parsed;
        }
        if let Some(token) = st.next() {
            match parse_int(token) {
                Some(parsed) => price = parsed,
                None => {
                    let max = self.custom_buffers.sell.settings.max_price;
                    self.send_message(player, &format!("Too big price! Maximum price is {max}"));
                    self.sell_send_buff_edit_menu(player);
                }
            }
        }
        if skill_id == -1 || price == -1 {
            return false;
        }
        let Some(skill) = self.sell_known_skill(player, skill_id) else {
            return false;
        };
        let list = self.custom_buffers.sell.lists.entry(player).or_default();
        if let Some(holder) = list.iter_mut().find(|h| h.skill_id == skill.id) {
            holder.price = i64::from(price);
            self.send_message(
                player,
                &format!("Price of {} has been changed to {price}!", skill.name),
            );
            self.sell_send_buff_edit_menu(player);
        }
        true
    }

    fn sell_remove(&mut self, player: ObjectId, params: &str) -> bool {
        if self.is_selling_buffs(player) || params.is_empty() {
            return true;
        }
        let mut st = space_tokens(params).into_iter();
        let mut skill_id = -1;
        if let Some(token) = st.next() {
            let Some(parsed) = parse_int(token) else {
                return true;
            };
            skill_id = parsed;
        }
        if skill_id == -1 {
            return false;
        }
        let Some(skill) = self.sell_known_skill(player, skill_id) else {
            return false;
        };
        let list = self.custom_buffers.sell.lists.entry(player).or_default();
        if let Some(at) = list.iter().position(|h| h.skill_id == skill.id) {
            list.remove(at);
            self.send_message(player, &format!("Skill {} has been removed!", skill.name));
            self.sell_send_buff_edit_menu(player);
        }
        true
    }

    fn sell_add_skill(&mut self, player: ObjectId, params: &str) -> bool {
        if self.is_selling_buffs(player) || params.is_empty() {
            return true;
        }
        let mut st = space_tokens(params).into_iter();
        let mut skill_id = -1;
        let mut price: i64 = -1;
        if let Some(token) = st.next() {
            let Some(parsed) = parse_int(token) else {
                return true;
            };
            skill_id = parsed;
        }
        if let Some(token) = st.next() {
            match parse_int(token) {
                Some(parsed) => price = i64::from(parsed),
                None => {
                    let min = self.custom_buffers.sell.settings.min_price;
                    self.send_message(player, &format!("Too big price! Maximum price is {min}"));
                    self.sell_send_buff_edit_menu(player);
                }
            }
        }
        if skill_id == -1 || price == -1 {
            return false;
        }
        let Some(skill) = self.sell_known_skill(player, skill_id) else {
            return false;
        };
        let settings = self.custom_buffers.sell.settings.clone();
        if price < settings.min_price {
            self.send_message(
                player,
                &format!("Too small price! Minimum price is {}", settings.min_price),
            );
            return false;
        } else if price > settings.max_price {
            self.send_message(
                player,
                &format!("Too big price! Maximum price is {}", settings.max_price),
            );
            return false;
        } else if i64::try_from(self.selling_buffs(player).len()).unwrap_or(i64::MAX)
            >= i64::from(settings.max_buffs)
        {
            self.send_message(
                player,
                &format!(
                    "You already reached max count of buffs! Max buffs is: {}",
                    settings.max_buffs
                ),
            );
            return false;
        } else if !self.sell_in_list(player, skill.id) {
            self.custom_buffers
                .sell
                .lists
                .entry(player)
                .or_default()
                .push(SellBuffHolder {
                    skill_id: skill.id,
                    price,
                });
            self.send_message(player, &format!("{} has been added!", skill.name));
            self.sell_send_buff_choice_menu(player, 0);
        }
        true
    }

    fn sell_buy_menu(&mut self, player: ObjectId, params: &str) {
        if params.is_empty() {
            return;
        }
        let mut st = space_tokens(params).into_iter();
        let mut obj_id = -1;
        let mut index = 0;
        if let Some(token) = st.next() {
            let Some(parsed) = parse_int(token) else {
                return;
            };
            obj_id = parsed;
        }
        if let Some(token) = st.next() {
            let Some(parsed) = parse_int(token) else {
                return;
            };
            index = parsed;
        }
        let seller = ObjectId(obj_id);
        if !self.players.contains_key(&seller) {
            return;
        }
        if !self.is_selling_buffs(seller)
            || !self.is_inside_radius_3d_of(player, seller, INTERACTION_DISTANCE)
        {
            return;
        }
        self.sell_send_buff_menu(player, seller, index);
    }

    /// `player.isInsideRadius3D(other, radius)`: strictly below.
    fn is_inside_radius_3d_of(&self, player: ObjectId, other: ObjectId, radius: i32) -> bool {
        self.position_of(other)
            .is_some_and(|to| self.inside_radius_3d(player, to, radius))
    }

    /// `sellbuffbuyskill` (`for_pet` false) and `sellbuffbuyskillPet`.
    fn sell_buy_skill(&mut self, player: ObjectId, params: &str, for_pet: bool) -> bool {
        if params.is_empty() {
            return true;
        }
        let mut st = space_tokens(params).into_iter();
        let mut obj_id = -1;
        let mut skill_id = -1;
        let mut index = 0;
        for slot in [&mut obj_id, &mut skill_id, &mut index] {
            if let Some(token) = st.next() {
                let Some(parsed) = parse_int(token) else {
                    return true;
                };
                *slot = parsed;
            }
        }
        if skill_id == -1 || obj_id == -1 {
            return false;
        }
        let seller = ObjectId(obj_id);
        if !self.players.contains_key(&seller) {
            return false;
        }
        let skill = self.sell_known_skill(seller, skill_id);
        // `LocationUtil.checkIfInRange(INTERACTION_DISTANCE, player, seller, true)`.
        if !self.is_selling_buffs(seller)
            || !self.check_if_in_range(INTERACTION_DISTANCE, player, seller, true)
            || skill.is_none()
        {
            return false;
        }
        let Some(skill) = skill else {
            return false;
        };
        let settings = self.custom_buffers.sell.settings.clone();
        let seller_name = self.creature_name(seller);
        let mp_cost = i64::from(skill.mp_consume) * i64::from(settings.mp_multipler);
        if self.core_of(seller).is_some_and(|c| c.cur_mp < mp_cost as f64) {
            self.send_message(
                player,
                &format!("{seller_name} has not enough mana for {}!", skill.name),
            );
            self.sell_send_buff_menu(player, seller, index);
            return false;
        }

        // Check if buff requires item to cast and seller has enough.
        let (item_consume_id, item_consume_count) = (skill.item_consume_id, skill.item_consume_count);
        if item_consume_id > 0 && item_consume_count > 0 {
            let available = self.inventory_item_count(seller, item_consume_id);
            if available < i64::from(item_consume_count) {
                let item_name = self
                    .item_templates()
                    .and_then(|t| t.get(item_consume_id).map(|i| i.name.clone()))
                    .unwrap_or_else(|| "required item".to_owned());
                // Check if the seller is online before sending them the message.
                let buyer_name = self.creature_name(player);
                self.send_message(
                    seller,
                    &format!(
                        "{buyer_name} tried to buy {} but you do not have enough {item_name}!",
                        skill.name
                    ),
                );
                self.send_message(
                    player,
                    &format!(
                        "{seller_name} doesn't have enough {item_name} to cast {}!",
                        skill.name
                    ),
                );
                self.sell_send_buff_menu(player, seller, index);
                return false;
            }
        }

        let holder = self
            .selling_buffs(seller)
            .iter()
            .find(|h| h.skill_id == skill.id)
            .copied();
        if let Some(holder) = holder {
            if self.inventory_item_count(player, settings.payment_id) >= holder.price {
                let pet = self.summon_of(player).filter(|s| !self.is_dead(*s));
                if for_pet && pet.is_none() {
                    self.send_message(player, "Your pet must be summoned and alive to receive buffs.");
                } else {
                    let mut ctx = ScriptCtx::new(self, ScriptId(0));
                    ctx.take_items(player, settings.payment_id, holder.price);
                    ctx.give_items(seller, settings.payment_id, holder.price);
                    self.reduce_mp(seller, mp_cost as f64);
                    // Consume item(s) required by the buff.
                    if item_consume_id > 0 && item_consume_count > 0 {
                        self.destroy_item_by_item_id(
                            seller,
                            item_consume_id,
                            i64::from(item_consume_count),
                            true,
                        );
                    }
                    // Cast buff.
                    let target = if for_pet { pet.unwrap_or(player) } else { player };
                    self.activate_skill(
                        seller,
                        &[target],
                        SkillKey {
                            id: skill.id,
                            level: skill.level,
                        },
                    );
                }
            } else {
                match self.sell_payment_name() {
                    Some(name) => self.send_message(player, &format!("Not enough {name}!")),
                    None => self.send_message(player, "Not enough items!"),
                }
            }
        }
        self.sell_send_buff_menu(player, seller, index);
        true
    }
}

/// `SellBuffBypassHandler.onCommand(command, player, target)`: splits the command at the spaces.
fn sell_buff_bypass_handler(
    world: &mut World,
    player: ObjectId,
    command: &str,
    _target: Option<ObjectId>,
) -> bool {
    let mut st = space_tokens(command).into_iter();
    let Some(cmd) = st.next() else {
        return false;
    };
    let params = st.collect::<Vec<_>>().join(" ");
    world.sell_buff_command(player, cmd, &params)
}
