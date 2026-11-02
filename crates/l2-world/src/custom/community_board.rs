//! The custom community board (`CommunityBoard.ini`): multisells, a shop list, teleports, buffs,
//! heal and delevel from the board's home page.
// Java: dist/game/data/scripts/handlers/bypass/communityboard/HomeBoard.java (`CUSTOM_COMMANDS`,
// Java: `COMBAT_CHECK`, and the custom branches of `onCommand`)
// Java: gameserver/config/custom/CommunityBoardConfig.java
//
// # What is a switch here
// `CustomCommunityBoard = False` (the default) turns the whole feature off: the pages of
// `data/html/CommunityBoard/Custom/` are not used and none of the custom commands is taken.
// NOTE(java-quirk): Java registers the custom command words (`_bbsheal`, `_bbsteleport`, ...)
// whatever `CustomCommunityBoard` says, and `_bbs...` commands need no page that offered them, so
// a modified client could heal or teleport for the (default 0) price with the board off. Here the
// words are taken only with `CustomCommunityBoard = True` ("every feature is off by default",
// D-57).
//
// `_bbspremium` (M9-C02) buys premium days with `CommunityPremiumSystem` and the premium system.

use std::collections::{BTreeSet, HashMap};

use l2_protocol_game::server::{MagicSkillUse, ShowBoard, SkillPosition};
use l2_util::ObjectId;

use crate::community_board::BoardTimer;
use crate::effects::SkillKey;
use crate::teleport::TeleportTarget;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;
use crate::zones::ZoneId;

/// The rest of `CommunityBoardConfig` (the switch, the peace and karma keys are in
/// `CommunityBoardSettings`); `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomBoardSettings {
    /// `COMMUNITYBOARD_CURRENCY` (`CommunityCurrencyId`).
    pub currency: i32,
    /// `COMMUNITYBOARD_ENABLE_MULTISELLS` (`CommunityEnableMultisells`).
    pub enable_multisells: bool,
    /// `COMMUNITYBOARD_ENABLE_TELEPORTS` (`CommunityEnableTeleports`).
    pub enable_teleports: bool,
    /// `COMMUNITYBOARD_ENABLE_BUFFS` (`CommunityEnableBuffs`).
    pub enable_buffs: bool,
    /// `COMMUNITYBOARD_ENABLE_HEAL` (`CommunityEnableHeal`).
    pub enable_heal: bool,
    /// `COMMUNITYBOARD_ENABLE_DELEVEL` (`CommunityEnableDelevel`).
    pub enable_delevel: bool,
    /// `COMMUNITYBOARD_TELEPORT_PRICE` (`CommunityTeleportPrice`).
    pub teleport_price: i32,
    /// `COMMUNITYBOARD_BUFF_PRICE` (`CommunityBuffPrice`).
    pub buff_price: i32,
    /// `COMMUNITYBOARD_HEAL_PRICE` (`CommunityHealPrice`).
    pub heal_price: i32,
    /// `COMMUNITYBOARD_DELEVEL_PRICE` (`CommunityDelevelPrice`).
    pub delevel_price: i32,
    /// `COMMUNITYBOARD_COMBAT_DISABLED` (`CommunityCombatDisabled`).
    pub combat_disabled: bool,
    /// `COMMUNITYBOARD_CAST_ANIMATIONS` (`CommunityCastAnimations`).
    pub cast_animations: bool,
    /// `COMMUNITY_AVAILABLE_BUFFS` (`CommunityAvailableBuffs`).
    pub available_buffs: BTreeSet<i32>,
    /// `COMMUNITY_AVAILABLE_TELEPORTS` (`CommunityTeleportList`): name to place.
    pub available_teleports: HashMap<String, (i32, i32, i32)>,
    /// `COMMUNITY_PREMIUM_SYSTEM_ENABLED` (`CommunityPremiumSystem`).
    pub premium_enabled: bool,
    /// `COMMUNITY_PREMIUM_COIN_ID` (`CommunityPremiumBuyCoinId`).
    pub premium_coin_id: i32,
    /// `COMMUNITY_PREMIUM_PRICE_PER_DAY` (`CommunityPremiumPricePerDay`).
    pub premium_price_per_day: i32,
}

