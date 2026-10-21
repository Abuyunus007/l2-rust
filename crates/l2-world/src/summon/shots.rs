//! The shots of a summon: the beast shot item handlers, the summon's own recharge and the
//! automatic use of beast shots.
// Java: dist/game/data/scripts/handlers/items/{BeastSoulShot,BeastSpiritShot}.java
// Java: entity/actor/Summon.java (rechargeShots, isChargedShot, setChargedShot)
// Java: network/clientpackets/RequestAutoSoulShot.java (the beast shot branch)
//
// The charge of a summon is the summon's own mask (`Summon._shotsMask`): the shot table keys it by
// the summon's object id, as it does for NPCs (`inventory/handlers/shots.rs`).

use l2_data::item::types::ActionType;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ExAutoSoulShot, MagicSkillUse, SkillPosition, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use crate::inventory::handlers::shots::ShotType;
use crate::inventory::{LOC_INVENTORY, LOC_PAPERDOLL};
use crate::player::ItemData;
use crate::world::World;

/// The beast shots `RequestAutoSoulShot` charges on activation.
const BEAST_SHOT_IDS: [i32; 6] = [6645, 6646, 6647, 20332, 20333, 20334];
/// `BeastSpiritShot`: "TODO: Unhardcode these!" (the blessed beast spiritshots).
const BLESSED_BEAST_SPIRITSHOTS: [i32; 2] = [6647, 20334];

impl World {
    /// Java `BeastSoulShot.onItemUse(playable, item, forceUse)` (`spirit` false) and
    /// `BeastSpiritShot.onItemUse` (`spirit` true), for a player. `true` when a shot was charged.
    pub(crate) fn use_beast_shot(&mut self, owner: ObjectId, item: &ItemData, spirit: bool) -> bool {
        // `!playable.isPlayer()`: only players use items here.
        let Some(summon) = self.summon_of(owner) else {
            self.send(
                owner,
                &SystemMessage::new(SystemMessageId::PetsAndServitorsAreNotAvailableAtThisTime),
            );
            return false;
        };
        if self.is_dead(summon) {
            self.send(
                owner,
                &SystemMessage::new(
                    SystemMessageId::SoulshotsAndSpiritshotsAreNotAvailableForADeadPetOrServitorSadIsnTIt,
                ),
            );
            return false;
        }
        let Some(templates) = self.item_templates() else {
            return false;
        };
        let skill = templates
            .get(item.item_id)
            .and_then(|t| t.skills.item_skills.as_ref())
            .and_then(|skills| skills.first().copied());
        let Some(skill) = skill else {
            let name = if spirit { "BeastSpiritShot" } else { "BeastSoulShot" };
            warn!("{name}: is missing skills!");
            return false;
        };
        let item_id = item.item_id;
        let (consumption, shot, not_enough) = if spirit {
            let shot = if BLESSED_BEAST_SPIRITSHOTS.contains(&item_id) {
                ShotType::BlessedSpiritshots
            } else {
                ShotType::Spiritshots
            };
            (
                self.spirit_shots_per_hit(summon),
                shot,
                SystemMessageId::YouDonTHaveEnoughSpiritshotsNeededForAPetServitor,
            )
        } else {
            (
                self.soul_shots_per_hit(summon),
                ShotType::Soulshots,
                SystemMessageId::YouDonTHaveEnoughSoulshotsNeededForAPetServitor,
            )
        };
        // `shotConsumption` is a `short`.
        let consumption = i64::from(consumption as i16);
        if item.count < consumption {
            // Not enough shots to use.
            if !self.disable_auto_shot(owner, item_id) {
                self.send(owner, &SystemMessage::new(not_enough));
            }
            return false;
        }
        if self.is_charged_shot(summon, shot) {
            // Shots are already active.
            return false;
        }
        if !self.destroy_item_by_item_id(owner, item_id, consumption, false) {
            if !self.disable_auto_shot(owner, item_id) {
                self.send(owner, &SystemMessage::new(not_enough));
            }
            return false;
        }
        // NOTE(java-quirk): both handlers say "Your pet uses spiritshot.", the soulshot one too.
        self.send(owner, &SystemMessage::new(SystemMessageId::YourPetUsesSpiritshot));
        self.set_charged_shot(summon, shot, true);
        // `activeOwner.broadcastSkillPacket(new MagicSkillUse(summon, summon, ...), activeOwner)`:
        // the owner's broadcast, the owner first.
        if let Some((x, y, z)) = self.position_of(summon) {
            let here = SkillPosition { x, y, z };
            let packet = MagicSkillUse {
                creature_id: summon.0,
                target_id: summon.0,
                skill_id: skill.id,
                skill_level: skill.level,
                hit_time: 0,
                reuse_delay: 0,
                creature: here,
                ground_location: None,
                target: here,
            };
            self.broadcast_creature_packet(owner, &packet);
        }
        true
    }

