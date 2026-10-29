//! Small `pub` entry points the scripts of `ai/areas/Hellbound` need and the `pub` world API did
//! not offer (S-AI-02). Each is a thin call into the module that owns the rule.
// Java: entity/spawns/Spawn.java (setRespawnDelay, doSpawn), SpawnTable.addSpawn,
//       entity/World.java (broadcastToAllOnlinePlayers), entity/actor/Attackable.java (getLeader)

use l2_util::ObjectId;

use crate::admin::{AdminHandlers, JavaException};
use crate::world::World;

/// Java `HellboundEngine.MAX_TRUST`: the trust a level ends at, by level.
pub const HELLBOUND_MAX_TRUST: [i32; 12] = [
    0, 300_000, 600_000, 1_000_000, 1_010_000, 1_400_000, 1_490_000, 2_000_000, 2_000_001, 2_500_000,
    4_000_000, 0,
];

/// Java `AdminHellbound.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    handlers.register("admin_hellbound_setlevel", admin_hellbound_setlevel);
    handlers.register("admin_hellbound", admin_hellbound);
}

/// Java `AdminHellbound` `admin_hellbound_setlevel`: `HellboundEngine.setLevel(level)` writes the
/// `HBLevel` variable; the engine's update (once a minute) sees the change like any change by an
/// administrator and applies it (`onLevelChange`).
fn admin_hellbound_setlevel(world: &mut World, gm: ObjectId, command: &str) -> Result<(), JavaException> {
    let level = command
        .split(' ')
        .filter(|t| !t.is_empty())
        .nth(1)
        .and_then(|t| t.parse::<i32>().ok())
        .filter(|l| (0..=11).contains(l));
    let Some(level) = level else {
        world.send_sys_message(gm, "Usage: //hellbound_setlevel 0-11");
        return Ok(());
    };
    // `setLevel`: nothing happens for the level it already is.
    if world.scripts.globals.get_int("HBLevel", 0) != level {
        world.scripts.globals.set("HBLevel", &level.to_string());
    }
    world.send_sys_message(gm, &format!("Hellbound level set to {level}"));
    Ok(())
}

/// Java `AdminHellbound.showMenu`. The limits are those of the current level (Java shows the ones
/// the engine cached at its last update).
fn admin_hellbound(world: &mut World, gm: ObjectId, _command: &str) -> Result<(), JavaException> {
    let level = world.scripts.globals.get_int("HBLevel", 0);
    let trust = world.scripts.globals.get_int("HBTrust", 0);
    let at = |l: i32| {
        usize::try_from(l)
            .ok()
            .and_then(|i| HELLBOUND_MAX_TRUST.get(i))
            .copied()
    };
    let (max, min) = match (at(level), at(level - 1)) {
        (Some(max), Some(min)) => (max, min),
        _ => (0, 0),
    };
    let mut page = world.admin_html_file(gm, "data/html/admin/hellbound.htm");
    page.replace("%hbstage%", &level.to_string());
    page.replace("%trust%", &trust.to_string());
    page.replace("%maxtrust%", &max.to_string());
    page.replace("%mintrust%", &min.to_string());
    world.send_admin_html(gm, &page);
    Ok(())
}