impl Default for CustomBoardSettings {
    fn default() -> Self {
        Self {
            currency: 57,
            enable_multisells: true,
            enable_teleports: true,
            enable_buffs: true,
            enable_heal: true,
            enable_delevel: false,
            teleport_price: 0,
            buff_price: 0,
            heal_price: 0,
            delevel_price: 0,
            combat_disabled: true,
            cast_animations: false,
            available_buffs: BTreeSet::new(),
            available_teleports: HashMap::new(),
            premium_enabled: false,
            premium_coin_id: 57,
            premium_price_per_day: 1_000_000,
        }
    }
}

impl CustomBoardSettings {
    /// Java `CommunityAvailableBuffs`: `split(",")`, each part an integer (a part that is not one
    /// is skipped here; Java throws and the server does not start).
    pub fn parse_buffs(text: &str) -> BTreeSet<i32> {
        text.split(',').filter_map(|s| s.trim().parse().ok()).collect()
    }

    /// Java `CommunityTeleportList`: `Name,X,Y,Z;Name,X,Y,Z...` (a malformed entry is skipped
    /// here; Java throws).
    pub fn parse_teleports(text: &str) -> HashMap<String, (i32, i32, i32)> {
        let mut map = HashMap::new();
        for entry in text.split(';') {
            let parts: Vec<&str> = entry.split(',').collect();
            if let [name, x, y, z, ..] = parts.as_slice()
                && let (Ok(x), Ok(y), Ok(z)) = (x.trim().parse(), y.trim().parse(), z.trim().parse())
            {
                map.insert((*name).to_owned(), (x, y, z));
            }
        }
        map
    }
}

/// The delayed tasks of the custom board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomBoardTimer {
    /// `ThreadPool.schedule(() -> MultisellData.separateAndSend(id, player, null, inventoryOnly), 100)`.
    Multisell {
        player: ObjectId,
        list_id: i32,
        inventory_only: bool,
    },
    /// `_bbssell`: the `BuyList` 423 and the `ExBuySellList`, after 100 ms.
    SellList { player: ObjectId },
    /// `_bbsteleport`: `ThreadPool.schedule(player::enableAllSkills, 3000)`.
    EnableSkills { player: ObjectId },
}

