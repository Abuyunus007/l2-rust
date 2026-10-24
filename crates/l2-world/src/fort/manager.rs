//! The fortress foreman (Java `FortManager` the NPC): expelling outsiders, the report, the doors,
//! the clan vault, the fortress functions and their management.
// Java: entity/actor/instance/FortManager.java
// Java: mechanics/siege/Fort.java (updateFunctions, getFunction)
//
// The pages are the datapack's `data/html/fortress/*.htm`; the numbers come from `Feature.ini`
// (`Fortress*FunctionFee*`). Every Java call that can throw (`Integer.parseInt` of a token, the
// token that is not there) ends the command here as well: nothing more happens and a warning
// says what the command was.

use l2_protocol_game::server::ActionFailed;
use l2_util::ObjectId;
use tracing::warn;

use crate::clan::model::ClanAccess;
use crate::residence::{
    CASTLE_FUNC_RESTORE_EXP, CASTLE_FUNC_RESTORE_HP, CASTLE_FUNC_RESTORE_MP, CASTLE_FUNC_SUPPORT,
    CASTLE_FUNC_TELEPORT, FortId, ResidenceRef,
};
use crate::warehouse::{A2Z, PendingClanWindow, WarehouseListType, get_order};
use crate::world::World;

/// `FortManager.COND_ALL_FALSE`.
const COND_ALL_FALSE: i32 = 0;
/// `FortManager.COND_BUSY_BECAUSE_OF_SIEGE`.
const COND_BUSY_BECAUSE_OF_SIEGE: i32 = 1;
/// `FortManager.COND_OWNER`.
const COND_OWNER: i32 = 2;

/// Java `StringTokenizer(command, " ")`.
pub(super) struct Words<'a> {
    words: Vec<&'a str>,
    pos: usize,
}

impl<'a> Words<'a> {
    pub(super) fn new(command: &'a str) -> Self {
        Self {
            words: command.split(' ').filter(|w| !w.is_empty()).collect(),
            pos: 0,
        }
    }

    /// `nextToken()`: `None` where Java throws `NoSuchElementException`.
    pub(super) fn next(&mut self) -> Option<&'a str> {
        let word = self.words.get(self.pos).copied();
        self.pos += usize::from(word.is_some());
        word
    }

    /// `countTokens()`.
    pub(super) fn count(&self) -> usize {
        self.words.len() - self.pos
    }
}

/// `Integer.parseInt(text)`.
pub(super) fn parse_int(text: &str) -> Option<i32> {
    text.parse::<i32>().ok()
}

/// `dd/MM/yyyy HH:mm` of an epoch time in the server's zone (`ofPattern("dd/MM/yyyy HH:mm")`).
pub(super) fn format_date(epoch_ms: i64) -> String {
    let local = epoch_ms.saturating_add(l2_util::time::local_offset_millis(epoch_ms));
    let seconds = local.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{day:02}/{month:02}/{year:04} {:02}:{:02}",
        rest / 3600,
        (rest % 3600) / 60
    )
}

impl World {
    /// Sends a page of `data/html/fortress/` (or another) with the NPC's ids replaced, after the
    /// replacements of the caller (Java `html.replace(...)` then `sendHtmlMessage`).
    pub(super) fn fort_page(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        path: &str,
        replace: &[(&str, String)],
    ) {
        let Some(mut message) = self.html_from_file(player, npc.0, path) else {
            return;
        };
        for (from, to) in replace {
            message.html = message.html.replace(from, to);
        }
        message.html = message.html.replace("%objectId%", &npc.0.to_string());
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        message.html = message.html.replace("%npcId%", &npc_id.to_string());
        self.send_html(player, &message);
    }

    fn fort_npc_name(&self, npc: ObjectId) -> String {
        self.npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .map(|t| t.name.clone())
            .unwrap_or_default()
    }

    /// Java `validateCondition(player)`.
    fn fort_manager_condition(&self, npc: ObjectId, player: ObjectId) -> i32 {
        if let Some(fort) = self.npc_fort(npc)
            && fort.0 > 0
            && self.clan_of(player).is_some()
        {
            if self.fort_zone_is_active(fort) {
                // Busy because of siege.
                return COND_BUSY_BECAUSE_OF_SIEGE;
            }
            let clan = self.clan_id_of(player);
            if self
                .fort(fort)
                .is_some_and(|f| f.owner_id != 0 && f.owner_id == clan)
            {
                // Owner.
                return COND_OWNER;
            }
        }
        COND_ALL_FALSE
    }

