//! The outposts and flags of the war: `OutpostCreate`, `OutpostDestroy`, `TakeTerritoryFlag`,
//! the `canCreateOutpost` condition, and what a siege flag of the war is (no siege, no talking).
// Java: dist/game/data/scripts/handlers/skill/effects/{OutpostCreate,OutpostDestroy,
//       TakeTerritoryFlag}.java
// Java: mechanics/conditions/ConditionPlayerCanCreateOutpost.java
// Java: entity/actor/instance/SiegeFlag.java (the `isTWInProgress()` branch of the constructor,
//       `doDie`'s `removeClanFlag`)
// Java: managers/TerritoryWarManager.java (setHQForClan, addClanFlag, removeClanFlag)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use super::{OUTPOST_NPC_ID, TERRITORY_FLAG_NPC_ID};
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::siege::SiegeNpc;
use crate::world::World;
use crate::zones::ZoneId;

impl World {
    /// Java `setHQForClan(clan, hq)`: the outpost of the clan's territory (a castle owner only).
    pub(crate) fn territory_set_hq_for_clan(&mut self, clan_id: i32, hq: Option<ObjectId>) {
        let castle = self.clan(clan_id).map_or(0, |c| c.castle_id);
        if castle > 0
            && let Some(t) = self.territory.territories.get_mut(&castle)
        {
            t.hq = hq;
        }
    }

    /// `new SiegeFlag(player, template, advanced, outPost)` while the war runs: the flag of the
    /// clan belongs to no siege and never warns its clan; an outpost is invulnerable and not
    /// advanced. Spawned 50 above the player with the clan's name as its title.
    pub(crate) fn territory_spawn_flag(
        &mut self,
        player: ObjectId,
        npc_id: i32,
        advanced: bool,
        outpost: bool,
    ) -> Option<ObjectId> {
        let clan_id = self.clan_of(player).map(|c| c.id)?;
        let (x, y, z) = self.position_of(player)?;
        let heading = self.heading_of(player);
        let flag = self.spawn_npc_at(npc_id, x, y, z + 50, heading)?;
        let invul = outpost;
        self.sieges.npcs.insert(
            flag,
            SiegeNpc::Flag {
                clan_id,
                siege: None,
                advanced: advanced && !outpost,
                can_talk: false,
                invul,
            },
        );
        self.set_invul(flag, invul);
        let name = self.clan(clan_id).map(|c| c.name.clone()).unwrap_or_default();
        self.sieges.npc_titles.insert(flag, name);
        // The title changed after the spawn sent the NPC: show it again.
        for viewer in self.visible_players(flag) {
            self.send_npc_info(flag, viewer);
        }
        Some(flag)
    }

    /// Java `OutpostCreate.onStart`: during the war the clan leader's outpost (invulnerable)
    /// stands 50 above him and is his territory's headquarters.
    pub fn territory_create_outpost(&mut self, player: ObjectId) {
        if !self.is_clan_leader(player) || !self.is_tw_in_progress() {
            return;
        }
        let Some(clan) = self.clan_of(player).map(|c| c.id) else {
            return;
        };
        // Spawn a new flag.
        let flag = self.territory_spawn_flag(player, OUTPOST_NPC_ID, true, true);
        self.territory_set_hq_for_clan(clan, flag);
    }

    /// Java `OutpostDestroy.onStart`: during the war the clan leader takes the outpost down.
    pub fn territory_destroy_outpost(&mut self, player: ObjectId) {
        if !self.is_clan_leader(player) || !self.is_tw_in_progress() {
            return;
        }
        let Some(clan) = self.clan_of(player).map(|c| c.id) else {
            return;
        };
        if let Some(flag) = self.territory_hq_for_clan(clan) {
            self.delete_npc(flag);
        }
        self.territory_set_hq_for_clan(clan, None);
    }

    /// Java `TakeTerritoryFlag.onStart`: during the war the clan leader displays the clan's flag.
    pub fn territory_take_flag(&mut self, player: ObjectId) {
        if !self.is_clan_leader(player) || !self.is_tw_in_progress() {
            return;
        }
        let Some(clan) = self.clan_of(player).map(|c| c.id) else {
            return;
        };
        // Spawn a new flag.
        if let Some(flag) = self.territory_spawn_flag(player, TERRITORY_FLAG_NPC_ID, false, false) {
            // `addClanFlag(clan, flag)`: a second flag replaces the first in the map.
            self.territory.clan_flags.insert(clan, flag);
        }
    }