impl World {
    /// Java `new Spawn(npcId)` + `setAmount(1)` + `setLocation` + `setRespawnDelay(delay,
    /// randomInterval)` + `SpawnTable.addSpawn(spawn)` as `HellboundSpawns` makes it: the spawn
    /// index, nothing spawned yet (`script_do_spawn`). `delay_s` and `random_interval_s` are
    /// seconds.
    pub fn script_add_hellbound_spawn(
        &mut self,
        npc_id: i32,
        (x, y, z): (i32, i32, i32),
        heading: i32,
        delay_s: i32,
        random_interval_s: i32,
    ) -> usize {
        // Java `Spawn.setRespawnDelay(delay, randomInterval)`.
        let (min_ms, max_ms) = if delay_s != 0 {
            (
                (delay_s - random_interval_s).max(1).saturating_mul(1000),
                (delay_s + random_interval_s).max(1).saturating_mul(1000),
            )
        } else {
            (0, 0)
        };
        self.add_runtime_spawn(l2_data::spawn::NpcSpawn {
            npc_id,
            amount: 1,
            location: l2_data::Location { x, y, z },
            heading,
            respawn_min_ms: min_ms,
            respawn_max_ms: max_ms,
            respawn_enabled: min_ms > 0,
            chase_range: 0,
            name: None,
            territory: None,
            period_of_day: None,
            ai_data: None,
            group: None,
            // Not "script": those slots are given back once their NPC is gone.
            source: "hellbound".to_owned(),
        })
    }

    /// Java `spawn.doSpawn()`: one NPC of spawn `index`.
    pub fn script_do_spawn(&mut self, index: usize) -> Option<ObjectId> {
        let mut report = crate::npc::SpawnReport::default();
        self.do_spawn(index, &mut report)
    }

    /// Java `World.broadcastToAllOnlinePlayers(text)`: an announcement chat line.
    pub fn script_broadcast_announcement(&self, text: &str) {
        self.broadcast_announcement(text, false);
    }

    /// Java `Player.isMinimapAllowed()`.
    pub fn is_minimap_allowed(&self, player: ObjectId) -> bool {
        self.scripts
            .players
            .get(&player)
            .is_some_and(|q| q.minimap_allowed)
    }

    /// Java `Player.setMinimapAllowed(value)`; the client hears of it with the next `UserInfo`.
    pub fn set_minimap_allowed(&mut self, player: ObjectId, value: bool) {
        if let Some(quests) = self.scripts.players.get_mut(&player) {
            quests.minimap_allowed = value;
        }
    }

    /// Java `DecayTaskManager.getInstance().add(creature)`: the NPC decays after its corpse time
    /// even when it is alive.
    pub fn script_add_decay_task(&mut self, npc: ObjectId) {
        self.add_decay_task(npc);
    }

    /// Java `player.getPlayerClass().equalsOrChildOf(PlayerClass.X)` (`class_id` is `X.getId()`).
    pub fn player_class_equals_or_child_of(&self, player: ObjectId, class_id: i32) -> bool {
        let Some(p) = self.player(player) else {
            return false;
        };
        let (Ok(mine), Ok(other)) = (u16::try_from(p.data.class_id), u16::try_from(class_id)) else {
            return false;
        };
        l2_data::ClassId(mine).equals_or_child_of(l2_data::ClassId(other))
    }

    /// Java `npc.getSpawn().getRespawnDelay()`: `(min + max) / 2` in milliseconds; 0 without a spawn.
    pub fn npc_spawn_respawn_delay_ms(&self, npc: ObjectId) -> i32 {
        self.npc_spawn_index(npc)
            .and_then(|index| self.spawn_def(index))
            .map_or(0, |spawn| (spawn.respawn_min_ms + spawn.respawn_max_ms) / 2)
    }

    /// Java `creature.stopSkillEffects(SkillFinishType.REMOVED, skillId)`.
    pub fn script_stop_skill_effects_removed(&mut self, owner: ObjectId, skill_id: i32) {
        self.stop_skill_effects(owner, crate::effects::SkillFinishType::Removed, skill_id);
    }

    /// Java `ZoneManager.getZones(creature)` contains the zone `zone_id`.
    pub fn is_inside_zone_id(&self, creature: ObjectId, zone_id: i32) -> bool {
        let Some((x, y, z)) = self.position_of(creature) else {
            return false;
        };
        self.zoning
            .zones
            .as_ref()
            .is_some_and(|zones| zones.zones_at(x, y, z).iter().any(|zone| zone.id == zone_id))
    }

