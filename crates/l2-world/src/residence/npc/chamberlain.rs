//! `CastleChamberlain`: the lord's steward (tax rate, treasury, doors and traps, functions,
//! the siege list, the manor link, the dawn tickets).
// Java: dist/game/data/scripts/ai/others/CastleChamberlain/CastleChamberlain.java and its pages
//
// The `onNpcManorBypass` listener of the chamberlain is `manor/npc.rs` (M7-MANOR-01); this is
// `onEvent` (`Script CastleChamberlain <event>`).

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use tracing::warn;

use super::seven_signs::{
    PERIOD_COMP_RECRUITING, PERIOD_COMP_RESULTS, PERIOD_COMPETITION, PERIOD_SEAL_VALIDATION, SEAL_AVARICE,
    SEAL_GNOSIS, SEAL_STRIFE,
};
use super::{
    CABAL_DAWN, CABAL_DUSK, CastleScript, Reply, ScriptFailed, civil_from_days, format_adena, parse_int,
    parse_long, words,
};
use crate::clan::model::ClanAccess;
use crate::effects::SkillKey;
use crate::residence::{
    CASTLE_FUNC_RESTORE_EXP, CASTLE_FUNC_RESTORE_HP, CASTLE_FUNC_RESTORE_MP, CASTLE_FUNC_SUPPORT,
    CASTLE_FUNC_TELEPORT, CastleId, FortId, ResidenceFunction, ResidenceRef,
};
use crate::world::World;

const SCRIPT: CastleScript = CastleScript::Chamberlain;

/// `CastleChamberlain.CROWN`: the Lord's Crown.
const CROWN: i32 = 6841;

/// `CastleChamberlain.FORTRESS`: the fortresses of each castle's domain.
fn domain_fortresses(castle: i32) -> Option<&'static [i32]> {
    Some(match castle {
        1 => &[101, 102, 112, 113],      // Gludio Castle
        2 => &[103, 112, 114, 115],      // Dion Castle
        3 => &[104, 114, 116, 118, 119], // Giran Castle
        4 => &[105, 113, 115, 116, 117], // Oren Castle
        5 => &[106, 107, 117, 118],      // Aden Castle
        6 => &[108, 119],                // Innadril Castle
        7 => &[109, 117, 120],           // Goddard Castle
        8 => &[110, 120, 121],           // Rune Castle
        9 => &[111, 121],                // Schuttgart Castle
        _ => return None,
    })
}

/// `CastleChamberlain.BUFFS`: skill id and level.
const BUFFS: [(i32, i32); 28] = [
    (4342, 2), // Wind Walk Lv.2
    (4343, 3), // Decrease Weight Lv.3
    (4344, 3), // Shield Lv.3
    (4346, 4), // Mental Shield Lv.4
    (4345, 3), // Might Lv.3
    (4347, 2), // Bless the Body Lv.2
    (4349, 1), // Magic Barrier Lv.1
    (4350, 1), // Resist Shock Lv.1
    (4348, 2), // Bless the Soul Lv.2
    (4351, 2), // Concentration Lv.2
    (4352, 1), // Berserker Spirit Lv.1
    (4353, 2), // Bless Shield Lv.2
    (4358, 1), // Guidance Lv.1
    (4354, 1), // Vampiric Rage Lv.1
    (4347, 6), // Bless the Body Lv.6
    (4349, 2), // Magic Barrier Lv.2
    (4350, 4), // Resist Shock Lv.4
    (4348, 6), // Bless the Soul Lv.6
    (4351, 6), // Concentration Lv.6
    (4352, 2), // Berserker Spirit Lv.2
    (4353, 6), // Bless Shield Lv.6
    (4358, 3), // Guidance Lv.3
    (4354, 4), // Vampiric Rage Lv.4
    (4355, 1), // Acumen Lv.1
    (4356, 1), // Empower Lv.1
    (4357, 1), // Haste Lv.1
    (4359, 1), // Focus Lv.1
    (4360, 1), // Death Whisper Lv.1
];

/// The pages `onEvent` answers with themselves (and which need no castle row).
const STATIC_PAGES: [&str; 5] = [
    "chamberlain-01.html",
    "manor-help-01.html",
    "manor-help-02.html",
    "manor-help-03.html",
    "manor-help-04.html",
];

/// Whether the event acts on the castle's row, so that the castle is read again first (D-50).
pub(super) fn needs_reread(event: &str) -> bool {
    !words(event).first().is_some_and(|w| STATIC_PAGES.contains(w))
}

/// `Reply` for a page.
fn page(name: &str) -> Reply {
    Ok(Some(name.to_owned()))
}

/// The seal's owner as the string id the pages show (`getSealOwner`).
fn seal_owner_string(owner: i32) -> &'static str {
    match owner {
        CABAL_DAWN => "1000511",
        CABAL_DUSK => "1000510",
        _ => "1000512",
    }
}

impl World {
    /// `isOwner(player, npc) && player.hasAccess(access)`.
    fn chamberlain_may(&self, player: ObjectId, npc: ObjectId, access: ClanAccess) -> bool {
        self.castle_script_is_owner(player, npc) && self.has_clan_access(player, access)
    }