    /// Java `FortManager.showChatWindow(player)`.
    pub(super) fn fort_manager_show_chat_window(&mut self, npc: ObjectId, player: ObjectId) {
        self.send(player, &ActionFailed);
        let mut filename = "data/html/fortress/foreman-no.htm";
        let condition = self.fort_manager_condition(npc, player);
        if condition > COND_ALL_FALSE {
            if condition == COND_BUSY_BECAUSE_OF_SIEGE {
                // Busy because of siege.
                filename = "data/html/fortress/foreman-busy.htm";
            } else if condition == COND_OWNER {
                // Owner message window.
                filename = "data/html/fortress/foreman.htm";
            }
        }
        let name = self.fort_npc_name(npc);
        self.fort_page(npc, player, filename, &[("%npcname%", name)]);
    }

    /// Java `FortManager.onBypassFeedback(player, command)`.
    pub(crate) fn fort_manager_bypass_feedback(&mut self, npc: ObjectId, player: ObjectId, command: &str) {
        // BypassValidation Exploit plug.
        if self.players.get(&player).and_then(|p| p.talk.last_folk_npc) != Some(npc) {
            return;
        }
        let condition = self.fort_manager_condition(npc, player);
        if condition <= COND_ALL_FALSE || condition == COND_BUSY_BECAUSE_OF_SIEGE {
            return;
        }
        if self.fort_manager_owner_command(npc, player, command).is_none() {
            warn!("Exception processing bypass from {player}: {command} (fortress foreman)");
        }
    }

    /// The `COND_OWNER` branch. `None` where Java's code throws.
    fn fort_manager_owner_command(&mut self, npc: ObjectId, player: ObjectId, command: &str) -> Option<()> {
        let fort = self.npc_fort(npc)?;
        let residence = ResidenceRef::Fort(fort);
        let mut st = Words::new(command);
        // Get actual command.
        let actual = st.next()?;
        let val = if st.count() >= 1 { st.next()? } else { "" };
        let lower = actual.to_ascii_lowercase();

        if lower == "expel" {
            let page = if self.has_clan_access(player, ClanAccess::CastleBanish) {
                "data/html/fortress/foreman-expel.htm"
            } else {
                "data/html/fortress/foreman-noprivs.htm"
            };
            self.fort_page(npc, player, page, &[]);
            return Some(());
        }
        if lower == "banish_foreigner" {
            if self.has_clan_access(player, ClanAccess::CastleBanish) {
                // Move non-clan members off fortress area.
                self.residence_banish_foreigners(residence);
                self.fort_page(npc, player, "data/html/fortress/foreman-expeled.htm", &[]);
            } else {
                self.fort_page(npc, player, "data/html/fortress/foreman-noprivs.htm", &[]);
            }
            return Some(());
        }
        if lower == "receive_report" {
            self.fort_manager_report(npc, player, fort);
            return Some(());
        }
        if lower == "operate_door" {
            // door control
            if self.has_clan_access(player, ClanAccess::CastleOpenDoor) {
                if !val.is_empty() {
                    let open = parse_int(val)? == 1;
                    while st.count() >= 1 {
                        let door = parse_int(st.next()?)?;
                        self.residence_open_close_door(player, residence, door, open);
                    }
                    let page = if open {
                        "data/html/fortress/foreman-opened.htm"
                    } else {
                        "data/html/fortress/foreman-closed.htm"
                    };
                    self.fort_page(npc, player, page, &[]);
                } else {
                    let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
                    let name = self.fort_npc_name(npc);
                    self.fort_page(
                        npc,
                        player,
                        &format!("data/html/fortress/{npc_id}-d.htm"),
                        &[("%npcname%", name)],
                    );
                }
            } else {
                self.fort_page(npc, player, "data/html/fortress/foreman-noprivs.htm", &[]);
            }
            return Some(());
        }
        if lower == "manage_vault" {
            self.fort_manager_vault(npc, player, val);
            return Some(());
        }
        if actual.starts_with("WithdrawSortedC") {
            // `command.split("_")`: "WithdrawSortedC_<type>_<order>".
            let param: Vec<&str> = command.split('_').collect();
            let sorted = match param.len() {
                n if n > 2 => (WarehouseListType::from_name(param[1])?, get_order(param[2])),
                2 => (WarehouseListType::from_name(param[1])?, A2Z),
                _ => (WarehouseListType::All, A2Z),
            };
            self.fort_show_vault_withdraw(player, Some(sorted));
            return Some(());
        }
        if lower == "functions" {
            self.fort_manager_functions(npc, player, fort, val);
            return Some(());
        }
        if lower == "manage" {
            return self.fort_manager_manage(npc, player, fort, &mut st, val);
        }
        if lower == "support" {
            return self.fort_manager_support(npc, player, fort, &mut st, val);
        }
        if lower == "support_back" {
            let level = self.residence_function(residence, CASTLE_FUNC_SUPPORT)?.lvl;
            if level == 0 {
                return Some(());
            }
            let mp = self.npcs.get(&npc).map_or(0.0, |n| n.core.cur_mp) as i32;
            self.fort_page(
                npc,
                player,
                &format!("data/html/fortress/support{level}.htm"),
                &[("%mp%", mp.to_string())],
            );
            return Some(());
        }
        if lower == "goto" {
            // goto listId locId
            if self.residence_function(residence, CASTLE_FUNC_TELEPORT).is_none() {
                return Some(());
            }
            let list_name = command.split(' ').nth(1)?;
            let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
            let teleporters = self.talk.teleporters.clone();
            let Some(holder) = teleporters.as_ref().and_then(|d| d.get_holder(npc_id, list_name)) else {
                warn!("{player} requested unknown teleport list: {list_name} for npc: {npc_id}!");
                return Some(());
            };
            // `StringUtil.parseNextInt(st, -1)`: the token after the list name.
            let location = st.next().and_then(parse_int).unwrap_or(-1);
            self.do_teleport(npc, player, holder, location);
            return Some(());
        }
        // `super.onBypassFeedback(player, command)`.
        self.generic_npc_bypass_feedback(npc, player, command);
        Some(())
    }

