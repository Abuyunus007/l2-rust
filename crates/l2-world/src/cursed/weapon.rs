//! The behaviour of a cursed weapon: Java `CursedWeapon` and the entry points of
//! `CursedWeaponsManager`.
// Java: entity/actor/holders/player/CursedWeapon.java
// Java: managers/CursedWeaponsManager.java

use l2_data::CursedWeaponDefs;
use l2_data::item::types::BodyPart;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{Earthquake, ExRedSky, SocialAction, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{info, warn};

use super::{
    AKAMANAH_ID, CURSED_KARMA, CursedLoad, CursedSettings, CursedTimer, CursedWeapon, CursedWrite,
    VOID_BURST, VOID_FLOW, ZARICHE_ID,
};
use crate::inventory::item_name_param;
use crate::outbox::PersistCommand;
use crate::party::PartyMessageType;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

/// `Rnd.get(100000) < _dropRate`: the drop rate is out of 100000.
const DROP_RATE_SCALE: i32 = 100_000;
/// `ThreadPool.schedule(() -> _player.broadcastPacket(new SocialAction(.., 17)), 300)`.
const SOCIAL_DELAY_MS: u64 = 300;
/// `ThreadPool.schedule(() -> TransformData.getInstance().transformPlayer(..), 500)`.
const TRANSFORM_DELAY_MS: u64 = 500;
/// `SocialAction(objectId, 17)`: the victory pose.
const SOCIAL_ACTION_ID: i32 = 17;
/// The distance the `RemoveTask` runs at: `_durationLost * 12000` ms.
const REMOVE_TASK_MS_PER_MINUTE: i64 = 12_000;
/// `_duration * 60000` and `_durationLost * 60000`: minutes to milliseconds.
const MS_PER_MINUTE: i32 = 60_000;

impl World {
    // ------------------------------------------------------------------------ settings, load

    /// Installs the cursed weapon settings. Call before [`World::load_cursed_weapons`].
    pub fn set_cursed_settings(&mut self, settings: CursedSettings) {
        self.cursed.settings = settings;
    }

    /// Java `CursedWeaponsManager.init()` without `controlPlayers` (the game server runs that
    /// against the database before the world starts): the weapons of `CursedWeapons.xml`, then
    /// `restore()` re-activates the ones the table says a character holds.
    ///
    /// With `AllowCursedWeapons` off nothing loads. The rows are only read with
    /// `CycleDatabaseWrites` (D-53): without it the table is Sieghardt's.
    pub fn load_cursed_weapons(&mut self, defs: &CursedWeaponDefs, load: CursedLoad) {
        let old: Vec<_> = self.cursed.weapons.iter().filter_map(|w| w.remove_task).collect();
        for timer in old {
            self.timers.cancel(timer);
        }
        self.cursed.weapons.clear();
        if !self.cursed.settings.allow_cursed_weapons {
            return;
        }
        for def in defs.iter() {
            let max_level = self
                .data
                .stat_data()
                .map_or(0, |data| data.skills.max_level(def.skill_id));
            self.cursed.weapons.push(CursedWeapon::from_def(def, max_level));
        }
        // restore()
        if self.cursed.settings.cycle_database_writes {
            for row in load.rows {
                let Some(index) = self.cursed.index_of(row.item_id) else {
                    // NOTE(java-quirk): `_cursedWeapons.get(itemId)` is null for an item the XML
                    // does not list; the NullPointerException ends the whole `restore()`.
                    warn!(
                        "Could not restore CursedWeapons data: unknown item {}.",
                        row.item_id
                    );
                    break;
                };
                let weapon = &mut self.cursed.weapons[index];
                weapon.player_id = row.char_id;
                weapon.player_karma = row.player_karma;
                weapon.player_pk_kills = row.player_pk_kills;
                weapon.nb_kills = row.nb_kills;
                weapon.end_time = row.end_time;
                self.cursed_re_activate(index);
            }
        }
        info!(
            "CursedWeaponsManager: Loaded : {} cursed weapon(s).",
            self.cursed.weapons.len()
        );
    }

    /// Java `System.currentTimeMillis()` for the cursed weapons (see
    /// [`CursedSettings::clock_override_unix_ms`]).
    pub(crate) fn cursed_now_ms(&self) -> i64 {
        match self.cursed.settings.clock_override_unix_ms {
            Some(base) => base.saturating_add(self.world_ms()),
            None => unix_ms_now(),
        }
    }

    // ------------------------------------------------------------------------ what the rest asks

    /// Java `CursedWeaponsManager.getCursedWeapon(itemId)`.
    pub fn cursed_weapon(&self, item_id: i32) -> Option<&CursedWeapon> {
        self.cursed.weapon(item_id)
    }

    /// Java `CursedWeaponsManager.getCursedWeapons()`, in the order of Java's `HashMap`.
    pub fn cursed_weapons(&self) -> &[CursedWeapon] {
        self.cursed.weapons()
    }

    /// Java `CursedWeaponsManager.isCursed(itemId)`.
    pub fn is_cursed_item(&self, item_id: i32) -> bool {
        self.cursed.index_of(item_id).is_some()
    }

    /// Java `Player.getCursedWeaponEquippedId()`: 0 for none.
    pub fn cursed_weapon_equipped_id(&self, id: ObjectId) -> i32 {
        self.players.get(&id).map_or(0, |p| p.cursed_weapon_equipped_id)
    }

    /// Java `Player.isCursedWeaponEquipped()`.
    pub fn is_cursed_weapon_equipped(&self, id: ObjectId) -> bool {
        self.cursed_weapon_equipped_id(id) != 0
    }

    /// `isCursedWeaponEquipped() ? CursedWeaponsManager.getLevel(id) : 0` of `UserInfo` and
    /// `CharInfo`.
    pub(crate) fn cursed_weapon_level_of(&self, id: ObjectId) -> i32 {
        let item_id = self.cursed_weapon_equipped_id(id);
        if item_id == 0 {
            return 0;
        }
        // Java: `_cursedWeapons.get(itemId).getLevel()`; a missing weapon is a null pointer.
        self.cursed.weapon(item_id).map_or(0, CursedWeapon::level)
    }

    /// `PlayableAI.setIntentionAttack`: a cursed weapon owner and a character of level 20 or less
    /// keep away from each other (either way).
    pub(crate) fn cursed_weapon_refuses(&self, player: ObjectId, target: ObjectId) -> bool {
        let (Some(p), Some(t)) = (self.players.get(&player), self.players.get(&target)) else {
            return false;
        };
        (t.cursed_weapon_equipped_id != 0 && p.core.level <= 20)
            || (p.cursed_weapon_equipped_id != 0 && t.core.level <= 20)
    }

    /// Java `CursedWeaponsManager.getCursedWeaponsIds()`.
    pub fn cursed_weapon_ids(&self) -> Vec<i32> {
        self.cursed.weapons.iter().map(|w| w.item_id).collect()
    }

    /// Java `CursedWeapon.getWorldPosition()`.
    pub fn cursed_world_position(&self, item_id: i32) -> Option<(i32, i32, i32)> {
        let weapon = self.cursed.weapon(item_id)?;
        if weapon.is_activated
            && let Some(player) = weapon.player
        {
            return self.position_of(player);
        }
        if weapon.is_dropped
            && let Some(item) = weapon.item
        {
            return self.ground.items.get(&item).map(|i| (i.x, i.y, i.z));
        }
        None
    }

    /// Java `CursedWeaponsManager.checkOwnsWeaponId(ownerId)`.
    pub fn cursed_owned_weapon_id(&self, owner_id: i32) -> i32 {
        self.cursed
            .weapons
            .iter()
            .find(|w| w.is_activated && w.player_id == owner_id)
            .map_or(-1, |w| w.item_id)
    }

    // ------------------------------------------------------------------------ the drop

    /// Java `CursedWeaponsManager.checkDrop(attackable, player)`: called when a monster dies.
    pub(crate) fn cursed_check_drop(&mut self, attackable: ObjectId, player: ObjectId) {
        if let Some(npc) = self.npcs.get(&attackable) {
            use crate::npc::NpcClass;
            // Defender, RiftInvader, FestivalMonster, Guard, GrandBoss, FeedableBeast and
            // FortCommander (and their subclasses) never drop one.
            if matches!(
                npc.class,
                NpcClass::Defender
                    | NpcClass::FortCommander
                    | NpcClass::Guard
                    | NpcClass::QuestGuard
                    | NpcClass::GrandBoss
            ) {
                return;
            }
            if let Some(template) = self.npc_world.template(npc.npc_id)
                && (template.is_type("RiftInvader")
                    || template.is_type("FestivalMonster")
                    || template.is_type("FeedableBeast"))
            {
                return;
            }
        }
        for index in 0..self.cursed.weapons.len() {
            if self.cursed.weapons[index].is_active() {
                continue;
            }
            if self.cursed_weapon_check_drop(index, attackable, player) {
                break;
            }
        }
    }

    /// Java `CursedWeapon.checkDrop(attackable, player)`.
    fn cursed_weapon_check_drop(&mut self, index: usize, attackable: ObjectId, player: ObjectId) -> bool {
        // D-53: the weapon is global, so this server drops none while Sieghardt owns the table;
        // the chance is not even rolled.
        if !self.cursed.settings.cycle_database_writes {
            return false;
        }
        let drop_rate = self.cursed.weapons[index].drop_rate;
        if self.rng().below(DROP_RATE_SCALE) >= drop_rate {
            return false;
        }
        // Drop the item.
        if !self.cursed_drop_from_monster(index, attackable, player) {
            return false;
        }
        // Start the Life Task.
        let weapon = &self.cursed.weapons[index];
        // `System.currentTimeMillis() + (_duration * 60000)`: the product is an `int`.
        let life = i64::from(weapon.duration.wrapping_mul(MS_PER_MINUTE));
        let end_time = self.cursed_now_ms().saturating_add(life);
        self.cursed.weapons[index].end_time = end_time;
        self.cursed_schedule_remove_task(index);
        true
    }

    /// Java `CursedWeapon.dropIt(attackable, player)`: the weapon lands next to the monster, the
    /// sky turns red and everybody is told where.
    fn cursed_drop_from_monster(&mut self, index: usize, attackable: ObjectId, player: ObjectId) -> bool {
        let item_id = self.cursed.weapons[index].item_id;
        self.cursed.weapons[index].is_activated = false;
        let Some(item) = self.npc_drop_item(attackable, Some(player), item_id, 1) else {
            // Java would throw a NullPointerException on `_item.setDropTime(0)`.
            warn!(
                "{}: the monster could not drop the cursed weapon {item_id}.",
                attackable
            );
            return false;
        };
        // `_item.setDropTime(0)`: prevent the item from being removed by ItemsAutoDestroy.
        if let Some(ground) = self.ground_item_mut(item) {
            ground.drop_time_ms = None;
        }
        self.ground.auto_destroy.remove(&item);
        self.cursed.weapons[index].item = Some(item);

        // RedSky and Earthquake.
        let (x, y, z) = self.position_of(player).unwrap_or_default();
        self.broadcast_to_all_online_players(&ExRedSky { duration: 10 });
        self.broadcast_to_all_online_players(&Earthquake {
            x,
            y,
            z,
            intensity: 14,
            duration: 3,
        });

        self.cursed.weapons[index].is_dropped = true;
        self.cursed_announce_dropped(item_id, (x, y, z));
        true
    }

    /// `S2_WAS_DROPPED_IN_THE_S1_REGION` to everybody.
    fn cursed_announce_dropped(&self, item_id: i32, (x, y, z): (i32, i32, i32)) {
        let templates = self.item_templates();
        let message = SystemMessage::new(SystemMessageId::S2WasDroppedInTheS1Region)
            .with(SystemMessageParam::ZoneName { x, y, z })
            .with(item_name_param(templates.as_deref(), item_id));
        self.broadcast_to_all_online_players(&message);
    }

    // ------------------------------------------------------------------------ the pickup

    /// Java `CursedWeaponsManager.activate(player, item)`: the weapon came into the inventory of
    /// `player` (a pickup, a GM's `//cw_add`, any `addItem`).
    pub(crate) fn cursed_activate(&mut self, player: ObjectId, item: ObjectId, item_id: i32) {
        let Some(index) = self.cursed.index_of(item_id) else {
            return;
        };
        let equipped_id = self.cursed_weapon_equipped_id(player);
        if equipped_id != 0 {
            // Cannot own 2 cursed swords.
            let Some(other) = self.cursed.index_of(equipped_id) else {
                return;
            };
            // The bonus level: "_stageKills" levels are added to the owner of the first weapon
            // (a temp fix of Java's, kept as it is).
            let stage_kills = self.cursed.weapons[other].stage_kills;
            self.cursed.weapons[other].nb_kills = stage_kills.wrapping_sub(1);
            self.cursed_increase_kills_of(other);
            // Erase the newly obtained cursed weapon: `setPlayer(player)` is NECESSARY to find
            // which inventory the weapon is in.
            self.cursed.weapons[index].player = Some(player);
            self.cursed_end_of_life(index);
        } else {
            self.cursed_weapon_activate(index, player, item);
        }
    }

    /// Java `CursedWeapon.activate(player, item)`.
    fn cursed_weapon_activate(&mut self, index: usize, player: ObjectId, item: ObjectId) {
        // If the player is mounted, attempt to unmount first. Only allow picking up the cursed
        // weapon if unmounting is successful.
        if self.is_mounted(player) && !self.dismount(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveFailedToPickUpS1),
            );
            self.player_drop_item_object(player, item, false, true);
            return;
        }

        let item_id = self.cursed.weapons[index].item_id;
        let Some((karma, pk_kills)) = self.players.get(&player).map(|p| (p.data.karma, p.data.pk_kills))
        else {
            return;
        };
        let weapon = &mut self.cursed.weapons[index];
        weapon.is_activated = true;
        // Player holding it data.
        weapon.player = Some(player);
        weapon.player_id = player.0;
        weapon.player_karma = karma;
        weapon.player_pk_kills = pk_kills;
        self.cursed_save_data(index);

        // Change player stats.
        if let Some(p) = self.players.get_mut(&player) {
            p.cursed_weapon_equipped_id = item_id;
        }
        self.set_karma(player, CURSED_KARMA);
        if let Some(p) = self.players.get_mut(&player) {
            p.data.pk_kills = 0;
        }
        if let Some(party) = self.party_of(player) {
            self.remove_party_member(party, player, PartyMessageType::Expelled);
        }

        // Disable All Skills. Do Transform.
        self.cursed_do_transform(index);
        // Add skill.
        self.cursed_give_skill(index);

        // Equip with the weapon.
        self.cursed.weapons[index].item = Some(item);
        let mut changed = Vec::new();
        self.equip_item(player, item, &mut changed);
        let templates = self.item_templates();
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouHaveEquippedYourS1)
                .with(item_name_param(templates.as_deref(), item_id)),
        );

        // Fully heal player.
        let (max_hp, max_mp, max_cp) = match self.stats(player) {
            Some(view) => (
                f64::from(view.max_hp()),
                f64::from(view.max_mp()),
                f64::from(view.max_cp()),
            ),
            None => return,
        };
        self.set_current_hp_mp(player, max_hp, max_mp);
        self.set_current_cp(player, max_cp, true);

        // Refresh inventory.
        self.send_item_list(player, false);
        // Refresh player stats.
        self.broadcast_user_info(player);

        let due = self.tick + TimerWheel::ms_to_ticks(SOCIAL_DELAY_MS);
        self.timers
            .schedule_at(due, TimerKind::Cursed(CursedTimer::Social(item_id)));

        let (x, y, z) = self.position_of(player).unwrap_or_default();
        let message = SystemMessage::new(SystemMessageId::TheOwnerOfS2HasAppearedInTheS1Region)
            .with(SystemMessageParam::ZoneName { x, y, z })
            .with(item_name_param(templates.as_deref(), item_id));
        self.broadcast_to_all_online_players(&message);
    }

    // ------------------------------------------------------------------------ the owner's skills

    /// Java `CursedWeapon.giveSkill()`: the passive skill at the level the kills earned and the
    /// two skills every owner has.
    fn cursed_give_skill(&mut self, index: usize) {
        let weapon = &self.cursed.weapons[index];
        let Some(player) = weapon.player else {
            return;
        };
        // `1 + (_nbKills / _stageKills)`, at most the skill's last level.
        let level =
            (1 + weapon.nb_kills.checked_div(weapon.stage_kills).unwrap_or(0)).min(weapon.skill_max_level);
        let skill_id = weapon.skill_id;
        if !self.add_skill(player, skill_id, level) {
            warn!("CursedWeapon: skill {skill_id} level {level} does not exist.");
        }
        // Void Burst, Void Flow.
        for (id, level) in [VOID_BURST, VOID_FLOW] {
            self.add_skill(player, id, level);
            if let Some(p) = self.players.get_mut(&player) {
                // `addTransformSkill(skill)`.
                p.transform.skills.insert(id, level);
            }
        }
        self.send_skill_list(player);
    }

    /// Java `CursedWeapon.removeSkill()`.
    fn cursed_remove_skill(&mut self, index: usize) {
        let weapon = &self.cursed.weapons[index];
        let Some(player) = weapon.player else {
            return;
        };
        let skill_id = weapon.skill_id;
        self.remove_skill(player, skill_id, true);
        self.remove_skill(player, VOID_BURST.0, true);
        self.remove_skill(player, VOID_FLOW.0, true);
        self.untransform(player);
        self.send_skill_list(player);
    }

    /// Java `CursedWeapon.doTransform()`.
    fn cursed_do_transform(&mut self, index: usize) {
        let weapon = &mut self.cursed.weapons[index];
        let item_id = weapon.item_id;
        if item_id == AKAMANAH_ID {
            weapon.transformation_id = 302;
        } else if item_id == ZARICHE_ID {
            weapon.transformation_id = 301;
        }
        let transformation_id = weapon.transformation_id;
        let Some(player) = weapon.player else {
            return;
        };
        if self.is_transformed(player) || self.is_in_stance(player) {
            self.stop_transformation(player, true);
            let due = self.tick + TimerWheel::ms_to_ticks(TRANSFORM_DELAY_MS);
            self.timers
                .schedule_at(due, TimerKind::Cursed(CursedTimer::Transform(item_id)));
        } else {
            self.transform_player(transformation_id, player);
        }
    }

    // ------------------------------------------------------------------------ kills

    /// Java `CursedWeaponsManager.increaseKills(itemId)`.
    pub(crate) fn cursed_increase_kills(&mut self, item_id: i32) {
        if let Some(index) = self.cursed.index_of(item_id) {
            self.cursed_increase_kills_of(index);
        }
    }

    /// Java `CursedWeapon.increaseKills()`.
    fn cursed_increase_kills_of(&mut self, index: usize) {
        let weapon = &mut self.cursed.weapons[index];
        weapon.nb_kills = weapon.nb_kills.wrapping_add(1);
        let (nb_kills, stage_kills, max_level) =
            (weapon.nb_kills, weapon.stage_kills, weapon.skill_max_level);
        let player = weapon.player;

        if let Some(player) = player.filter(|p| self.players.contains_key(p)) {
            if let Some(p) = self.players.get_mut(&player) {
                p.data.pk_kills = nb_kills;
            }
            self.send_user_info(player);
            // `(_nbKills % _stageKills) == 0 && _nbKills <= _stageKills * (_skillMaxLevel - 1)`.
            let on_stage = nb_kills.checked_rem(stage_kills) == Some(0);
            if on_stage && nb_kills <= stage_kills.wrapping_mul(max_level - 1) {
                self.cursed_give_skill(index);
            }
        }

        // Reduce time-to-live.
        let weapon = &mut self.cursed.weapons[index];
        weapon.end_time -= i64::from(weapon.duration_lost.wrapping_mul(MS_PER_MINUTE));
        self.cursed_save_data(index);
    }

    // ------------------------------------------------------------------------ the owner dies

    /// Java `CursedWeaponsManager.drop(itemId, killer)`: the owner died.
    pub(crate) fn cursed_drop(&mut self, item_id: i32) {
        let Some(index) = self.cursed.index_of(item_id) else {
            return;
        };
        let disapear_chance = self.cursed.weapons[index].disapear_chance;
        if self.rng().below(100) <= disapear_chance {
            // Remove it.
            self.cursed_end_of_life(index);
            return;
        }
        // Unequip & drop.
        self.cursed_drop_from_player(index);
        // Reset player stats (again: Java does it in `dropIt` and here).
        let weapon = &self.cursed.weapons[index];
        let (player, karma, pk_kills) = (weapon.player, weapon.player_karma, weapon.player_pk_kills);
        let Some(player) = player else {
            return;
        };
        self.set_karma(player, karma);
        if let Some(p) = self.players.get_mut(&player) {
            p.data.pk_kills = pk_kills;
            p.cursed_weapon_equipped_id = 0;
        }
        self.cursed_remove_skill(index);
        self.abort_attack(player);
        self.broadcast_user_info(player);
    }

    /// Java `CursedWeapon.dropIt(null, null, killer, false)`.
    fn cursed_drop_from_player(&mut self, index: usize) {
        let weapon = &mut self.cursed.weapons[index];
        weapon.is_activated = false;
        let (item_id, karma, pk_kills, player) = (
            weapon.item_id,
            weapon.player_karma,
            weapon.player_pk_kills,
            weapon.player,
        );
        let Some(player) = player else {
            return;
        };
        let item = self
            .inventory_item_by_item_id(player, item_id)
            .map(|i| i.object_id);
        self.cursed.weapons[index].item = item;
        if let Some(item) = item {
            // `_player.dropItem(DEATH, _item, killer, true)`.
            self.player_drop_item_object(player, item, false, true);
        }
        self.set_karma(player, karma);
        if let Some(p) = self.players.get_mut(&player) {
            p.data.pk_kills = pk_kills;
            p.cursed_weapon_equipped_id = 0;
        }
        self.cursed_remove_skill(index);
        self.abort_attack(player);

        self.cursed.weapons[index].is_dropped = true;
        // `sm.addZoneName(_player.getX(), ...)`: where the owner died.
        let position = self.position_of(player).unwrap_or_default();
        self.cursed_announce_dropped(item_id, position);
    }

    // ------------------------------------------------------------------------ the end

    /// Java `CursedWeapon.endOfLife()`.
    pub(crate) fn cursed_end_of_life(&mut self, index: usize) {
        let weapon = &self.cursed.weapons[index];
        let (item_id, name, player, player_id) = (
            weapon.item_id,
            weapon.name.clone(),
            weapon.player,
            weapon.player_id,
        );
        let (karma, pk_kills) = (weapon.player_karma, weapon.player_pk_kills);
        let ground_item = weapon.item;
        let online = player.filter(|p| self.players.contains_key(p));
        if weapon.is_activated {
            if let Some(player) = online {
                // Remove from player.
                info!("{name} being removed online.");
                self.abort_attack(player);
                self.set_karma(player, karma);
                if let Some(p) = self.players.get_mut(&player) {
                    p.data.pk_kills = pk_kills;
                    p.cursed_weapon_equipped_id = 0;
                }
                self.cursed_remove_skill(index);

                // Remove.
                let mut changed = Vec::new();
                self.unequip_item_in_body_slot(player, BodyPart::BothHands, &mut changed);
                // `_player.storeMe()`.
                if let Some(snapshot) = self.store_snapshot(player) {
                    self.outbox
                        .persist(PersistCommand::StorePlayer(Box::new(snapshot)));
                }

                // Destroy.
                self.cursed_destroy_item(player, item_id);
                self.send_item_list(player, true);
                self.broadcast_user_info(player);
            } else {
                // Remove from Db.
                info!("{name} being removed offline.");
                self.outbox
                    .persist(PersistCommand::Cursed(CursedWrite::OfflineEnd {
                        player_id,
                        item_id,
                        karma,
                        pk_kills,
                    }));
            }
        }
        // Either this cursed weapon is in the inventory of someone who has another cursed weapon
        // equipped, OR this cursed weapon is on the ground.
        else if let Some(player) = player.filter(|p| self.inventory_item_by_item_id(*p, item_id).is_some())
        {
            // Destroy.
            self.cursed_destroy_item(player, item_id);
            self.send_item_list(player, true);
            self.broadcast_user_info(player);
        }
        // Is dropped on the ground.
        else if let Some(item) = ground_item {
            // `_item.decayMe()`.
            self.decay_ground_item(item);
            info!("{name} item has been removed from World.");
        }

        // Delete infos from table if any.
        self.cursed_remove_from_db(item_id);

        let templates = self.item_templates();
        let message = SystemMessage::new(SystemMessageId::S1HasDisappeared)
            .with(item_name_param(templates.as_deref(), item_id));
        self.broadcast_to_all_online_players(&message);

        // Reset state.
        self.cursed_cancel_task(index);
        let weapon = &mut self.cursed.weapons[index];
        weapon.is_activated = false;
        weapon.is_dropped = false;
        weapon.end_time = 0;
        weapon.player = None;
        weapon.player_id = 0;
        weapon.player_karma = 0;
        weapon.player_pk_kills = 0;
        weapon.item = None;
        weapon.nb_kills = 0;
    }

    /// `getInventory().destroyItemByItemId(NONE, itemId, 1, player, null)`: the inventory's own
    /// destroy, which tells nobody (the callers send the item list).
    fn cursed_destroy_item(&mut self, player: ObjectId, item_id: i32) {
        if let Some(item) = self
            .inventory_item_by_item_id(player, item_id)
            .map(|i| i.object_id)
        {
            self.destroy_inventory_item(player, item, 1);
        }
    }

    // ------------------------------------------------------------------------ the life timer

    /// Java `reActivate()`: the weapon is held (by a restored row or a GM's `//cw_add`).
    pub(crate) fn cursed_re_activate(&mut self, index: usize) {
        self.cursed.weapons[index].is_activated = true;
        if self.cursed.weapons[index].end_time - self.cursed_now_ms() <= 0 {
            self.cursed_end_of_life(index);
        } else {
            // NOTE(java-quirk): Java overwrites `_removeTask` without cancelling the old one,
            // which would then keep ending the weapon every period. The old task is cancelled.
            self.cursed_cancel_task(index);
            self.cursed_schedule_remove_task(index);
        }
    }

    /// `ThreadPool.scheduleAtFixedRate(new RemoveTask(), _durationLost * 12000, ...)`.
    pub(crate) fn cursed_schedule_remove_task(&mut self, index: usize) {
        self.cursed_cancel_task(index);
        let weapon = &self.cursed.weapons[index];
        let item_id = weapon.item_id;
        let period_ms =
            u64::try_from(i64::from(weapon.duration_lost) * REMOVE_TASK_MS_PER_MINUTE).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(period_ms);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Cursed(CursedTimer::Remove(item_id)));
        self.cursed.weapons[index].remove_task = Some(timer);
    }

    /// Java `cancelTask()`.
    fn cursed_cancel_task(&mut self, index: usize) {
        if let Some(timer) = self.cursed.weapons[index].remove_task.take() {
            self.timers.cancel(timer);
        }
    }

    /// A cursed weapon timer fired.
    pub(crate) fn on_cursed_timer(&mut self, timer: CursedTimer) {
        match timer {
            CursedTimer::Remove(item_id) => {
                let Some(index) = self.cursed.index_of(item_id) else {
                    return;
                };
                if self.cursed.weapons[index].remove_task.is_none() {
                    return;
                }
                // A fixed rate task: the next run is scheduled before this one works.
                self.cursed_schedule_remove_task(index);
                // `RemoveTask.run`.
                if self.cursed_now_ms() >= self.cursed.weapons[index].end_time {
                    self.cursed_end_of_life(index);
                }
            }
            CursedTimer::Social(item_id) => {
                let player = self.cursed.weapon(item_id).and_then(|w| w.player);
                if let Some(player) = player.filter(|p| self.players.contains_key(p)) {
                    self.broadcast_to_self_and_visible(
                        player,
                        &SocialAction {
                            object_id: player.0,
                            action_id: SOCIAL_ACTION_ID,
                        },
                    );
                }
            }
            CursedTimer::Transform(item_id) => {
                let Some(weapon) = self.cursed.weapon(item_id) else {
                    return;
                };
                let (player, transformation_id) = (weapon.player, weapon.transformation_id);
                if let Some(player) = player.filter(|p| self.players.contains_key(p)) {
                    self.transform_player(transformation_id, player);
                }
            }
        }
    }

    // ------------------------------------------------------------------------ the table

    /// Java `CursedWeapon.saveData()`: the row of a held weapon (D-53: only with
    /// `CycleDatabaseWrites`).
    pub(crate) fn cursed_save_data(&mut self, index: usize) {
        if !self.cursed.settings.cycle_database_writes {
            return;
        }
        let weapon = &self.cursed.weapons[index];
        let write = CursedWrite::Save {
            item_id: weapon.item_id,
            row: weapon.is_activated.then(|| weapon.row()),
        };
        self.outbox.persist(PersistCommand::Cursed(write));
    }

    /// Java `CursedWeaponsManager.removeFromDb(itemId)` (D-53: only with `CycleDatabaseWrites`).
    fn cursed_remove_from_db(&mut self, item_id: i32) {
        if self.cursed.settings.cycle_database_writes {
            self.outbox
                .persist(PersistCommand::Cursed(CursedWrite::Remove { item_id }));
        }
    }

    /// Java `CursedWeaponsManager.saveData()` (the shutdown): every weapon's row.
    pub fn save_cursed_weapons(&mut self) {
        for index in 0..self.cursed.weapons.len() {
            self.cursed_save_data(index);
        }
    }

    // ------------------------------------------------------------------------ the owner logs in

    /// Java `CursedWeaponsManager.checkPlayer(player)` (`Player.restore`): the holder of a weapon
    /// is back; the weapon knows its player again.
    pub(crate) fn cursed_check_player(&mut self, player: ObjectId) {
        for index in 0..self.cursed.weapons.len() {
            let weapon = &self.cursed.weapons[index];
            if !(weapon.is_activated && player.0 == weapon.player_id) {
                continue;
            }
            let item_id = weapon.item_id;
            let item = self
                .inventory_item_by_item_id(player, item_id)
                .map(|i| i.object_id);
            let weapon = &mut self.cursed.weapons[index];
            weapon.player = Some(player);
            weapon.item = item;
            let (name, end_time) = (weapon.name.clone(), weapon.end_time);
            self.cursed_give_skill(index);
            if let Some(p) = self.players.get_mut(&player) {
                p.cursed_weapon_equipped_id = item_id;
            }
            // `(int) ((cw.getEndTime() - System.currentTimeMillis()) / 60000)`.
            let minutes = (end_time - self.cursed_now_ms()) / i64::from(MS_PER_MINUTE);
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::S1HasS2MinuteSOfUsageTimeRemaining)
                    .with(SystemMessageParam::Text(name))
                    .with(SystemMessageParam::IntNumber(minutes as i32)),
            );
        }
    }

    /// Java `cursedOnLogin()`, called by `EnterWorld` for a player who holds a weapon.
    pub(crate) fn cursed_on_login(&mut self, player: ObjectId) {
        let item_id = self.cursed_weapon_equipped_id(player);
        let Some(index) = self.cursed.index_of(item_id) else {
            return;
        };
        self.cursed_do_transform(index);
        self.cursed_give_skill(index);

        let templates = self.item_templates();
        let (x, y, z) = self.position_of(player).unwrap_or_default();
        let message = SystemMessage::new(SystemMessageId::S2SOwnerHasLoggedIntoTheS1Region)
            .with(SystemMessageParam::ZoneName { x, y, z })
            .with(item_name_param(templates.as_deref(), item_id));
        self.broadcast_to_all_online_players(&message);

        let time_left = self.cursed.weapons[index].end_time - self.cursed_now_ms();
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::S1HasS2MinuteSOfUsageTimeRemaining)
                .with(item_name_param(templates.as_deref(), item_id))
                .with(SystemMessageParam::IntNumber((time_left / 60_000) as i32)),
        );
    }

    /// Java `Player.deleteMe`: `getCursedWeapon(id).setPlayer(null)`. The weapon keeps its owner's
    /// id; the player is offline.
    pub(crate) fn cursed_player_left(&mut self, player: ObjectId) {
        let item_id = self.cursed_weapon_equipped_id(player);
        if item_id == 0 {
            return;
        }
        if let Some(index) = self.cursed.index_of(item_id) {
            self.cursed.weapons[index].player = None;
        }
    }

    /// `EnterWorld`: "Remove demonic weapon if character is not cursed weapon equipped." D-53: not
    /// while the table is Sieghardt's (the item is his weapon's).
    pub(crate) fn cursed_enter_world_cleanup(&mut self, player: ObjectId) {
        if self.is_cursed_weapon_equipped(player) || !self.cursed.settings.cycle_database_writes {
            return;
        }
        for item_id in [ZARICHE_ID, AKAMANAH_ID] {
            if self.inventory_item_by_item_id(player, item_id).is_some() {
                // `player.destroyItem(DESTROY, item, null, true)`.
                self.destroy_item_by_item_id(player, item_id, 1, true);
            }
        }
    }
}