    /// `SiegeFlag.doDie` of a flag of the war: `removeClanFlag(_clan)`.
    // NOTE(java-quirk): it removes the clan's `TakeTerritoryFlag` flag from the list whichever
    // flag of the clan died (an outpost or a headquarters of the war too); the flag itself stays.
    pub(crate) fn territory_remove_clan_flag(&mut self, clan_id: i32) {
        self.territory.clan_flags.remove(&clan_id);
    }

    /// Java `ConditionPlayerCanCreateOutpost.testImpl` for a player: whether the clan leader may
    /// build an outpost or a flag here (with the messages that say why not).
    pub fn cond_can_create_outpost(&self, player: ObjectId) -> bool {
        let clan = self.clan_of(player).map(|c| c.id);
        // `isCursedWeaponEquipped()`: cursed weapons are M8.
        let mut can = !(self.is_alike_dead(player) || clan.is_none());
        let (castle, fort) = match self.position_of(player) {
            Some((x, y, z)) => (self.castle_at(x, y, z), self.fort_at(x, y, z)),
            None => (None, None),
        };
        if castle.is_none() && fort.is_none() {
            can = false;
        }
        let castle_active = |c: crate::residence::CastleId| {
            self.castle(c)
                .and_then(|c| c.siege_zone)
                .is_some_and(|z| self.is_siege_zone_active(z))
        };
        if fort.is_some_and(|f| f.0 == 0) || castle.is_some_and(|c| c.0 == 0) {
            self.send_message(
                player,
                "You must be on fort or castle ground to construct an outpost or flag.",
            );
            can = false;
        } else if fort.is_some_and(|f| !self.fort_zone_is_active(f))
            || castle.is_some_and(|c| !castle_active(c))
        {
            self.send_message(
                player,
                "You can only construct an outpost or flag on siege field.",
            );
            can = false;
        } else if !self.is_clan_leader(player) {
            self.send_message(
                player,
                "You must be a clan leader to construct an outpost or flag.",
            );
            can = false;
        } else if clan.is_some_and(|c| self.territory_hq_for_clan(c).is_some()) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::AnOutpostOrHeadquartersCannotBeBuiltBecauseOneAlreadyExists,
                ),
            );
            can = false;
        } else if clan.is_some_and(|c| self.territory_flag_for_clan(c).is_some()) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::AFlagIsAlreadyBeingDisplayedAnotherFlagCannotBeDisplayed,
                ),
            );
            can = false;
        } else if !self.is_inside_zone(player, ZoneId::Hq) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouCannotSetUpABaseHere),
            );
            can = false;
        }
        can
    }
}

/// Java `OutpostCreate`: the clan leader of a castle owner builds the territory's outpost.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct OutpostCreate;

impl OutpostCreate {
    pub(crate) fn create(
        _template: &l2_data::skill::EffectTemplate,
    ) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(OutpostCreate))
    }
}

impl EffectHandler for OutpostCreate {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        // `effector.asPlayer()`.
        if let Some(player) = world.acting_player(cx.effector) {
            world.territory_create_outpost(player);
        }
    }
}

/// Java `OutpostDestroy`: the clan leader takes the outpost of his territory down.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct OutpostDestroy;

impl OutpostDestroy {
    pub(crate) fn create(
        _template: &l2_data::skill::EffectTemplate,
    ) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(OutpostDestroy))
    }
}

impl EffectHandler for OutpostDestroy {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if let Some(player) = world.acting_player(cx.effector) {
            world.territory_destroy_outpost(player);
        }
    }
}

/// Java `TakeTerritoryFlag`: the clan leader displays the clan's flag of the war.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TakeTerritoryFlag;

impl TakeTerritoryFlag {
    pub(crate) fn create(
        _template: &l2_data::skill::EffectTemplate,
    ) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(TakeTerritoryFlag))
    }
}

impl EffectHandler for TakeTerritoryFlag {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if let Some(player) = world.acting_player(cx.effector) {
            world.territory_take_flag(player);
        }
    }
}
