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
    let at = |l: i32| usize::try_from(l).ok().and_then(|i| HELLBOUND_MAX_TRUST.get(i)).copied();
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
        self.scripts.players.get(&player).is_some_and(|q| q.minimap_allowed)
    }

    /// Java `Player.setMinimapAllowed(value)`; the client hears of it with the next `UserInfo`.
    pub fn set_minimap_allowed(&mut self, player: ObjectId, value: bool) {
        if let Some(quests) = self.scripts.players.get_mut(&player) {
            quests.minimap_allowed = value;
        }
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
