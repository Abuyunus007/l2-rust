//! What the rift invasions (L2Rust's own feature, D-64, script `custom/RiftInvasion`) need from
//! the world: the monsters' own rewards, their level and HP, the bound rift shard, and the heals
//! done while a rift is open. The script owns the rift itself.

use l2_data::RiftZoneDef;
use l2_util::ObjectId;

use crate::world::World;

/// The rewards of one monster instead of its template's (a rift monster).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NpcRewardOverride {
    /// Factor on the exp the monster gives (`RiftMobsExpRate`).
    pub exp_rate: f64,
    /// Factor on the SP the monster gives (`RiftMobsSpRate`).
    pub sp_rate: f64,
    /// The monster drops its items and can be swept (`RiftMobsNormalDrops`).
    pub drops: bool,
}

impl World {
    /// The zones of `data/RiftInvasion.xml`.
    pub fn rift_invasion_zones(&self) -> &[RiftZoneDef] {
        &self.custom_scripts.settings.rift_zones
    }

    /// Gives the NPC its own rewards (`None`: the template's again). A spawned NPC starts with
    /// none, even when its object id was used before.
    pub fn set_npc_reward_override(&mut self, npc: ObjectId, value: Option<NpcRewardOverride>) {
        match value {
            Some(value) => {
                self.custom_scripts.reward_overrides.insert(npc, value);
            }
            None => {
                self.custom_scripts.reward_overrides.remove(&npc);
            }
        }
    }

    /// The NPC's own rewards, if a script gave it some.
    pub fn npc_reward_override(&self, npc: ObjectId) -> Option<NpcRewardOverride> {
        self.custom_scripts.reward_overrides.get(&npc).copied()
    }

    /// Gives a monster `level` and `hp_factor` times its max HP (full HP), keeping the attack of
    /// its template (the level modifier still applies, as for any NPC).
    pub fn scale_monster(&mut self, npc: ObjectId, level: i32, hp_factor: f32) {
        let Some((p_atk, m_atk)) = self
            .stats(npc)
            .and_then(|v| v.npc_template())
            .map(|t| (t.base_p_atk, t.base_m_atk))
        else {
            return;
        };
        self.set_npc_scale(
            npc,
            crate::NpcScale {
                level,
                hp: hp_factor,
                mp: 1.0,
                p_atk,
                m_atk,
                defence: 1.0,
            },
        );
    }

    /// The item is the bound rift shard (`RiftShardBound`): it cannot be traded, dropped, sold in
    /// a private store, mailed, sent by freight or put into the clan warehouse; the own warehouse
    /// takes it.
    pub fn is_l2r_bound_item(&self, item_id: i32) -> bool {
        self.custom_scripts.l2r.rift.bound_item() == Some(item_id)
    }

    /// Starts (`true`) or stops recording the HP heals of skills (`Heal`, `HealPercent`); stopping
    /// forgets what was recorded.
    pub fn set_heal_ledger(&mut self, on: bool) {
        self.custom_scripts.heal_ledger = on.then(Vec::new);
    }

    /// The heals recorded since the last call: `(healer, healed, hp)`.
    pub fn take_heal_ledger(&mut self) -> Vec<(ObjectId, ObjectId, f64)> {
        self.custom_scripts
            .heal_ledger
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    /// A skill of `healer` restored `amount` HP of `healed` (recorded while the ledger is on).
    pub fn note_heal(&mut self, healer: ObjectId, healed: ObjectId, amount: f64) {
        if amount <= 0.0 {
            return;
        }
        if let Some(ledger) = self.custom_scripts.heal_ledger.as_mut() {
            // A rift lasts minutes; the script drains this every second. The bound only guards
            // against a ledger left on by mistake.
            if ledger.len() < 100_000 {
                ledger.push((healer, healed, amount));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::WorldRng;

    use super::*;
    use crate::outbox::{LsEvent, Outbox, PersistCommand};

    struct Quiet;

    impl Outbox for Quiet {
        fn persist(&mut self, _command: PersistCommand) {}
        fn ls_event(&mut self, _event: LsEvent) {}
    }

    fn world() -> World {
        World::new(WorldRng::from_seed(1), Box::new(Quiet))
    }

    #[test]
    fn the_heal_ledger_records_only_while_it_is_on() {
        let mut world = world();
        world.note_heal(ObjectId(1), ObjectId(2), 50.0);
        assert!(world.take_heal_ledger().is_empty());
        world.set_heal_ledger(true);
        world.note_heal(ObjectId(1), ObjectId(2), 50.0);
        world.note_heal(ObjectId(1), ObjectId(2), 0.0);
        assert_eq!(world.take_heal_ledger(), vec![(ObjectId(1), ObjectId(2), 50.0)]);
        assert!(world.take_heal_ledger().is_empty());
        world.set_heal_ledger(false);
        world.note_heal(ObjectId(1), ObjectId(2), 50.0);
        assert!(world.take_heal_ledger().is_empty());
    }

    #[test]
    fn only_the_bound_shard_is_bound() {
        let mut world = world();
        assert!(!world.is_l2r_bound_item(4405));
        let mut settings = world.l2r_settings().clone();
        settings.rift.shard_item_id = 4405;
        world.set_l2r_settings(settings.clone());
        assert!(world.is_l2r_bound_item(4405));
        assert!(!world.is_l2r_bound_item(57));
        settings.rift.shard_bound = false;
        world.set_l2r_settings(settings);
        assert!(!world.is_l2r_bound_item(4405));
    }

    #[test]
    fn the_bound_shard_is_not_tradeable_but_goes_into_the_own_warehouse() {
        let mut world = world();
        let templates = l2_data::ItemTemplates::from_xml(
            r#"<list><item id="4405" type="EtcItem" name="Shard"><set name="is_stackable" val="true" /></item></list>"#,
        )
        .unwrap();
        let template = templates.get(4405).unwrap();
        let item = crate::player::ItemData {
            object_id: ObjectId(500),
            item_id: 4405,
            count: 10,
            loc: "INVENTORY".to_owned(),
            mana_left: -1,
            time: -1,
            ..Default::default()
        };
        // Not bound: trade and both warehouses.
        assert!(world.item_is_tradeable(&item, template));
        assert!(world.item_is_depositable(&item, template, false));
        let mut settings = world.l2r_settings().clone();
        settings.rift.shard_item_id = 4405;
        world.set_l2r_settings(settings);
        // Bound: no trade, private store or mail (they ask `item_is_tradeable`), no clan
        // warehouse; the own warehouse takes it.
        assert!(!world.item_is_tradeable(&item, template));
        assert!(!world.item_is_depositable(&item, template, false));
        assert!(world.item_is_depositable(&item, template, true));
    }

    #[test]
    fn a_reward_override_is_kept_until_removed() {
        let mut world = world();
        let value = NpcRewardOverride {
            exp_rate: 2.0,
            sp_rate: 0.5,
            drops: false,
        };
        world.set_npc_reward_override(ObjectId(9), Some(value));
        assert_eq!(world.npc_reward_override(ObjectId(9)), Some(value));
        world.set_npc_reward_override(ObjectId(9), None);
        assert_eq!(world.npc_reward_override(ObjectId(9)), None);
    }
}