    /// `receive_report`.
    fn fort_manager_report(&mut self, npc: ObjectId, player: ObjectId, fort: FortId) {
        let state = self.fort(fort).map_or(0, |f| f.state);
        let max_own = self.fort.settings.fs_max_own_time;
        // `hour = t / 3600; minutes = (t - hour * 3600) / 60`.
        let (hour, minutes) = {
            let t = if max_own > 0 {
                self.fort_time_till_rebel_army(fort)
            } else {
                self.fort(fort)
                    .map_or(0, |f| f.owned_time(crate::world::unix_ms_now()))
            };
            let hour = t / 3600;
            (hour, (t - hour * 3600) / 60)
        };
        if state < 2 {
            self.fort_page(
                npc,
                player,
                "data/html/fortress/foreman-report.htm",
                &[("%hr%", hour.to_string()), ("%min%", minutes.to_string())],
            );
        } else {
            let t2 = self.fort_time_till_next_update(fort);
            let hour2 = (t2 / 3600) as i32;
            let minutes2 = ((t2 - i64::from(hour2) * 3600) / 60) as i32;
            // NOTE(java-quirk): `getContractedCastle().getName()` fails for a fortress without
            // its castle; the page is not sent then.
            let castle = self
                .fort_contracted_castle(fort)
                .map(|c| self.residence_name(ResidenceRef::Castle(c)));
            let Some(castle) = castle else {
                return;
            };
            self.fort_page(
                npc,
                player,
                "data/html/fortress/foreman-castlereport.htm",
                &[
                    ("%hr%", hour.to_string()),
                    ("%min%", minutes.to_string()),
                    ("%castle%", castle),
                    ("%hr2%", hour2.to_string()),
                    ("%min2%", minutes2.to_string()),
                ],
            );
        }
    }

    // ------------------------------------------------------------------ the clan vault