/// Java `String.split(regex)` for a one character separator: trailing empty strings are dropped,
/// and a text without the separator is one part.
fn java_split(text: &str, separator: char) -> Vec<&str> {
    if !text.contains(separator) {
        return vec![text];
    }
    let mut parts: Vec<&str> = text.split(separator).collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// The `BoardTimer` the board's timer kind carries.
pub(crate) fn board_timer(timer: CustomBoardTimer) -> TimerKind {
    TimerKind::Board(BoardTimer::Custom(timer))
}

impl World {
    /// Installs the custom board settings (`Custom/CommunityBoard.ini`).
    pub fn set_custom_board_settings(&mut self, settings: CustomBoardSettings) {
        self.custom_buffers.board = settings;
    }

    /// The custom board settings.
    pub fn custom_board_settings(&self) -> &CustomBoardSettings {
        &self.custom_buffers.board
    }

    /// The custom command words that are registered (Java `getCommandList()` without the
    /// `null`s), none with `CustomCommunityBoard = False`.
    pub(crate) fn custom_board_words(&self) -> Vec<&'static str> {
        if !self.community_board.settings.custom_cb_enabled {
            return Vec::new();
        }
        let board = &self.custom_buffers.board;
        let mut words = Vec::new();
        if board.enable_multisells {
            words.extend(["_bbsexcmultisell", "_bbsmultisell", "_bbssell"]);
        }
        if board.enable_teleports {
            words.push("_bbsteleport");
        }
        if board.enable_buffs {
            words.push("_bbsbuff");
        }
        if board.enable_heal {
            words.push("_bbsheal");
        }
        if board.enable_delevel {
            words.push("_bbsdelevel");
        }
        // `PREMIUM_SYSTEM_ENABLED && COMMUNITY_PREMIUM_SYSTEM_ENABLED ? "_bbspremium" : null`
        if self.premium_system_enabled() && board.premium_enabled {
            words.push("_bbspremium");
        }
        words
    }

    /// Whether a command is one of the registered custom words (the prefix test of Java).
    pub(crate) fn is_custom_board_command(&self, command: &str) -> bool {
        let lower = command.to_lowercase();
        self.custom_board_words().iter().any(|w| lower.starts_with(w))
    }

    /// Java `HomeBoard.COMBAT_CHECK` as `onCommand` asks it (with `COMMUNITYBOARD_COMBAT_DISABLED`).
    pub(crate) fn custom_board_combat_check(&self, id: ObjectId, command: &str) -> bool {
        if !self.custom_buffers.board.combat_disabled {
            return false;
        }
        // `command.startsWith(c)` is case sensitive.
        if !self.custom_board_words().iter().any(|w| command.starts_with(w)) {
            return false;
        }
        // NOTE: `player.isOnEvent()` (the custom events) does not exist in this server.
        self.is_casting_now(id)
            || self.is_casting_simultaneously_now(id)
            || self.is_in_combat(id)
            || self.is_in_duel(id)
            || self.is_in_olympiad_mode(id)
            || self.is_inside_zone(id, ZoneId::Siege)
            || self.is_inside_zone(id, ZoneId::Pvp)
            || self.pvp_flag_of(id) > 0
            || self.is_alike_dead(id)
            || self.is_in_store_mode(id)
    }

    /// Java `Player.getInventory().getInventoryItemCount(currency, -1)`.
    fn board_currency_count(&self, id: ObjectId) -> i64 {
        self.inventory_item_count(id, self.custom_buffers.board.currency)
    }

    /// `player.destroyItemByItemId(FEE, COMMUNITYBOARD_CURRENCY, price, player, true)`.
    fn board_pay(&mut self, id: ObjectId, price: i64) {
        let currency = self.custom_buffers.board.currency;
        self.destroy_item_by_item_id(id, currency, price, true);
    }

    fn board_custom_page(&self, id: ObjectId, page: &str) -> Option<String> {
        self.get_htm(id, &format!("data/html/CommunityBoard/Custom/{page}.html"))
    }

    /// The custom branches of `HomeBoard.onCommand`: the `returnHtml` (before `%navigation%`).
    /// A Java exception (`NumberFormatException`, `ArrayIndexOutOfBoundsException`, a `null`
    /// skill) ends the command: `None`.
    pub(crate) fn custom_home_command(&mut self, id: ObjectId, command: &str) -> Option<String> {
        if command.starts_with("_bbsmultisell") {
            return self.board_multisell(id, command, "_bbsmultisell;", false);
        }
        if command.starts_with("_bbsexcmultisell") {
            return self.board_multisell(id, command, "_bbsexcmultisell;", true);
        }
        if command.starts_with("_bbssell") {
            let page = command.replace("_bbssell;", "");
            let html = self.board_custom_page(id, &page);
            let due = self.tick + TimerWheel::ms_to_ticks(100);
            self.timers
                .schedule_at(due, board_timer(CustomBoardTimer::SellList { player: id }));
            return html;
        }
        if command.starts_with("_bbsteleport") {
            self.board_teleport(id, command);
            return None;
        }
        if command.starts_with("_bbsbuff") {
            return self.board_buff(id, command);
        }
        if command.starts_with("_bbsheal") {
            return self.board_heal(id, command);
        }
        if command == "_bbsdelevel" {
            return self.board_delevel(id);
        }
        if command.starts_with("_bbspremium") {
            return self.board_premium(id, command);
        }
        None
    }

    /// `HomeBoard`'s `_bbspremium;<days>`: 1 to 30 days for `CommunityPremiumPricePerDay` each.
    /// A days part that is no number is Java's `NumberFormatException`: `None`.
    fn board_premium(&mut self, id: ObjectId, command: &str) -> Option<String> {
        let rest = command.replace("_bbspremium;", "");
        let days: i64 = rest.split(',').next()?.parse::<i32>().ok()?.into();
        let board = &self.custom_buffers.board;
        let (coin, per_day) = (board.premium_coin_id, i64::from(board.premium_price_per_day));
        let price = per_day.wrapping_mul(days);
        if !(1..=30).contains(&days) || self.inventory_item_count(id, coin) < price {
            self.send_message(id, "Not enough currency!");
            return None;
        }
        let key = self.premium_key(id)?;
        self.destroy_item_by_item_id(id, coin, price, true);
        self.add_premium_time(&key, days);
        let until = crate::custom::premium::premium_date(self.premium_expiration(&key));
        self.send_message(id, &format!("You will now have premium status until {until}."));
        if self.premium_pc_cafe_retail_like() {
            self.pc_cafe_run(id);
        }
        self.board_custom_page(id, "premium/thankyou")
    }

    fn board_multisell(
        &mut self,
        id: ObjectId,
        command: &str,
        prefix: &str,
        inventory_only: bool,
    ) -> Option<String> {
        let full_bypass = command.replace(prefix, "");
        let options = java_split(&full_bypass, ',');
        let list_id: i32 = options.first()?.parse().ok()?;
        let page = options.get(1)?;
        let html = self.board_custom_page(id, page);
        let due = self.tick + TimerWheel::ms_to_ticks(100);
        self.timers.schedule_at(
            due,
            board_timer(CustomBoardTimer::Multisell {
                player: id,
                list_id,
                inventory_only,
            }),
        );
        html
    }

    fn board_teleport(&mut self, id: ObjectId, command: &str) {
        let tele = command.replace("_bbsteleport;", "");
        let price = i64::from(self.custom_buffers.board.teleport_price);
        if self.board_currency_count(id) < price {
            self.send_message(id, "Not enough currency!");
            return;
        }
        let Some(&(x, y, z)) = self.custom_buffers.board.available_teleports.get(&tele) else {
            return;
        };
        self.disable_all_skills(id);
        self.send(id, &ShowBoard::hide());
        self.board_pay(id, price);
        self.set_in_7s_dungeon(id, false);
        self.set_instance_id(id, 0);
        self.tele_to_location(id, TeleportTarget::at(x, y, z));
        let due = self.tick + TimerWheel::ms_to_ticks(3000);
        self.timers
            .schedule_at(due, board_timer(CustomBoardTimer::EnableSkills { player: id }));
    }

    fn board_buff(&mut self, id: ObjectId, command: &str) -> Option<String> {
        let full_bypass = command.replace("_bbsbuff;", "");
        let options = java_split(&full_bypass, ';');
        // `buypassOptions[buffCount]` with `buffCount = length - 1`: an empty split throws.
        let buff_count = options.len().checked_sub(1)?;
        let page = options[buff_count];
        // Java's `int` product.
        let cost = self
            .custom_buffers
            .board
            .buff_price
            .wrapping_mul(i32::try_from(buff_count).unwrap_or(i32::MAX));
        if self.board_currency_count(id) < i64::from(cost) {
            self.send_message(id, "Not enough currency!");
        } else {
            self.board_pay(id, i64::from(cost));
            let mut targets = vec![id];
            if let Some(pet) = self.summon_of(id) {
                targets.push(pet);
            }
            for option in options.iter().take(buff_count) {
                let parts = java_split(option, ',');
                let skill_id: i32 = parts.first()?.parse().ok()?;
                let level: i32 = parts.get(1)?.parse().ok()?;
                // `SkillData.getSkill` is null for an unknown skill: `skill.getId()` throws.
                let template = self.skill_table()?.get(skill_id, level)?.clone();
                if !self.custom_buffers.board.available_buffs.contains(&template.id) {
                    continue;
                }
                for &target in &targets {
                    self.apply_effects(
                        id,
                        target,
                        SkillKey {
                            id: template.id,
                            level: template.level,
                        },
                    );
                    if self.custom_buffers.board.cast_animations {
                        self.board_cast_animation(id, target, &template);
                    }
                }
            }
        }
        self.board_custom_page(id, page)
    }

    /// `player.sendPacket(new MagicSkillUse(player, target, id, level, hitTime, reuseDelay))`.
    fn board_cast_animation(
        &mut self,
        player: ObjectId,
        target: ObjectId,
        skill: &l2_data::skill::SkillTemplate,
    ) {
        let (Some(c), Some(t)) = (self.position_of(player), self.position_of(target)) else {
            return;
        };
        let position = |p: (i32, i32, i32)| SkillPosition {
            x: p.0,
            y: p.1,
            z: p.2,
        };
        self.send(
            player,
            &MagicSkillUse {
                creature_id: player.0,
                target_id: target.0,
                skill_id: skill.id,
                skill_level: skill.level,
                hit_time: skill.hit_time,
                reuse_delay: skill.reuse_delay,
                creature: position(c),
                ground_location: None,
                target: position(t),
            },
        );
    }

    fn board_heal(&mut self, id: ObjectId, command: &str) -> Option<String> {
        let page = command.replace("_bbsheal;", "");
        let price = i64::from(self.custom_buffers.board.heal_price);
        if self.board_currency_count(id) < price {
            self.send_message(id, "Not enough currency!");
        } else {
            self.board_pay(id, price);
            self.board_full_vitals(id);
            if let Some(pet) = self.summon_of(id) {
                self.board_full_vitals(pet);
            }
            self.send_user_info(id);
            self.send_message(id, "You used heal!");
        }
        self.board_custom_page(id, &page)
    }

    /// `setCurrentHp(getMaxHp())`, `setCurrentMp(getMaxMp())`, `setCurrentCp(getMaxCp())`.
    fn board_full_vitals(&mut self, id: ObjectId) {
        let Some((hp, mp, cp)) = self.stats(id).map(|v| {
            (
                f64::from(v.max_hp()),
                f64::from(v.max_mp()),
                f64::from(v.max_cp()),
            )
        }) else {
            return;
        };
        self.set_current_hp(id, hp, true);
        self.set_current_mp(id, mp, true);
        self.set_current_cp(id, cp, true);
    }

    fn board_delevel(&mut self, id: ObjectId) -> Option<String> {
        let price = i64::from(self.custom_buffers.board.delevel_price);
        if self.board_currency_count(id) < price {
            self.send_message(id, "Not enough currency!");
            return None;
        }
        let level = self.players.get(&id)?.core.level;
        if level == 1 {
            self.send_message(id, "You are at minimum level!");
            return None;
        }
        self.board_pay(id, price);
        let new_level = level - 1;
        let exp = self.exp_for_level(new_level)?;
        self.set_exp(id, exp);
        self.set_level(id, new_level);
        // The maximum vitals follow the level (Java's stats watch in `getMaxHp()`).
        self.refresh_max_vitals(id, false);
        if let Some((hp, mp)) = self
            .stats(id)
            .map(|v| (f64::from(v.max_hp()), f64::from(v.max_mp())))
        {
            self.set_current_hp_mp(id, hp, mp);
        }
        if let Some(cp) = self.stats(id).map(|v| f64::from(v.max_cp())) {
            self.set_current_cp(id, cp, true);
        }
        self.broadcast_user_info(id);
        // Adjust skills according to the new level.
        self.check_player_skills_and_store(id);
        self.board_custom_page(id, "delevel/complete")
    }

    /// The tasks of [`CustomBoardTimer`].
    pub(crate) fn on_custom_board_timer(&mut self, timer: CustomBoardTimer) {
        match timer {
            CustomBoardTimer::Multisell {
                player,
                list_id,
                inventory_only,
            } => {
                if self.players.contains_key(&player) {
                    self.multisell_separate_and_send(list_id, player, None, inventory_only, 1.0, 1.0);
                }
            }
            CustomBoardTimer::SellList { player } => {
                // `new BuyList(BuyListData.getInstance().getBuyList(423), player.getAdena(), 0)`.
                if let Some(list) = self.buy_list_packet(player, 423, 0.0) {
                    self.send(player, &list);
                }
                if let Some(list) = self.ex_buy_sell_list_packet(player, false) {
                    self.send(player, &list);
                }
            }
            CustomBoardTimer::EnableSkills { player } => self.enable_all_skills(player),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_split_drops_trailing_empty_parts() {
        assert_eq!(java_split("a;b;", ';'), ["a", "b"]);
        assert_eq!(java_split("a;;b", ';'), ["a", "", "b"]);
        assert_eq!(java_split("", ';'), [""]);
        assert!(java_split(";;", ';').is_empty());
        assert_eq!(java_split("one", ';'), ["one"]);
    }

    #[test]
    fn the_lists_of_the_ini() {
        let buffs = CustomBoardSettings::parse_buffs("1077,1240,1086");
        assert_eq!(buffs.into_iter().collect::<Vec<_>>(), [1077, 1086, 1240]);
        let teleports =
            CustomBoardSettings::parse_teleports("Giran,82698,148638,-3473;Aden,147450,27064,-2208;");
        assert_eq!(teleports.get("Giran"), Some(&(82698, 148638, -3473)));
        assert_eq!(teleports.get("Aden"), Some(&(147450, 27064, -2208)));
        assert_eq!(teleports.len(), 2);
    }

    #[test]
    fn the_defaults_are_javas() {
        let s = CustomBoardSettings::default();
        assert_eq!(s.currency, 57);
        assert!(s.enable_multisells && s.enable_teleports && s.enable_buffs && s.enable_heal);
        assert!(!s.enable_delevel);
        assert!(s.combat_disabled && !s.cast_animations);
    }
}