    /// Java `npc.getAI().getIntention() == Intention.FOLLOW`: here, that the NPC has a follow task
    /// (`script_start_follow` does not change the intention value, it starts the task).
    pub fn is_intention_follow(&self, npc: ObjectId) -> bool {
        self.pawn_of(npc).is_some_and(|pawn| pawn.follow.is_some())
    }

    /// Java `npc.scheduleDespawn(delay)`: the NPC is deleted after `delay_ms`.
    pub fn script_schedule_despawn(&mut self, npc: ObjectId, delay_ms: i64) {
        self.schedule_script_despawn(npc, delay_ms);
    }

    /// Java `creature.stopSkillEffects(SkillFinishType.SILENT, skillId)`.
    pub fn script_stop_skill_effects_silent(&mut self, owner: ObjectId, skill_id: i32) {
        self.stop_skill_effects(owner, crate::effects::SkillFinishType::Silent, skill_id);
    }

    /// Java `npc.getBusyMessage()`: empty when none was set.
    pub fn script_npc_busy_message(&self, npc: ObjectId) -> String {
        self.talk
            .busy
            .get(&npc)
            .map(|(_, message)| message.clone())
            .unwrap_or_default()
    }

    /// Java `npc.broadcastSay(chatType, npcStringId, radius)`: `NpcSay` to the players within
    /// `range` of the NPC.
    pub fn script_broadcast_npc_say_in_range(
        &self,
        npc: ObjectId,
        chat: l2_protocol_game::server::ChatType,
        npc_string: l2_protocol_game::NpcStringId,
        range: i32,
    ) {
        let Some(npc_id) = self.npc(npc).map(|n| n.npc_id) else {
            return;
        };
        let packet = l2_protocol_game::server::NpcSay {
            object_id: npc.0,
            text_type: chat.client_id(),
            npc_id: self.npc_world.template(npc_id).map_or(npc_id, |t| t.display_id),
            npc_string: npc_string.id(),
            text: String::new(),
            parameters: Vec::new(),
        };
        self.broadcast_packet_in_range(npc, &packet, range);
    }

    /// Java `player.isGM()` for the closures of `enter_instance`, which get a `World`.
    pub fn script_is_gm(&self, player: ObjectId) -> bool {
        self.is_gm(player)
    }

    /// Java `npc.getAI().getIntention()` is `ATTACK` or `CAST`.
    pub fn script_is_attacking_or_casting(&self, npc: ObjectId) -> bool {
        matches!(
            self.intention_of(npc),
            crate::movement::Intention::Attack | crate::movement::Intention::Cast
        )
    }

    /// Java `ZoneManager.getZones(x, y, z)` contains the zone `zone_id`.
    pub fn script_point_in_zone_id(&self, (x, y, z): (i32, i32, i32), zone_id: i32) -> bool {
        self.zoning
            .zones
            .as_ref()
            .is_some_and(|zones| zones.zones_at(x, y, z).iter().any(|zone| zone.id == zone_id))
    }

    /// Java `zone.setEnabled(value)` of a `DamageZone` or `SwampZone` (the switch the flame towers
    /// of the castle sieges use).
    pub fn script_set_trap_zone_enabled(&mut self, zone_id: i32, value: bool) {
        self.sieges.trap_zones.insert(zone_id, value);
    }

    /// Java `Monster.getLeader()`: the master of a minion.
    pub fn script_leader_of(&self, npc: ObjectId) -> Option<ObjectId> {
        self.leader_of(npc)
    }

    /// Java `creature.getEffectList().getBuffInfoBySkillId(skillId)` then
    /// `getSkill().getAbnormalLevel()`: `None` without that buff.
    pub fn script_buff_abnormal_level_of_skill(&self, owner: ObjectId, skill_id: i32) -> Option<i32> {
        self.effect_list(owner)?
            .info_by_skill_id(skill_id)
            .map(|info| info.facts.abnormal_level)
    }
}