    fn fort_manager_vault(&mut self, npc: ObjectId, player: ObjectId, val: &str) {
        if !self.has_clan_access(player, ClanAccess::AccessWarehouse) {
            self.fort_page(npc, player, "data/html/fortress/foreman-noprivs.htm", &[]);
            return;
        }
        if val.eq_ignore_ascii_case("deposit") {
            self.fort_show_vault_deposit(player);
        } else if val.eq_ignore_ascii_case("withdraw") {
            if self.settings.warehouse.sorting_clan {
                let file = "data/html/mods/WhSortedC.htm";
                match self.get_htm(player, file) {
                    Some(content) => {
                        let content = content.replace("%objectId%", &npc.0.to_string());
                        if let Some(message) = l2_protocol_game::server::NpcHtmlMessage::new(npc.0, &content)
                        {
                            self.send_html(player, &message);
                        }
                    }
                    None => warn!("Missing htm: {file} !"),
                }
            } else {
                self.fort_show_vault_withdraw(player, None);
            }
        } else {
            self.fort_page(npc, player, "data/html/fortress/foreman-vault.htm", &[]);
        }
    }

    /// Java `showVaultWindowDeposit(player)`. The rows of the clan warehouse are read first
    /// (M6-SYNC-01, D-50), like the window of every other clan warehouse.
    fn fort_show_vault_deposit(&mut self, player: ObjectId) {
        self.send(player, &ActionFailed);
        let clan = self.clan_id_of(player);
        if self.clan(clan).is_some() {
            self.request_clan_window(player, clan, PendingClanWindow::Deposit);
        }
    }

    /// Java `showVaultWindowWithdraw(player, itemtype, sortorder)`.
    fn fort_show_vault_withdraw(&mut self, player: ObjectId, sorted: Option<(WarehouseListType, i8)>) {
        if self.is_clan_leader(player) || self.has_clan_access(player, ClanAccess::AccessWarehouse) {
            self.send(player, &ActionFailed);
            let clan = self.clan_id_of(player);
            if self.clan(clan).is_some() {
                self.request_clan_window(player, clan, PendingClanWindow::Withdraw(sorted));
            }
        } else {
            // Not reachable from the manager (the command checked the privilege), but Java has it.
            let npc = self.players.get(&player).and_then(|p| p.talk.last_folk_npc);
            if let Some(npc) = npc {
                self.fort_page(npc, player, "data/html/fortress/foreman-noprivs.htm", &[]);
            }
        }
    }

    // ------------------------------------------------------------------ functions

    fn fort_function_level(&self, fort: FortId, function: i32) -> Option<i32> {
        self.residence_function(ResidenceRef::Fort(fort), function)
            .map(|f| f.lvl)
    }

    /// `functions`: tele, support, back, or the list.
    fn fort_manager_functions(&mut self, npc: ObjectId, player: ObjectId, fort: FortId, val: &str) {
        if val.eq_ignore_ascii_case("tele") {
            match self.fort_function_level(fort, CASTLE_FUNC_TELEPORT) {
                None => self.fort_page(npc, player, "data/html/fortress/foreman-nac.htm", &[]),
                Some(level) if level > 0 => {
                    let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
                    let teleporters = self.talk.teleporters.clone();
                    if let Some(holder) = teleporters
                        .as_ref()
                        .and_then(|d| d.get_holder(npc_id, &format!("tel{level}")))
                    {
                        let bypass = format!("npc_{}_goto", npc.0);
                        self.show_teleport_list_with(npc, player, holder, &bypass);
                    }
                }
                Some(_) => {}
            }
        } else if val.eq_ignore_ascii_case("support") {
            match self.fort_function_level(fort, CASTLE_FUNC_SUPPORT) {
                None => self.fort_page(npc, player, "data/html/fortress/foreman-nac.htm", &[]),
                Some(level) => {
                    let mp = self.npcs.get(&npc).map_or(0.0, |n| n.core.cur_mp) as i32;
                    self.fort_page(
                        npc,
                        player,
                        &format!("data/html/fortress/support{level}.htm"),
                        &[("%mp%", mp.to_string())],
                    );
                }
            }
        } else if val.eq_ignore_ascii_case("back") {
            self.fort_manager_show_chat_window(npc, player);
        } else {
            let level = |world: &World, function| world.fort_function_level(fort, function).unwrap_or(0);
            let replace = [
                ("%xp_regen%", level(self, CASTLE_FUNC_RESTORE_EXP).to_string()),
                ("%hp_regen%", level(self, CASTLE_FUNC_RESTORE_HP).to_string()),
                ("%mp_regen%", level(self, CASTLE_FUNC_RESTORE_MP).to_string()),
            ];
            self.fort_page(npc, player, "data/html/fortress/foreman-functions.htm", &replace);
        }
    }