    /// `getFunctionFee(func, level)`.
    fn chamberlain_function_fee(&self, func: i32, level: i32) -> i32 {
        let s = &self.residence_npc.settings;
        match func {
            CASTLE_FUNC_RESTORE_EXP => {
                if level == 45 {
                    s.cs_expreg1_fee
                } else {
                    s.cs_expreg2_fee
                }
            }
            CASTLE_FUNC_RESTORE_HP => {
                if level == 300 {
                    s.cs_hpreg1_fee
                } else {
                    s.cs_hpreg2_fee
                }
            }
            CASTLE_FUNC_RESTORE_MP => {
                if level == 40 {
                    s.cs_mpreg1_fee
                } else {
                    s.cs_mpreg2_fee
                }
            }
            CASTLE_FUNC_SUPPORT => {
                if level == 5 {
                    s.cs_support1_fee
                } else {
                    s.cs_support2_fee
                }
            }
            CASTLE_FUNC_TELEPORT => {
                if level == 1 {
                    s.cs_tele1_fee
                } else {
                    s.cs_tele2_fee
                }
            }
            _ => 0,
        }
    }

    /// `getFunctionRatio(func)`.
    fn chamberlain_function_ratio(&self, func: i32) -> i64 {
        let s = &self.residence_npc.settings;
        match func {
            CASTLE_FUNC_RESTORE_EXP => s.cs_expreg_fee_ratio,
            CASTLE_FUNC_RESTORE_HP => s.cs_hpreg_fee_ratio,
            CASTLE_FUNC_RESTORE_MP => s.cs_mpreg_fee_ratio,
            CASTLE_FUNC_SUPPORT => s.cs_support_fee_ratio,
            CASTLE_FUNC_TELEPORT => s.cs_tele_fee_ratio,
            _ => 0,
        }
    }

    /// The strife seal's change of a price: `price *= 3` of the Dusk, `price *= 0.8` of the Dawn
    /// (a compound assignment, which truncates to `int`).
    fn chamberlain_seal_price(&self, price: i32) -> i32 {
        match self.seven_signs_seal_owner(SEAL_STRIFE) {
            CABAL_DUSK => price.wrapping_mul(3),
            CABAL_DAWN => (f64::from(price) * 0.8) as i32,
            _ => price,
        }
    }

    /// `getDoorUpgradePrice(type, level)`.
    fn chamberlain_door_upgrade_price(&self, kind: i32, level: i32) -> i32 {
        let s = &self.residence_npc.settings;
        let price = match (kind, level) {
            (1, 2) => s.outer_door_upgrade_price2,
            (1, 3) => s.outer_door_upgrade_price3,
            (1, 5) => s.outer_door_upgrade_price5,
            (2, 2) => s.inner_door_upgrade_price2,
            (2, 3) => s.inner_door_upgrade_price3,
            (2, 5) => s.inner_door_upgrade_price5,
            (3, 2) => s.wall_upgrade_price2,
            (3, 3) => s.wall_upgrade_price3,
            (3, 5) => s.wall_upgrade_price5,
            _ => 0,
        };
        self.chamberlain_seal_price(price)
    }

    /// `getTrapUpgradePrice(level)`.
    fn chamberlain_trap_upgrade_price(&self, level: i32) -> i32 {
        let s = &self.residence_npc.settings;
        let price = match level {
            1 => s.trap_upgrade_price1,
            2 => s.trap_upgrade_price2,
            3 => s.trap_upgrade_price3,
            4 => s.trap_upgrade_price4,
            _ => 0,
        };
        self.chamberlain_seal_price(price)
    }

    /// `getTaxLimit()`.
    fn chamberlain_tax_limit(&self) -> i32 {
        match self.seven_signs_seal_owner(SEAL_STRIFE) {
            CABAL_DAWN => 25,
            CABAL_DUSK => 5,
            _ => 15,
        }
    }

