//! The small castle scripts: `CastleSiegeManager`, `CastleMercenaryManager`, `CastleWarehouse`
//! (the vault keeper of the blood alliance items) and `CastleBlacksmith`.
// Java: dist/game/data/scripts/ai/others/CastleSiegeManager/CastleSiegeManager.java
// Java: dist/game/data/scripts/ai/others/CastleMercenaryManager/CastleMercenaryManager.java
// Java: dist/game/data/scripts/ai/others/CastleWarehouse/CastleWarehouse.java
// Java: dist/game/data/scripts/ai/others/CastleBlacksmith/CastleBlacksmith.java

use l2_util::ObjectId;
use tracing::{debug, warn};

use super::seven_signs::SEAL_STRIFE;
use super::{CABAL_DAWN, CABAL_DUSK, CastleScript, Reply, ScriptFailed, parse_int, words};
use crate::clan::model::ClanAccess;
use crate::residence::{ResidenceWrite, Writes};
use crate::world::World;

/// `CastleWarehouse.BLOOD_OATH`.
const BLOOD_OATH: i32 = 9910;
/// `CastleWarehouse.BLOOD_ALLIANCE`.
const BLOOD_ALLIANCE: i32 = 9911;

/// `Npc.getConquerableHall()`: `CHSiegeManager.getNearbyClanHall(x, y, 10000)`.
const CONQUERABLE_HALL_DISTANCE: i32 = 10_000;

impl World {
    // ------------------------------------------------------------------ CastleSiegeManager

    /// `CastleSiegeManager.isInSiege(npc)`.
    fn siege_manager_is_in_siege(&self, npc: ObjectId) -> Result<bool, ScriptFailed> {
        // M7-CHSIEGE-01: `getConquerableHall().isInSiege()`.
        self.castle_siege_running_at(npc).ok_or(ScriptFailed)
    }

    /// `onFirstTalk` of the siege manager: the page, or `None` when it sent the siege list.
    pub(super) fn siege_manager_first_talk(&mut self, npc: ObjectId, player: ObjectId) -> Option<String> {
        let in_siege = match self.siege_manager_is_in_siege(npc) {
            Ok(in_siege) => in_siege,
            Err(ScriptFailed) => {
                warn!("CastleSiegeManager: the NPC {npc} stands at no castle.");
                return None;
            }
        };
        if self.is_clan_leader(player) && self.clan_id_of(player) == self.castle_owner_of_npc(npc) {
            return Some(
                if in_siege {
                    "CastleSiegeManager.html"
                } else {
                    "CastleSiegeManager-01.html"
                }
                .to_owned(),
            );
        }
        if in_siege {
            return Some("CastleSiegeManager-02.html".to_owned());
        }
        let hall = self
            .position_of(npc)
            .and_then(|(x, y, _)| self.nearby_siegable_hall(x, y, CONQUERABLE_HALL_DISTANCE));
        if let Some(hall) = hall {
            // M7-CHSIEGE-01: `getConquerableHall().showSiegeInfo(player)`.
            debug!("CastleSiegeManager: the siege info of {hall} is M7-CHSIEGE-01's.");
            return None;
        }
        if let Some(castle) = self.npc_castle(npc) {
            self.send_siege_info(player, castle);
        }
        None
    }

    // ------------------------------------------------------------------ CastleMercenaryManager

    /// `onFirstTalk` of the mercenary manager.
    pub(super) fn mercenary_manager_first_talk(&self, npc: ObjectId, player: ObjectId) -> String {
        let castle_owner = self.castle_owner_of_npc(npc);
        let allowed = self.is_gm(player)
            || (self.clan_id_of(player) == castle_owner
                && self.has_clan_access(player, ClanAccess::CastleMercenaries));
        if !allowed {
            return "mercmanager-no.html".to_owned();
        }
        // NOTE(java-quirk): `getSiege()` of a castle that is not there throws; the page is the
        // normal one then.
        if self.castle_siege_running_at(npc).unwrap_or(false) {
            return "mercmanager-siege.html".to_owned();
        }
        match self.seven_signs_seal_owner(SEAL_STRIFE) {
            CABAL_DUSK => "mercmanager-dusk.html",
            CABAL_DAWN => "mercmanager-dawn.html",
            _ => "mercmanager.html",
        }
        .to_owned()
    }

    /// `onEvent` of the mercenary manager.
    pub(super) fn mercenary_manager_on_event(
        &mut self,
        event: &str,
        npc: ObjectId,
        player: ObjectId,
    ) -> Reply {
        let st = words(event);
        match *st.first().ok_or(ScriptFailed)? {
            "limit" => {
                let castle = self
                    .npc_castle(npc)
                    .and_then(|c| self.castle(c))
                    .map(|c| (c.id, c.name.clone()))
                    .ok_or(ScriptFailed)?;
                // NOTE(java-quirk): the names are compared case sensitive, and the castle table
                // has "Aden" and "Rune", so the general page is what a stock database shows.
                let file = match castle.1.as_str() {
                    "aden" => "mercmanager-aden-limit.html",
                    "rune" => "mercmanager-rune-limit.html",
                    _ => "mercmanager-limit.html",
                };
                self.castle_script_send_packet(
                    CastleScript::MercenaryManager,
                    player,
                    npc,
                    file,
                    &[("%feud_name%", (1_001_000 + castle.0.0).to_string())],
                );
                Ok(None)
            }
            "buy" => {
                if self.seven_signs_is_seal_validation() {
                    // `Integer.parseInt(npc.getId() + st.nextToken())`: the digits side by side.
                    let list_id = parse_int(&format!(
                        "{}{}",
                        self.castle_npc_id(npc),
                        st.get(1).ok_or(ScriptFailed)?
                    ))?;
                    // NOTE: not affected by castle taxes, the base tax is 20% (done in the buy
                    // lists).
                    self.show_buy_window_untaxed(npc, player, list_id);
                    Ok(None)
                } else {
                    Ok(Some("mercmanager-ssq.html".to_owned()))
                }
            }
            "main" => Ok(Some(self.mercenary_manager_first_talk(npc, player))),
            "mercmanager-01.html" => Ok(Some(event.to_owned())),
            _ => Ok(None),
        }
    }