    /// `support <skillId> [level]`: the foreman casts a support spell on the player.
    fn fort_manager_support(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        fort: FortId,
        st: &mut Words<'_>,
        val: &str,
    ) -> Option<()> {
        self.set_creature_target(npc, Some(player));
        if val.is_empty() {
            return Some(());
        }
        let Some(skill_id) = parse_int(val) else {
            self.send_message(player, "Invalid skill level, contact your admin!");
            return Some(());
        };
        let residence = ResidenceRef::Fort(fort);
        let Some(function) = self.residence_function(residence, CASTLE_FUNC_SUPPORT) else {
            return Some(());
        };
        if function.lvl == 0 {
            return Some(());
        }
        let skill_level = if st.count() >= 1 {
            match parse_int(st.next()?) {
                Some(level) => level,
                None => {
                    self.send_message(player, "Invalid skill level, contact your admin!");
                    return Some(());
                }
            }
        } else {
            0
        };
        let table = self.skill_table()?;
        let Some(skill) = table.get(skill_id, skill_level) else {
            // `skill.hasEffectType` on null.
            self.send_message(player, "Invalid skill level, contact your admin!");
            return Some(());
        };
        let key = crate::effects::SkillKey::new(skill_id, skill_level);
        let current_mp = self.npcs.get(&npc).map_or(0.0, |n| n.core.cur_mp);
        let mp = current_mp as i32;
        if self.skill_has_effect_type(skill, &[crate::effects::EffectType::Summon]) {
            self.do_cast(player, key);
        } else if f64::from(skill.mp_consume + skill.mp_initial_consume) <= current_mp {
            self.do_cast(npc, key);
        } else {
            self.fort_page(
                npc,
                player,
                "data/html/fortress/support-no_mana.htm",
                &[("%mp%", mp.to_string())],
            );
            return Some(());
        }
        let mp = self.npcs.get(&npc).map_or(0.0, |n| n.core.cur_mp) as i32;
        self.fort_page(
            npc,
            player,
            "data/html/fortress/support-done.htm",
            &[("%mp%", mp.to_string())],
        );
        Some(())
    }

    // ------------------------------------------------------------------ manage

    fn fort_fee_days(ratio: i64) -> i64 {
        ratio / 1000 / 60 / 60 / 24
    }

    /// The fee of a function level, and the ratio.
    fn fort_function_fee(&self, function: i32, level: i32) -> (i32, i64) {
        let s = &self.fort.settings;
        match function {
            CASTLE_FUNC_RESTORE_HP => (
                if level == 300 {
                    s.fs_hpreg1_fee
                } else {
                    s.fs_hpreg2_fee
                },
                s.fs_hpreg_fee_ratio,
            ),
            CASTLE_FUNC_RESTORE_MP => (
                if level == 40 {
                    s.fs_mpreg1_fee
                } else {
                    s.fs_mpreg2_fee
                },
                s.fs_mpreg_fee_ratio,
            ),
            CASTLE_FUNC_RESTORE_EXP => (
                if level == 45 {
                    s.fs_expreg1_fee
                } else {
                    s.fs_expreg2_fee
                },
                s.fs_expreg_fee_ratio,
            ),
            CASTLE_FUNC_TELEPORT => (
                if level == 1 {
                    s.fs_tele1_fee
                } else {
                    s.fs_tele2_fee
                },
                s.fs_tele_fee_ratio,
            ),
            _ => (
                if level == 1 {
                    s.fs_support1_fee
                } else {
                    s.fs_support2_fee
                },
                s.fs_support_fee_ratio,
            ),
        }
    }