    /// `isDomainFortressInContractStatus(castleId)`.
    fn chamberlain_domain_fortress_in_contract(&self, castle: CastleId) -> Result<bool, ScriptFailed> {
        let fortresses = domain_fortresses(castle.0).ok_or(ScriptFailed)?;
        let count = if castle.0 == 1 || castle.0 == 5 { 2 } else { 1 };
        for id in fortresses.iter().take(count) {
            let fort = self.fort(FortId(*id)).ok_or(ScriptFailed)?;
            if fort.state == 2 {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `funcReplace(castle, html, func, str)`: the four lines of one function on a page.
    fn chamberlain_func_replace(
        &self,
        castle: CastleId,
        func: i32,
        key: &str,
        replace: &mut Vec<(String, String)>,
    ) {
        let function: Option<ResidenceFunction> = self
            .residence_function(ResidenceRef::Castle(castle), func)
            .cloned();
        let mut put = |name: &str, value: String| replace.push((format!("%{key}{name}%"), value));
        match function {
            None => {
                put("Depth", "<fstring>4</fstring>".to_owned());
                put("Cost", String::new());
                put("Expire", "<fstring>4</fstring>".to_owned());
                put("Reset", String::new());
            }
            Some(function) => {
                let fstring = if func == CASTLE_FUNC_SUPPORT || func == CASTLE_FUNC_TELEPORT {
                    "9"
                } else {
                    "10"
                };
                // `Calendar.getInstance()` in the machine's zone (D-48).
                let local = function
                    .end_time
                    .saturating_add(self.siege_offset_at(function.end_time));
                let (year, month, day) = civil_from_days(local.div_euclid(86_400_000));
                put(
                    "Depth",
                    format!("<fstring p1=\"{}\">{fstring}</fstring>", function.lvl),
                );
                put(
                    "Cost",
                    format!(
                        "<fstring p1=\"{}\" p2=\"{}\">6</fstring>",
                        function.lease,
                        function.rate / 86_400_000
                    ),
                );
                put(
                    "Expire",
                    format!("<fstring p1=\"{day}\" p2=\"{month}\" p3=\"{year}\">5</fstring>"),
                );
                put(
                    "Reset",
                    format!("[<a action=\"bypass -h Script CastleChamberlain {key} 0\">Deactivate</a>]"),
                );
            }
        }
    }

    /// `funcConfirmHtml(player, npc, castle, func, level)`.
    fn chamberlain_func_confirm(
        &mut self,
        player: ObjectId,
        npc: ObjectId,
        castle: CastleId,
        func: i32,
        level: i32,
    ) -> Reply {
        if !self.chamberlain_may(player, npc, ClanAccess::CastleManageFunctions) {
            return page("chamberlain-21.html");
        }
        let fstring = if func == CASTLE_FUNC_TELEPORT { "9" } else { "10" };
        let current = self
            .residence_function(ResidenceRef::Castle(castle), func)
            .map(|f| f.lvl);
        let (file, replace) = if level == 0 {
            (
                "castleresetdeco.html".to_owned(),
                vec![("%AgitDecoSubmit%", func.to_string())],
            )
        } else if current == Some(level) {
            (
                "castledecoalreadyset.html".to_owned(),
                vec![(
                    "%AgitDecoEffect%",
                    format!("<fstring p1=\"{level}\">{fstring}</fstring>"),
                )],
            )
        } else {
            (
                format!("castledeco-0{func}.html"),
                vec![
                    (
                        "%AgitDecoCost%",
                        format!(
                            "<fstring p1=\"{}\" p2=\"{}\">6</fstring>",
                            self.chamberlain_function_fee(func, level),
                            self.chamberlain_function_ratio(func) / 86_400_000
                        ),
                    ),
                    (
                        "%AgitDecoEffect%",
                        format!("<fstring p1=\"{level}\">{fstring}</fstring>"),
                    ),
                    ("%AgitDecoSubmit%", format!("{func} {level}")),
                ],
            )
        };
        self.castle_script_send_packet(SCRIPT, player, npc, &file, &replace);
        Ok(None)
    }

    /// `onEvent(event, npc, player)` of the chamberlain.
    pub(super) fn chamberlain_on_event(&mut self, event: &str, npc: ObjectId, player: ObjectId) -> Reply {
        // `npc.getCastle()` is dereferenced by nearly every branch.
        let castle = self.npc_castle(npc).ok_or_else(|| {
            warn!("CastleChamberlain: the NPC {npc} stands at no castle.");
            ScriptFailed
        })?;
        let residence = ResidenceRef::Castle(castle);
        let st = words(event);
        let first = *st.first().ok_or(ScriptFailed)?;
        let args = &st[1..];
        let siege = self.is_castle_siege_in_progress(castle);
        let npc_id = self.castle_npc_id(npc);
        let no_rights = || page("chamberlain-21.html");
        match first {
            "chamberlain-01.html"
            | "manor-help-01.html"
            | "manor-help-02.html"
            | "manor-help-03.html"
            | "manor-help-04.html" => page(event),
            "fort_status" => {
                if !self.fort_is_my_lord(npc, player) {
                    return no_rights();
                }
                let mut list = String::new();
                for id in domain_fortresses(castle.0).ok_or(ScriptFailed)? {
                    let fort = self.fort(FortId(*id)).ok_or(ScriptFailed)?;
                    let fort_type = if fort.id.0 < 112 { "1300133" } else { "1300134" };
                    let status = match fort.state {
                        1 => "1300122",
                        2 => "1300124",
                        _ => "1300123",
                    };
                    list.push_str(&format!("<fstring>1300{}</fstring>", fort.id.0));
                    list.push_str(&format!(" (<fstring>{fort_type}</fstring>)"));
                    list.push_str(&format!(
                        " : <font color=\"00FFFF\"><fstring>{status}</fstring></font><br>"
                    ));
                }
                self.castle_script_send_packet(
                    SCRIPT,
                    player,
                    npc,
                    "chamberlain-28.html",
                    &[("%list%", list)],
                );
                Ok(None)
            }
            "siege_functions" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleManageFunctions) {
                    return no_rights();
                }
                if siege {
                    page("chamberlain-08.html")
                } else if !self.chamberlain_domain_fortress_in_contract(castle)? {
                    page("chamberlain-27.html")
                } else if !self.seven_signs_is_comp_results_period() {
                    page("chamberlain-26.html")
                } else {
                    page("chamberlain-12.html")
                }
            }
            "manage_doors" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleManageFunctions) {
                    return no_rights();
                }
                match args.split_first() {
                    Some((kind, doors)) => {
                        let doors: String = doors.iter().map(|d| format!(" {d}")).collect();
                        self.castle_script_send_packet(
                            SCRIPT,
                            player,
                            npc,
                            "chamberlain-13.html",
                            &[("%type%", (*kind).to_owned()), ("%doors%", doors)],
                        );
                        Ok(None)
                    }
                    None => page(&format!("{npc_id}-du.html")),
                }
            }
            "upgrade_doors" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleManageFunctions) {
                    return no_rights();
                }
                let kind = parse_int(args.first().ok_or(ScriptFailed)?)?;
                let level = parse_int(args.get(1).ok_or(ScriptFailed)?)?;
                let price = self.chamberlain_door_upgrade_price(kind, level);
                // `event.substring("upgrade_doors".length() + 1)`.
                let rest = event
                    .get("upgrade_doors".len() + 1..)
                    .unwrap_or_default()
                    .to_owned();
                self.castle_script_send_packet(
                    SCRIPT,
                    player,
                    npc,
                    "chamberlain-14.html",
                    &[("%gate_price%", price.to_string()), ("%event%", rest)],
                );
                Ok(None)
            }
            "upgrade_doors_confirm" => {
                self.chamberlain_upgrade_doors_confirm(args, npc, player, castle, siege)
            }
            "manage_trap" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleManageFunctions) {
                    return no_rights();
                }
                match args.first() {
                    Some(index) => {
                        let is_aden = self
                            .castle(castle)
                            .is_some_and(|c| c.name.eq_ignore_ascii_case("aden"));
                        let file = if is_aden {
                            "chamberlain-17a.html"
                        } else {
                            "chamberlain-17.html"
                        };
                        self.castle_script_send_packet(
                            SCRIPT,
                            player,
                            npc,
                            file,
                            &[("%trapIndex%", (*index).to_owned())],
                        );
                        Ok(None)
                    }
                    None => page(&format!("{npc_id}-tu.html")),
                }
            }
            "upgrade_trap" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleManageFunctions) {
                    return no_rights();
                }
                let index = *args.first().ok_or(ScriptFailed)?;
                let level = parse_int(args.get(1).ok_or(ScriptFailed)?)?;
                let price = self.chamberlain_trap_upgrade_price(level);
                self.castle_script_send_packet(
                    SCRIPT,
                    player,
                    npc,
                    "chamberlain-18.html",
                    &[
                        ("%trapIndex%", index.to_owned()),
                        ("%level%", level.to_string()),
                        ("%dmgzone_price%", price.to_string()),
                    ],
                );
                Ok(None)
            }
            "upgrade_trap_confirm" => self.chamberlain_upgrade_trap_confirm(args, npc, player, castle, siege),
            "receive_report" => self.chamberlain_receive_report(npc, player, castle, siege),
            "manage_tax" => {
                let tax = self.castle(castle).map_or(0, |c| c.tax_percent);
                if self.chamberlain_may(player, npc, ClanAccess::CastleVault) {
                    if siege {
                        return page("chamberlain-08.html");
                    }
                    let limit = self.chamberlain_tax_limit();
                    self.castle_script_send_packet(
                        SCRIPT,
                        player,
                        npc,
                        "castlesettaxrate.html",
                        &[
                            ("%tax_rate%", tax.to_string()),
                            // TODO(java): "Implement me!"
                            ("%next_tax_rate%", "0".to_owned()),
                            ("%tax_limit%", limit.to_string()),
                        ],
                    );
                    Ok(None)
                } else if self.castle_script_is_owner(player, npc) {
                    self.castle_script_send_packet(
                        SCRIPT,
                        player,
                        npc,
                        "chamberlain-03.html",
                        &[
                            ("%tax_rate%", tax.to_string()),
                            ("%next_tax_rate%", "0".to_owned()),
                        ],
                    );
                    Ok(None)
                } else {
                    no_rights()
                }
            }
            "set_tax" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleVault) {
                    return no_rights();
                }
                if siege {
                    return page("chamberlain-08.html");
                }
                let tax = match args.first() {
                    Some(token) => parse_int(token)?,
                    None => 0,
                };
                let limit = self.chamberlain_tax_limit();
                // NOTE(java-quirk): only the upper limit is checked; a negative rate is accepted.
                if tax > limit {
                    self.castle_script_send_packet(
                        SCRIPT,
                        player,
                        npc,
                        "castletoohightaxrate.html",
                        &[("%tax_limit%", limit.to_string())],
                    );
                } else {
                    self.castle_set_tax_percent(castle, tax);
                    self.castle_script_send_packet(
                        SCRIPT,
                        player,
                        npc,
                        "castleaftersettaxrate.html",
                        &[("%next_tax_rate%", tax.to_string())],
                    );
                }
                Ok(None)
            }
            "manage_vault" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleVault) {
                    return no_rights();
                }
                let mut seed_income: i64 = 0;
                if self.manor.settings.allow_manor {
                    // `getSeedProduction(id, false)` is null for a castle the manor never loaded.
                    let production = self.manor_seed_production(castle.0, false).ok_or(ScriptFailed)?;
                    for sp in production {
                        let diff = sp.start_amount.wrapping_sub(sp.amount);
                        if diff != 0 {
                            seed_income = seed_income.wrapping_add(diff.wrapping_mul(sp.price));
                        }
                    }
                }
                let treasury = self.castle(castle).map_or(0, |c| c.treasury);
                self.castle_script_send_packet(
                    SCRIPT,
                    player,
                    npc,
                    "castlemanagevault.html",
                    &[
                        ("%tax_income%", format_adena(treasury)),
                        // TODO(java): "Implement me!"
                        ("%tax_income_reserved%", "0".to_owned()),
                        ("%seed_income%", format_adena(seed_income)),
                    ],
                );
                Ok(None)
            }
            "deposit" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleVault) {
                    return no_rights();
                }
                let amount = match args.first() {
                    Some(token) => parse_long(token)?,
                    None => 0,
                };
                if amount > 0 && amount < self.settings.ground.max_adena {
                    if self.adena_of(player) >= amount {
                        self.script_take_items(player, l2_data::item::ADENA_ID, amount);
                        // A player's own deposit: written (D-51).
                        self.castle_add_to_treasury_no_tax(castle, amount, crate::residence::Writes::Player);
                    } else {
                        self.send(
                            player,
                            &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena),
                        );
                    }
                }
                page("chamberlain-01.html")
            }
            "withdraw" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleVault) {
                    return no_rights();
                }
                let amount = match args.first() {
                    Some(token) => parse_long(token)?,
                    None => 0,
                };
                let treasury = self.castle(castle).map_or(0, |c| c.treasury);
                // NOTE(java-quirk): a negative amount passes this check; it adds that much to the
                // treasury and gives the player nothing (`giveItems` ignores a count below 1).
                if amount <= treasury {
                    self.castle_add_to_treasury_no_tax(
                        castle,
                        amount.wrapping_neg(),
                        crate::residence::Writes::Player,
                    );
                    self.script_give_items(player, l2_data::item::ADENA_ID, amount);
                    page("chamberlain-01.html")
                } else {
                    self.castle_script_send_packet(
                        SCRIPT,
                        player,
                        npc,
                        "castlenotenoughbalance.html",
                        &[
                            ("%tax_income%", format_adena(treasury)),
                            ("%withdraw_amount%", format_adena(amount)),
                        ],
                    );
                    Ok(None)
                }
            }
            "manage_functions" => {
                if !self.castle_script_is_owner(player, npc) {
                    page("chamberlain-21.html")
                } else if siege {
                    page("chamberlain-08.html")
                } else {
                    page("chamberlain-23.html")
                }
            }
            "banish_foreigner_show" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleBanish) {
                    page("chamberlain-21.html")
                } else if siege {
                    page("chamberlain-08.html")
                } else {
                    page("chamberlain-10.html")
                }
            }
            "banish_foreigner" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleBanish) {
                    return no_rights();
                }
                if siege {
                    return page("chamberlain-08.html");
                }
                self.residence_banish_foreigners(residence);
                page("chamberlain-11.html")
            }
            "doors" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleOpenDoor) {
                    page("chamberlain-21.html")
                } else if siege {
                    page("chamberlain-08.html")
                } else {
                    page(&format!("{npc_id}-d.html"))
                }
            }
            "operate_door" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleOpenDoor) {
                    return page("chamberlain-21.html");
                }
                if siege {
                    return page("chamberlain-08.html");
                }
                let open = parse_int(args.first().ok_or(ScriptFailed)?)? == 1;
                for door in &args[1..] {
                    let door = parse_int(door)?;
                    self.residence_open_close_door(player, residence, door, open);
                }
                page(if open {
                    "chamberlain-05.html"
                } else {
                    "chamberlain-06.html"
                })
            }
            "additional_functions" => {
                if self.chamberlain_may(player, npc, ClanAccess::CastleManageFunctions) {
                    page("castletdecomanage.html")
                } else {
                    no_rights()
                }
            }
            "recovery" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleManageFunctions) {
                    return no_rights();
                }
                let mut replace = Vec::new();
                self.chamberlain_func_replace(castle, CASTLE_FUNC_RESTORE_HP, "HP", &mut replace);
                self.chamberlain_func_replace(castle, CASTLE_FUNC_RESTORE_MP, "MP", &mut replace);
                self.chamberlain_func_replace(castle, CASTLE_FUNC_RESTORE_EXP, "XP", &mut replace);
                let replace: Vec<(&str, String)> =
                    replace.iter().map(|(p, v)| (p.as_str(), v.clone())).collect();
                self.castle_script_send_packet(SCRIPT, player, npc, "castledeco-AR01.html", &replace);
                Ok(None)
            }
            "other" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleManageFunctions) {
                    return no_rights();
                }
                let mut replace = Vec::new();
                self.chamberlain_func_replace(castle, CASTLE_FUNC_TELEPORT, "TP", &mut replace);
                self.chamberlain_func_replace(castle, CASTLE_FUNC_SUPPORT, "BF", &mut replace);
                let replace: Vec<(&str, String)> =
                    replace.iter().map(|(p, v)| (p.as_str(), v.clone())).collect();
                self.castle_script_send_packet(SCRIPT, player, npc, "castledeco-AE01.html", &replace);
                Ok(None)
            }
            "HP" | "MP" | "XP" | "TP" | "BF" => {
                let level = parse_int(args.first().ok_or(ScriptFailed)?)?;
                let func = match first {
                    "HP" => CASTLE_FUNC_RESTORE_HP,
                    "MP" => CASTLE_FUNC_RESTORE_MP,
                    "XP" => CASTLE_FUNC_RESTORE_EXP,
                    "TP" => CASTLE_FUNC_TELEPORT,
                    _ => CASTLE_FUNC_SUPPORT,
                };
                self.chamberlain_func_confirm(player, npc, castle, func, level)
            }
            "set_func" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleManageFunctions) {
                    return no_rights();
                }
                let func = parse_int(args.first().ok_or(ScriptFailed)?)?;
                let level = parse_int(args.get(1).ok_or(ScriptFailed)?)?;
                if level == 0 {
                    self.residence_update_functions(player, residence, func, level, 0, 0, false);
                    Ok(None)
                } else {
                    let add_new = self.residence_function(residence, func).is_none();
                    let fee = self.chamberlain_function_fee(func, level);
                    let ratio = self.chamberlain_function_ratio(func);
                    if self.residence_update_functions(player, residence, func, level, fee, ratio, add_new) {
                        Ok(None)
                    } else {
                        page("chamberlain-09.html")
                    }
                }
            }
            "functions" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleFunctions) {
                    return no_rights();
                }
                let depth = |world: &Self, func: i32| {
                    world
                        .residence_function(residence, func)
                        .map_or("0".to_owned(), |f| f.lvl.to_string())
                };
                let replace = [
                    ("%HPDepth%", depth(self, CASTLE_FUNC_RESTORE_HP)),
                    ("%MPDepth%", depth(self, CASTLE_FUNC_RESTORE_MP)),
                    ("%XPDepth%", depth(self, CASTLE_FUNC_RESTORE_EXP)),
                ];
                self.castle_script_send_packet(SCRIPT, player, npc, "castledecofunction.html", &replace);
                Ok(None)
            }
            "teleport" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleFunctions) {
                    return no_rights();
                }
                match self.residence_function(residence, CASTLE_FUNC_TELEPORT) {
                    None => page("castlefuncdisabled.html"),
                    Some(f) => page(&format!("{npc_id}-t{}.html", f.lvl)),
                }
            }
            "goto" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleFunctions) {
                    return no_rights();
                }
                let Some(level) = self
                    .residence_function(residence, CASTLE_FUNC_TELEPORT)
                    .map(|f| f.lvl)
                else {
                    return page("castlefuncdisabled.html");
                };
                // `goto listId locId`: the list index is two characters, the function level and
                // the location (`Character.getNumericValue`).
                let list_index: Vec<char> = args.first().ok_or(ScriptFailed)?.chars().collect();
                let function_level = if list_index.len() == 2 {
                    numeric_value(list_index[0])
                } else {
                    -1
                };
                if level == function_level {
                    let teleporters = self.talk.teleporters.clone();
                    if let Some(holder) = teleporters
                        .as_ref()
                        .and_then(|d| d.get_holder(npc_id, l2_data::TeleportType::Other.name()))
                    {
                        self.do_teleport(npc, player, holder, numeric_value(list_index[1]));
                    }
                }
                Ok(None)
            }
            "buffer" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleFunctions) {
                    return no_rights();
                }
                let Some(level) = self
                    .residence_function(residence, CASTLE_FUNC_SUPPORT)
                    .map(|f| f.lvl)
                else {
                    return page("castlefuncdisabled.html");
                };
                let mp = self.core_of(npc).map_or(0, |c| c.cur_mp as i32);
                self.castle_script_send_packet(
                    SCRIPT,
                    player,
                    npc,
                    &format!("castlebuff-0{level}.html"),
                    &[("%MPLeft%", mp.to_string())],
                );
                Ok(None)
            }
            "cast_buff" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleFunctions) {
                    return no_rights();
                }
                if self.residence_function(residence, CASTLE_FUNC_SUPPORT).is_none() {
                    return page("castlefuncdisabled.html");
                }
                let index = parse_int(args.first().ok_or(ScriptFailed)?)?;
                // `BUFFS.length > index`; a negative index throws at `BUFFS[index]`.
                if usize::try_from(index).is_ok_and(|i| i >= BUFFS.len()) {
                    return Ok(None);
                }
                let (skill_id, skill_level) = BUFFS[usize::try_from(index).map_err(|_| ScriptFailed)?];
                let mp_consume = self
                    .skill_table()
                    .and_then(|t| t.get(skill_id, skill_level).map(|s| f64::from(s.mp_consume)))
                    .ok_or(ScriptFailed)?;
                let mp = self.core_of(npc).map_or(0.0, |c| c.cur_mp);
                let file = if mp_consume < mp {
                    self.set_npc_target(npc, Some(player));
                    self.do_cast(npc, SkillKey::new(skill_id, skill_level));
                    "castleafterbuff.html"
                } else {
                    "castlenotenoughmp.html"
                };
                let mp_left = self.core_of(npc).map_or(0, |c| c.cur_mp as i32);
                self.castle_script_send_packet(
                    SCRIPT,
                    player,
                    npc,
                    file,
                    &[("%MPLeft%", mp_left.to_string())],
                );
                Ok(None)
            }
            "list_siege_clans" => {
                if self.chamberlain_may(player, npc, ClanAccess::CastleSiege) {
                    self.send_siege_info(player, castle);
                    Ok(None)
                } else {
                    no_rights()
                }
            }
            "list_territory_clans" => {
                if self.chamberlain_may(player, npc, ClanAccess::CastleSiege) {
                    let packet = self.dominion_registry_packet(castle, player);
                    self.send(player, &packet);
                    Ok(None)
                } else {
                    no_rights()
                }
            }
            "manor" => {
                if self.manor.settings.allow_manor {
                    if self.chamberlain_may(player, npc, ClanAccess::CastleManor) {
                        page("manor.html")
                    } else {
                        no_rights()
                    }
                } else {
                    self.send_message(player, "Manor system is deactivated.");
                    Ok(None)
                }
            }
            "products" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleFunctions) {
                    return no_rights();
                }
                self.castle_script_send_packet(
                    SCRIPT,
                    player,
                    npc,
                    "chamberlain-22.html",
                    &[("%npcId%", npc_id.to_string())],
                );
                Ok(None)
            }
            "buy" => {
                if !self.chamberlain_may(player, npc, ClanAccess::CastleFunctions) {
                    return no_rights();
                }
                let list_id = parse_int(args.first().ok_or(ScriptFailed)?)?;
                self.show_buy_window(npc, player, list_id);
                Ok(None)
            }
            "give_crown" => {
                if siege {
                    page("chamberlain-08.html")
                } else if self.fort_is_my_lord(npc, player) {
                    if self.script_has_items(player, CROWN) {
                        page("chamberlain-24.html")
                    } else {
                        let name = self
                            .players
                            .get(&player)
                            .map(|p| p.data.name.clone())
                            .unwrap_or_default();
                        self.castle_script_send_packet(
                            SCRIPT,
                            player,
                            npc,
                            "chamberlain-25.html",
                            &[
                                ("%owner_name%", name),
                                ("%feud_name%", (1_001_000 + castle.0).to_string()),
                            ],
                        );
                        self.script_give_items(player, CROWN, 1);
                        Ok(None)
                    }
                } else {
                    no_rights()
                }
            }
            "manors_cert" | "manors_cert_confirm" => {
                self.chamberlain_manors_cert(first == "manors_cert_confirm", npc, player, castle, siege)
            }
            _ => Ok(None),
        }
    }

    /// `case "upgrade_doors_confirm"`.
    fn chamberlain_upgrade_doors_confirm(
        &mut self,
        args: &[&str],
        npc: ObjectId,
        player: ObjectId,
        castle: CastleId,
        siege: bool,
    ) -> Reply {
        if !self.chamberlain_may(player, npc, ClanAccess::CastleManageFunctions) {
            return page("chamberlain-21.html");
        }
        if siege {
            return page("chamberlain-08.html");
        }
        let kind = parse_int(args.first().ok_or(ScriptFailed)?)?;
        let level = parse_int(args.get(1).ok_or(ScriptFailed)?)?;
        let price = self.chamberlain_door_upgrade_price(kind, level);
        // `for (i = 0; i <= st.countTokens(); i++) doors[i] = Integer.parseInt(st.nextToken())`
        // with `doors = new int[2]`: the count shrinks as the tokens are read.
        // NOTE(java-quirk): four or more door ids throw before anything changed; none at all
        // throws `NoSuchElementException`.
        let rest = &args[2..];
        let mut doors = [0_i32; 2];
        let mut index = 0;
        while index <= rest.len().saturating_sub(index) {
            let token = rest.get(index).ok_or(ScriptFailed)?;
            let door = parse_int(token)?;
            *doors.get_mut(index).ok_or(ScriptFailed)? = door;
            index += 1;
        }
        let Some(door) = self.residence_door(ResidenceRef::Castle(castle), doors[0]) else {
            return Ok(None);
        };
        let current_level = self.door(door).map_or(1, |d| d.upgrade_hp_ratio);
        if current_level >= level {
            self.castle_script_send_packet(
                SCRIPT,
                player,
                npc,
                "chamberlain-15.html",
                &[("%doorlevel%", current_level.to_string())],
            );
            Ok(None)
        } else if self.adena_of(player) >= i64::from(price) {
            self.script_take_items(player, l2_data::item::ADENA_ID, i64::from(price));
            for door_id in doors {
                self.castle_set_door_upgrade(castle, door_id, level, true);
            }
            page("chamberlain-16.html")
        } else {
            page("chamberlain-09.html")
        }
    }

    /// `case "upgrade_trap_confirm"`.
    fn chamberlain_upgrade_trap_confirm(
        &mut self,
        args: &[&str],
        npc: ObjectId,
        player: ObjectId,
        castle: CastleId,
        siege: bool,
    ) -> Reply {
        if !self.chamberlain_may(player, npc, ClanAccess::CastleManageFunctions) {
            return page("chamberlain-21.html");
        }
        if siege {
            return page("chamberlain-08.html");
        }
        let index = parse_int(args.first().ok_or(ScriptFailed)?)?;
        let level = parse_int(args.get(1).ok_or(ScriptFailed)?)?;
        let price = self.chamberlain_trap_upgrade_price(level);
        // `getFlameTowers(id).get(index)` throws for an index the list does not have.
        let current_level = usize::try_from(index)
            .ok()
            .filter(|i| {
                self.sieges
                    .settings
                    .castles
                    .get(&castle.0)
                    .is_some_and(|c| *i < c.flame_towers.len())
            })
            .map(|i| self.castle_trap_upgrade_level(castle, i))
            .ok_or(ScriptFailed)?;
        if current_level >= level {
            self.castle_script_send_packet(
                SCRIPT,
                player,
                npc,
                "chamberlain-19.html",
                &[("%dmglevel%", current_level.to_string())],
            );
            Ok(None)
        } else if self.adena_of(player) >= i64::from(price) {
            self.script_take_items(player, l2_data::item::ADENA_ID, i64::from(price));
            self.castle_set_trap_upgrade(castle, index, level, true);
            page("chamberlain-20.html")
        } else {
            page("chamberlain-09.html")
        }
    }

    /// `case "receive_report"`.
    fn chamberlain_receive_report(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        castle: CastleId,
        siege: bool,
    ) -> Reply {
        if !self.fort_is_my_lord(npc, player) {
            return page("chamberlain-21.html");
        }
        if siege {
            return page("chamberlain-07.html");
        }
        let owner = self.castle(castle).map_or(0, |c| c.owner_id);
        // `ClanTable.getClan(castle.getOwnerId())` is null for a castle nobody owns.
        let clan_name = self.clan(owner).ok_or(ScriptFailed)?.name.clone();
        let leader = self.clan_leader_name(owner);
        let event = match self.seven_signs_current_period() {
            PERIOD_COMP_RECRUITING => Some("1000509"),
            PERIOD_COMPETITION => Some("1000507"),
            PERIOD_SEAL_VALIDATION | PERIOD_COMP_RESULTS => Some("1000508"),
            _ => None,
        };
        let mut replace = vec![
            ("%clanleadername%", leader),
            ("%clanname%", clan_name),
            ("%castlename%", (1_001_000 + castle.0).to_string()),
        ];
        // An unknown period leaves `%ss_event%` as it is.
        if let Some(event) = event {
            replace.push(("%ss_event%", event.to_owned()));
        }
        replace.push((
            "%ss_avarice%",
            seal_owner_string(self.seven_signs_seal_owner(SEAL_AVARICE)).to_owned(),
        ));
        replace.push((
            "%ss_gnosis%",
            seal_owner_string(self.seven_signs_seal_owner(SEAL_GNOSIS)).to_owned(),
        ));
        replace.push((
            "%ss_strife%",
            seal_owner_string(self.seven_signs_seal_owner(SEAL_STRIFE)).to_owned(),
        ));
        self.castle_script_send_packet(SCRIPT, player, npc, "chamberlain-02.html", &replace);
        Ok(None)
    }

    /// `case "manors_cert"` and `case "manors_cert_confirm"`: the Seven Signs tickets the lord of
    /// the dawn buys.
    fn chamberlain_manors_cert(
        &mut self,
        confirm: bool,
        npc: ObjectId,
        player: ObjectId,
        castle: CastleId,
        siege: bool,
    ) -> Reply {
        if !self.fort_is_my_lord(npc, player) {
            return page("chamberlain-21.html");
        }
        if siege {
            return page("chamberlain-08.html");
        }
        if !(self.seven_signs_player_cabal(player) == CABAL_DAWN && self.seven_signs_is_competition_period())
        {
            return page("ssq_notdawnorevent.html");
        }
        let s = self.residence_npc.settings.clone();
        let ticket_count = self.castle(castle).map_or(0, |c| c.ticket_buy_count);
        // Integer division by a bundle of zero throws.
        if s.ssq_dawn_ticket_bundle == 0 {
            return Err(ScriptFailed);
        }
        if ticket_count >= s.ssq_dawn_ticket_quantity / s.ssq_dawn_ticket_bundle {
            return page("ssq_notenoughticket.html");
        }
        if !confirm {
            self.castle_script_send_packet(
                SCRIPT,
                player,
                npc,
                "ssq_selldawnticket.html",
                &[
                    (
                        "%DawnTicketLeft%",
                        s.ssq_dawn_ticket_quantity
                            .wrapping_sub(ticket_count.wrapping_mul(s.ssq_dawn_ticket_bundle))
                            .to_string(),
                    ),
                    ("%DawnTicketBundle%", s.ssq_dawn_ticket_bundle.to_string()),
                    (
                        "%DawnTicketPrice%",
                        s.ssq_dawn_ticket_price
                            .wrapping_mul(s.ssq_dawn_ticket_bundle)
                            .to_string(),
                    ),
                ],
            );
            return Ok(None);
        }
        let total_cost = i64::from(s.ssq_dawn_ticket_price.wrapping_mul(s.ssq_dawn_ticket_bundle));
        if self.adena_of(player) >= total_cost {
            self.script_take_items(player, l2_data::item::ADENA_ID, total_cost);
            self.script_give_items(
                player,
                s.ssq_manors_agreement_id,
                i64::from(s.ssq_dawn_ticket_bundle),
            );
            self.castle_set_ticket_buy_count(castle, ticket_count + 1);
            Ok(None)
        } else {
            page("chamberlain-09.html")
        }
    }
}

/// Java `Character.getNumericValue(ch)` for the characters the pages use: `0`-`9` are 0 to 9,
/// letters 10 to 35, anything else -1.
fn numeric_value(c: char) -> i32 {
    c.to_digit(36).map_or(-1, |d| d as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_values_follow_character_get_numeric_value() {
        assert_eq!(numeric_value('0'), 0);
        assert_eq!(numeric_value('7'), 7);
        assert_eq!(numeric_value('a'), 10);
        assert_eq!(numeric_value('Z'), 35);
        assert_eq!(numeric_value('-'), -1);
    }

    #[test]
    fn only_the_static_pages_skip_the_reread() {
        assert!(!needs_reread("chamberlain-01.html"));
        assert!(!needs_reread("manor-help-03.html"));
        assert!(needs_reread("manage_tax"));
        assert!(needs_reread("withdraw 5"));
        assert!(needs_reread("chamberlain-04.html"));
    }

    #[test]
    fn the_domain_fortresses_match_the_script() {
        assert_eq!(domain_fortresses(1), Some(&[101, 102, 112, 113][..]));
        assert_eq!(domain_fortresses(5).map(<[i32]>::len), Some(4));
        assert_eq!(domain_fortresses(10), None);
    }
}