    /// Java `Merchant.showBuyWindow(player, value, false)`: the buy window without any tax.
    fn show_buy_window_untaxed(&mut self, npc: ObjectId, player: ObjectId, list_id: i32) {
        use l2_protocol_game::server::ActionFailed;
        let npc_id = self.castle_npc_id(npc);
        let allowed = match self.shop.lists.as_ref().and_then(|l| l.get(list_id)) {
            None => {
                warn!("BuyList not found! BuyListId:{list_id}");
                self.send(player, &ActionFailed);
                return;
            }
            Some(list) => list.is_npc_allowed(npc_id),
        };
        if !allowed {
            warn!("Npc not allowed in BuyList! BuyListId:{list_id} NpcId:{npc_id}");
            self.send(player, &ActionFailed);
            return;
        }
        self.set_inventory_blocking_status(player);
        if let Some(packet) = self.buy_list_packet(player, list_id, 0.0) {
            self.send(player, &packet);
        }
        self.send_ex_buy_sell_list(player, false);
        self.send(player, &ActionFailed);
    }

    // ------------------------------------------------------------------ CastleWarehouse

    /// `onEvent` of the vault keeper.
    pub(super) fn warehouse_on_event(&mut self, event: &str, npc: ObjectId, player: ObjectId) -> Reply {
        match event {
            "warehouse-01.html" | "warehouse-02.html" | "warehouse-03.html" => Ok(Some(event.to_owned())),
            "warehouse-04.html" => {
                if !self.fort_is_my_lord(npc, player) {
                    return Ok(Some("warehouse-no.html".to_owned()));
                }
                let blood = self.clan_of(player).map_or(0, |c| c.blood_alliance_count);
                let content = self
                    .castle_script_htm(CastleScript::Warehouse, player, "warehouse-04.html")
                    .ok_or(ScriptFailed)?;
                // The whole page: `showResult` sends it as it is.
                Ok(Some(content.replace("%blood%", &blood.to_string())))
            }
            "Receive" => {
                if !self.fort_is_my_lord(npc, player) {
                    return Ok(Some("warehouse-no.html".to_owned()));
                }
                let clan_id = self.clan_id_of(player);
                let count = self.clan_of(player).map_or(0, |c| c.blood_alliance_count);
                if count == 0 {
                    return Ok(Some("warehouse-05.html".to_owned()));
                }
                self.script_give_items(player, BLOOD_ALLIANCE, i64::from(count));
                // `clan.resetBloodAllianceCount()`: the lord's own action, written.
                // FOLLOW-UP(D-50): the count is not read again first (see the module comment).
                if let Some(clan) = self.clan_mut(clan_id) {
                    clan.blood_alliance_count = 0;
                }
                self.write_residence(
                    Writes::Player,
                    ResidenceWrite::ClanBloodAllianceCount { clan_id, count: 0 },
                );
                Ok(Some("warehouse-06.html".to_owned()))
            }
            "Exchange" => {
                if !self.fort_is_my_lord(npc, player) {
                    Ok(Some("warehouse-no.html".to_owned()))
                } else if !self.script_has_items(player, BLOOD_ALLIANCE) {
                    Ok(Some("warehouse-08.html".to_owned()))
                } else {
                    self.script_take_items(player, BLOOD_ALLIANCE, 1);
                    self.script_give_items(player, BLOOD_OATH, 30);
                    Ok(Some("warehouse-07.html".to_owned()))
                }
            }
            // `default: htmltext = null`.
            _ => Ok(None),
        }
    }

    // ------------------------------------------------------------------ CastleBlacksmith

    /// `CastleBlacksmith.hasRights(player, npc)`.
    fn blacksmith_has_rights(&self, player: ObjectId, npc: ObjectId) -> bool {
        self.is_gm(player)
            || self.fort_is_my_lord(npc, player)
            || (self.clan_id_of(player) == self.castle_owner_of_npc(npc)
                && self.has_clan_access(player, ClanAccess::CastleManor))
    }

    /// `onFirstTalk` of the blacksmith.
    pub(super) fn blacksmith_first_talk(&self, npc: ObjectId, player: ObjectId) -> String {
        if self.blacksmith_has_rights(player, npc) {
            format!("{}-01.html", self.castle_npc_id(npc))
        } else {
            "no.html".to_owned()
        }
    }

    /// `onEvent` of the blacksmith.
    pub(super) fn blacksmith_on_event(&self, event: &str, npc: ObjectId, player: ObjectId) -> Option<String> {
        let page = format!("{}-02.html", self.castle_npc_id(npc));
        (event.eq_ignore_ascii_case(&page) && self.blacksmith_has_rights(player, npc))
            .then(|| event.to_owned())
    }
}