    fn fort_manager_manage(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        fort: FortId,
        st: &mut Words<'_>,
        val: &str,
    ) -> Option<()> {
        if !self.has_clan_access(player, ClanAccess::CastleManageFunctions) {
            self.fort_page(npc, player, "data/html/fortress/foreman-noprivs.htm", &[]);
            return Some(());
        }
        if val.eq_ignore_ascii_case("recovery") {
            if st.count() >= 1 {
                if self.fort(fort).is_none_or(|f| f.owner_id == 0) {
                    self.send_message(
                        player,
                        "This fortress has no owner, you cannot change the configuration.",
                    );
                    return Some(());
                }
                let val = st.next()?;
                if self.fort_manage_recovery_step(npc, player, fort, st, val)? {
                    return Some(());
                }
            }
            self.fort_page_edit_recovery(npc, player, fort);
        } else if val.eq_ignore_ascii_case("other") {
            if st.count() >= 1 {
                if self.fort(fort).is_none_or(|f| f.owner_id == 0) {
                    self.send_message(
                        player,
                        "This fortress has no owner, you cannot change the configuration.",
                    );
                    return Some(());
                }
                let val = st.next()?;
                if self.fort_manage_other_step(npc, player, fort, st, val)? {
                    return Some(());
                }
            }
            self.fort_page_edit_other(npc, player, fort);
        } else if val.eq_ignore_ascii_case("back") {
            self.fort_manager_show_chat_window(npc, player);
        } else {
            self.fort_page(npc, player, "data/html/fortress/manage.htm", &[]);
        }
        Some(())
    }

    /// The recovery steps. `Some(true)`: the command is answered, `Some(false)`: show the
    /// recovery page.
    fn fort_manage_recovery_step(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        fort: FortId,
        st: &mut Words<'_>,
        val: &str,
    ) -> Option<bool> {
        // (word, function, "name", "use text", "apply word", the levels)
        let cancel = |world: &mut World, apply: &str| {
            world.fort_page(
                npc,
                player,
                "data/html/fortress/functions-cancel.htm",
                &[("%apply%", apply.to_owned())],
            );
        };
        match val.to_ascii_lowercase().as_str() {
            "hp_cancel" => cancel(self, "recovery hp 0"),
            "mp_cancel" => cancel(self, "recovery mp 0"),
            "exp_cancel" => cancel(self, "recovery exp 0"),
            "edit_hp" | "edit_mp" | "edit_exp" => {
                let (function, name, text, apply) = match val.to_ascii_lowercase().as_str() {
                    "edit_hp" => (
                        CASTLE_FUNC_RESTORE_HP,
                        "(HP Recovery Device)",
                        "Provides additional HP recovery for clan members in the fortress.",
                        "recovery hp ",
                    ),
                    "edit_mp" => (
                        CASTLE_FUNC_RESTORE_MP,
                        "(MP Recovery)",
                        "Provides additional MP recovery for clan members in the fortress.",
                        "recovery mp ",
                    ),
                    _ => (
                        CASTLE_FUNC_RESTORE_EXP,
                        "(EXP Recovery Device)",
                        "Restores the Exp of any clan member who is resurrected in the fortress.",
                        "recovery exp ",
                    ),
                };
                let percent = parse_int(st.next()?)?;
                let (cost, ratio) = self.fort_function_fee(function, percent);
                self.fort_page(
                    npc,
                    player,
                    "data/html/fortress/functions-apply.htm",
                    &[
                        ("%name%", name.to_owned()),
                        (
                            "%cost%",
                            format!("{cost}</font>Adena /{} Day</font>)", Self::fort_fee_days(ratio)),
                        ),
                        ("%use%", format!("{text}<font color=\"00FFFF\">{percent}%</font>")),
                        ("%apply%", format!("{apply}{percent}")),
                    ],
                );
            }
            "hp" | "mp" | "exp" => {
                let function = match val.to_ascii_lowercase().as_str() {
                    "hp" => CASTLE_FUNC_RESTORE_HP,
                    "mp" => CASTLE_FUNC_RESTORE_MP,
                    _ => CASTLE_FUNC_RESTORE_EXP,
                };
                if st.count() >= 1 {
                    let val = st.next()?;
                    self.fort_apply_function(npc, player, fort, function, val, true)?;
                }
            }
            _ => return Some(false),
        }
        Some(true)
    }

