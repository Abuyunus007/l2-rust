//! `MercenaryCaptain`: the captain of the Territory War mercenaries of a castle (striders for
//! territory badges, the elite mercenary multisells, the registration window).
// Java: dist/game/data/scripts/ai/others/MercenaryCaptain/MercenaryCaptain.java
// Java: network/serverpackets/ExShowDominionRegistry.java
//
// The captains stand where the Territory War puts them (`territory/`, M7-TW-01): the war spawns
// them with the town NPCs of an owned territory, starts their hourly shout
// (`startQuestTimer("say", DELAY, spawn.getNpc(), null, true)`, which calls
// [`World::captain_say`]), and owns `isTWInProgress()`, the badge counts of `TerritoryWar.ini` and
// the registration window (`ExShowDominionRegistry`).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{ChatType, NpcSay};
use l2_util::ObjectId;

use super::{CastleScript, Reply, ScriptFailed, words};
use crate::world::World;

const SCRIPT: CastleScript = CastleScript::MercenaryCaptain;

/// `MercenaryCaptain.NPCS`: the badge item each captain takes (the Territory badges of Gludio to
/// Schuttgart).
fn badge_of(npc_id: i32) -> Option<i32> {
    (36481..=36489)
        .contains(&npc_id)
        .then(|| 13757 + (npc_id - 36481))
}

/// `STRIDER_WIND`.
const STRIDER_WIND: i32 = 4422;
/// `STRIDER_STAR`.
const STRIDER_STAR: i32 = 4423;
/// `STRIDER_TWILIGHT`.
const STRIDER_TWILIGHT: i32 = 4424;
/// `GUARDIAN_STRIDER`.
const GUARDIAN_STRIDER: i32 = 14819;
/// `ELITE_MERCENARY_CERTIFICATE`.
const ELITE_MERCENARY_CERTIFICATE: i32 = 13767;
/// `TOP_ELITE_MERCENARY_CERTIFICATE`.
const TOP_ELITE_MERCENARY_CERTIFICATE: i32 = 13768;
/// `MIN_LEVEL`.
const MIN_LEVEL: i32 = 40;
/// `CLASS_LEVEL`.
const CLASS_LEVEL: i32 = 2;

impl World {
    /// `onFirstTalk` of the captain.
    pub(super) fn captain_first_talk(&self, npc: ObjectId, player: ObjectId) -> String {
        let class_level = self
            .players
            .get(&player)
            .and_then(|p| u16::try_from(p.data.class_id).ok())
            .map_or(0, |c| l2_data::ClassId(c).class_level());
        let level = self.players.get(&player).map_or(0, |p| p.core.level);
        let in_war = self.castle_siege_running_at(npc).unwrap_or(false) || self.is_tw_in_progress();
        if level < MIN_LEVEL || class_level < CLASS_LEVEL {
            "36481-08.html".to_owned()
        } else if self.fort_is_my_lord(npc, player) {
            if in_war { "36481-05.html" } else { "36481-04.html" }.to_owned()
        } else if in_war {
            "36481-06.html".to_owned()
        } else {
            format!("{}-01.html", self.castle_npc_id(npc))
        }
    }

    /// `onEvent` of the captain with a player (a button).
    pub(super) fn captain_on_event(&mut self, event: &str, npc: ObjectId, player: ObjectId) -> Reply {
        let st = words(event);
        let npc_id = self.castle_npc_id(npc);
        match *st.first().ok_or(ScriptFailed)? {
            "36481-02.html" => Ok(Some(event.to_owned())),
            "36481-03.html" => {
                let s = &self.territory.settings;
                let replace = [
                    ("%strider%", s.min_tw_badge_for_striders.to_string()),
                    ("%gstrider%", s.min_tw_badge_for_big_strider.to_string()),
                ];
                self.castle_script_send_packet(SCRIPT, player, npc, "36481-03.html", &replace);
                Ok(None)
            }
            "territory" => {
                let castle = self.npc_castle(npc).ok_or(ScriptFailed)?;
                let packet = self.dominion_registry_packet(castle, player);
                self.send(player, &packet);
                Ok(None)
            }
            "strider" => {
                let kind = *st.get(1).ok_or(ScriptFailed)?;
                let price = if kind == "3" {
                    self.territory.settings.min_tw_badge_for_big_strider
                } else {
                    self.territory.settings.min_tw_badge_for_striders
                };
                let badge = badge_of(npc_id).ok_or(ScriptFailed)?;
                if self.inventory_item_count(player, badge) < i64::from(price) {
                    return Ok(Some("36481-07.html".to_owned()));
                }
                let strider = match kind {
                    "0" => STRIDER_WIND,
                    "1" => STRIDER_STAR,
                    "2" => STRIDER_TWILIGHT,
                    "3" => GUARDIAN_STRIDER,
                    _ => {
                        tracing::warn!("MercenaryCaptain: Unknown strider type: {kind}");
                        return Ok(None);
                    }
                };
                self.script_take_items(player, badge, i64::from(price));
                self.script_give_items(player, strider, 1);
                Ok(Some("36481-09.html".to_owned()))
            }
            "elite" => {
                if !self.script_has_items(player, ELITE_MERCENARY_CERTIFICATE) {
                    return Ok(Some("36481-10.html".to_owned()));
                }
                let castle = self.npc_castle(npc).ok_or(ScriptFailed)?;
                self.multisell_separate_and_send(676 + castle.0, player, Some(npc), false, 1.0, 1.0);
                Ok(None)
            }
            "top-elite" => {
                if !self.script_has_items(player, TOP_ELITE_MERCENARY_CERTIFICATE) {
                    return Ok(Some("36481-10.html".to_owned()));
                }
                let castle = self.npc_castle(npc).ok_or(ScriptFailed)?;
                self.multisell_separate_and_send(685 + castle.0, player, Some(npc), false, 1.0, 1.0);
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// The `say` event of the captain (no player): shouts about the war, or about hiring. The war
    /// arms the repeating timer of each captain it spawned (`territory/war.rs`).
    pub fn captain_say(&mut self, npc: ObjectId) {
        if !self.npcs.contains_key(&npc) {
            return;
        }
        let string = if self.is_tw_in_progress() {
            NpcStringId::ChargeChargeCharge
        } else if self.rng().below(2) == 0 {
            NpcStringId::CourageAmbitionPassionMercenariesWhoWantToRealizeTheirDreamOfFightingInTheTerritoryWarComeToMeFortuneAndGloryAreWaitingForYou
        } else {
            NpcStringId::DoYouWishToFightAreYouAfraidNoMatterHowHardYouTryYouHaveNowhereToRunButIfYouFaceItHeadOnOurMercenaryTroopWillHelpYouOut
        };
        let display_id = self.npc_template(npc).map_or(0, |t| t.display_id);
        self.broadcast_packet(
            npc,
            &NpcSay {
                object_id: npc.0,
                text_type: ChatType::NpcShout as i32,
                npc_id: display_id,
                npc_string: string as i32,
                text: String::new(),
                parameters: Vec::new(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_badge_of_each_captain() {
        assert_eq!(badge_of(36481), Some(13757));
        assert_eq!(badge_of(36489), Some(13765));
        assert_eq!(badge_of(36490), None);
    }
}