    /// The first item of the owner with this id in the inventory (`getItemByItemId`).
    fn first_owned_item(&self, owner: ObjectId, item_id: i32) -> Option<ItemData> {
        self.players
            .get(&owner)?
            .data
            .items
            .iter()
            .find(|i| i.item_id == item_id && (i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL))
            .cloned()
    }

    /// Java `Summon.rechargeShots(physical, magic)`: the owner's automatic summon shots are used
    /// for the summon; a shot the owner no longer has is switched off.
    pub(crate) fn summon_recharge_shots(&mut self, summon: ObjectId, physical: bool, magic: bool) {
        let Some(owner) = self.summon_owner(summon) else {
            return;
        };
        let Some(templates) = self.item_templates() else {
            return;
        };
        for item_id in self.auto_soul_shots(owner) {
            let Some(item) = self.first_owned_item(owner, item_id) else {
                self.remove_auto_soul_shot(owner, item_id);
                continue;
            };
            let Some(template) = templates.get(item.item_id) else {
                continue;
            };
            let handler = template.etc().and_then(|e| e.handler.as_deref());
            if magic && template.default_action == ActionType::SummonSpiritShot {
                self.use_beast_shot_handler(owner, &item, handler);
            }
            if physical && template.default_action == ActionType::SummonSoulShot {
                self.use_beast_shot_handler(owner, &item, handler);
            }
        }
    }

    /// `ItemHandler.getHandler(item.getEtcItem())` + `onItemUse(_owner, item, false)` for the
    /// beast shot handlers.
    fn use_beast_shot_handler(&mut self, owner: ObjectId, item: &ItemData, handler: Option<&str>) {
        match handler {
            Some("BeastSoulShot") => {
                self.use_beast_shot(owner, item, false);
            }
            Some("BeastSpiritShot") => {
                self.use_beast_shot(owner, item, true);
            }
            _ => {}
        }
    }

    /// The beast shot branch of `RequestAutoSoulShot` (switching on): `true` when it handled the
    /// request (the caller returns).
    pub(crate) fn auto_soul_shot_beast_on(&mut self, owner: ObjectId, item: &ItemData, shot_type: i32) -> bool {
        if !BEAST_SHOT_IDS.contains(&item.item_id) {
            return false;
        }
        let Some(summon) = self.summon_of(owner) else {
            self.send(
                owner,
                &SystemMessage::new(SystemMessageId::YouDoNotHaveAServitorOrPetAndThereforeCannotUseTheAutomaticUseFunction),
            );
            return true;
        };
        let Some(templates) = self.item_templates() else {
            return true;
        };
        let handler = templates
            .get(item.item_id)
            .and_then(|t| t.etc())
            .and_then(|e| e.handler.clone());
        // NOTE(java-quirk): "not enough" is the soulshot message for both kinds.
        let per_hit = if handler.as_deref() == Some("BeastSoulShot") {
            self.soul_shots_per_hit(summon)
        } else {
            self.spirit_shots_per_hit(summon)
        };
        if i64::from(per_hit) > item.count {
            self.send(
                owner,
                &SystemMessage::new(SystemMessageId::YouDonTHaveEnoughSoulshotsNeededForAPetServitor),
            );
            return true;
        }
        self.add_auto_soul_shot(owner, item.item_id);
        self.send(
            owner,
            &ExAutoSoulShot {
                item_id: item.item_id,
                shot_type,
            },
        );
        // Start the auto soulshot use.
        self.send(
            owner,
            &SystemMessage::new(SystemMessageId::TheAutomaticUseOfS1HasBeenActivated)
                .with(crate::inventory::item_name_param(Some(&templates), item.item_id)),
        );
        self.recharge_shots(owner, true, true);
        self.summon_recharge_shots(summon, true, true);
        true
    }
}