    /// The "other" steps (teleport and support).
    fn fort_manage_other_step(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        fort: FortId,
        st: &mut Words<'_>,
        val: &str,
    ) -> Option<bool> {
        match val.to_ascii_lowercase().as_str() {
            "tele_cancel" => self.fort_page(
                npc,
                player,
                "data/html/fortress/functions-cancel.htm",
                &[("%apply%", "other tele 0".to_owned())],
            ),
            "support_cancel" => self.fort_page(
                npc,
                player,
                "data/html/fortress/functions-cancel.htm",
                &[("%apply%", "other support 0".to_owned())],
            ),
            "edit_support" | "edit_tele" => {
                let support = val.eq_ignore_ascii_case("edit_support");
                let stage = parse_int(st.next()?)?;
                let function = if support {
                    CASTLE_FUNC_SUPPORT
                } else {
                    CASTLE_FUNC_TELEPORT
                };
                let (cost, ratio) = self.fort_function_fee(function, stage);
                let (name, text, apply) = if support {
                    (
                        "Insignia (Supplementary Magic)",
                        "Enables the use of supplementary magic.".to_owned(),
                        format!("other support {stage}"),
                    )
                } else {
                    (
                        "Mirror (Teleportation Device)",
                        format!(
                            "Teleports clan members in a fort to the target <font color=\"00FFFF\">Stage {stage}</font> staging area"
                        ),
                        format!("other tele {stage}"),
                    )
                };
                self.fort_page(
                    npc,
                    player,
                    "data/html/fortress/functions-apply.htm",
                    &[
                        ("%name%", name.to_owned()),
                        (
                            "%cost%",
                            format!("{cost}</font>Adena /{} Day</font>)", Self::fort_fee_days(ratio)),
                        ),
                        ("%use%", text),
                        ("%apply%", apply),
                    ],
                );
            }
            "tele" | "support" => {
                let function = if val.eq_ignore_ascii_case("tele") {
                    CASTLE_FUNC_TELEPORT
                } else {
                    CASTLE_FUNC_SUPPORT
                };
                if st.count() >= 1 {
                    let val = st.next()?;
                    // The support branch sends the page once; the others after a failed
                    // purchase send "low_adena" and then the same page again (the quirk).
                    self.fort_apply_function(
                        npc,
                        player,
                        fort,
                        function,
                        val,
                        function != CASTLE_FUNC_SUPPORT,
                    )?;
                }
            }
            _ => return Some(false),
        }
        Some(true)
    }

    /// The common tail of a function purchase: Java's
    /// `getFort().updateFunctions(player, type, level, fee, ratio, getFunction(type) == null)`.
    /// `double_send` is the `sendHtmlMessage(player, html)` after a failed purchase that falls
    /// through to the one at the end of the branch.
    // NOTE(java-quirk): the HP, MP, EXP and teleport branches send the page twice when the player
    // cannot pay: "low_adena" in the failure branch, and the same page object again at the end.
    fn fort_apply_function(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        fort: FortId,
        function: i32,
        val: &str,
        double_send: bool,
    ) -> Option<()> {
        let residence = ResidenceRef::Fort(fort);
        let is_recovery = matches!(
            function,
            CASTLE_FUNC_RESTORE_HP | CASTLE_FUNC_RESTORE_MP | CASTLE_FUNC_RESTORE_EXP
        );
        let level = parse_int(val)?;
        // The page for "already used".
        if self.fort_function_level(fort, function) == Some(level) {
            let shown = if is_recovery {
                format!("{val}%")
            } else {
                format!("Stage {val}")
            };
            self.fort_page(
                npc,
                player,
                "data/html/fortress/functions-used.htm",
                &[("%val%", shown)],
            );
            return Some(());
        }
        let (fee, ratio) = if level == 0 {
            (0, self.fort_function_fee(function, level).1)
        } else {
            self.fort_function_fee(function, level)
        };
        let mut page = if level == 0 {
            "data/html/fortress/functions-cancel_confirmed.htm"
        } else {
            "data/html/fortress/functions-apply_confirmed.htm"
        };
        let add_new = self.residence_function(residence, function).is_none();
        if !self.residence_update_functions(player, residence, function, level, fee, ratio, add_new) {
            page = "data/html/fortress/low_adena.htm";
            self.fort_page(npc, player, page, &[]);
            if !double_send {
                return Some(());
            }
        }
        self.fort_page(npc, player, page, &[]);
        Some(())
    }

    fn fort_function_line(
        &self,
        fort: FortId,
        function: i32,
        ratio: i64,
        with_percent: bool,
    ) -> Option<(String, String)> {
        let f = self.residence_function(ResidenceRef::Fort(fort), function)?;
        let level = if with_percent {
            format!("{}%", f.lvl)
        } else {
            format!("Stage {}", f.lvl)
        };
        Some((
            format!(
                "{level}</font> (<font color=\"FFAABB\">{}</font>Adena /{} Day)",
                f.lease,
                Self::fort_fee_days(ratio)
            ),
            format!(
                "Withdraw the fee for the next time at {}",
                format_date(f.end_time)
            ),
        ))
    }

    fn fort_page_edit_recovery(&mut self, npc: ObjectId, player: ObjectId, fort: FortId) {
        let s = self.fort.settings.clone();
        let hp = "[<a action=\"bypass -h npc_%objectId%_manage recovery edit_hp 300\">300%</a>][<a action=\"bypass -h npc_%objectId%_manage recovery edit_hp 400\">400%</a>]";
        let exp = "[<a action=\"bypass -h npc_%objectId%_manage recovery edit_exp 45\">45%</a>][<a action=\"bypass -h npc_%objectId%_manage recovery edit_exp 50\">50%</a>]";
        let mp = "[<a action=\"bypass -h npc_%objectId%_manage recovery edit_mp 40\">40%</a>][<a action=\"bypass -h npc_%objectId%_manage recovery edit_mp 50\">50%</a>]";
        let mut replace: Vec<(&str, String)> = Vec::new();
        for (function, ratio, key, deactivate, edit) in [
            (
                CASTLE_FUNC_RESTORE_HP,
                s.fs_hpreg_fee_ratio,
                "hp",
                "hp_cancel",
                hp,
            ),
            (
                CASTLE_FUNC_RESTORE_EXP,
                s.fs_expreg_fee_ratio,
                "exp",
                "exp_cancel",
                exp,
            ),
            (
                CASTLE_FUNC_RESTORE_MP,
                s.fs_mpreg_fee_ratio,
                "mp",
                "mp_cancel",
                mp,
            ),
        ] {
            let (recovery, period, change) = match self.fort_function_line(fort, function, ratio, true) {
                Some((line, period)) => (
                    line,
                    period,
                    format!(
                        "[<a action=\"bypass -h npc_%objectId%_manage recovery {deactivate}\">Deactivate</a>]{edit}"
                    ),
                ),
                None => ("none".to_owned(), "none".to_owned(), edit.to_owned()),
            };
            let (r, p, c) = match key {
                "hp" => ("%hp_recovery%", "%hp_period%", "%change_hp%"),
                "exp" => ("%exp_recovery%", "%exp_period%", "%change_exp%"),
                _ => ("%mp_recovery%", "%mp_period%", "%change_mp%"),
            };
            replace.push((r, recovery));
            replace.push((p, period));
            replace.push((c, change));
        }
        self.fort_page(npc, player, "data/html/fortress/edit_recovery.htm", &replace);
    }

    fn fort_page_edit_other(&mut self, npc: ObjectId, player: ObjectId, fort: FortId) {
        let s = self.fort.settings.clone();
        let tele = "[<a action=\"bypass -h npc_%objectId%_manage other edit_tele 1\">Level 1</a>][<a action=\"bypass -h npc_%objectId%_manage other edit_tele 2\">Level 2</a>]";
        let support = "[<a action=\"bypass -h npc_%objectId%_manage other edit_support 1\">Level 1</a>][<a action=\"bypass -h npc_%objectId%_manage other edit_support 2\">Level 2</a>]";
        let mut replace: Vec<(&str, String)> = Vec::new();
        for (function, ratio, deactivate, edit, keys) in [
            (
                CASTLE_FUNC_TELEPORT,
                s.fs_tele_fee_ratio,
                "tele_cancel",
                tele,
                ("%tele%", "%tele_period%", "%change_tele%"),
            ),
            (
                CASTLE_FUNC_SUPPORT,
                s.fs_support_fee_ratio,
                "support_cancel",
                support,
                ("%support%", "%support_period%", "%change_support%"),
            ),
        ] {
            let (line, period, change) = match self.fort_function_line(fort, function, ratio, false) {
                Some((line, period)) => (
                    line,
                    period,
                    format!(
                        "[<a action=\"bypass -h npc_%objectId%_manage other {deactivate}\">Deactivate</a>]{edit}"
                    ),
                ),
                None => ("none".to_owned(), "none".to_owned(), edit.to_owned()),
            };
            replace.push((keys.0, line));
            replace.push((keys.1, period));
            replace.push((keys.2, change));
        }
        self.fort_page(npc, player, "data/html/fortress/edit_other.htm", &replace);
    }
}
