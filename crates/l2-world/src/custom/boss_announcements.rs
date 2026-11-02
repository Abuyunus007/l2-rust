//! Raid boss and grand boss spawn / defeat announcements (`Custom/BossAnnouncements.ini`).
// Java: config/custom/BossAnnouncementsConfig.java, entity/actor/Creature.java (`onSpawn`, `doDie`),
//       entity/World.java (`broadcastToAllOnlinePlayers`, `broadcastToAllOnlinePlayersOnScreen`)

use std::collections::BTreeSet;

use l2_protocol_game::server::ExShowScreenMessage;
use l2_util::ObjectId;

use crate::npc::NpcClass;
use crate::world::World;

/// Java `BossAnnouncementsConfig`. `Default` is the Java default of each key (all off).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BossAnnouncementSettings {
    /// `RaidBossSpawnAnnouncements`.
    pub raidboss_spawn_announcements: bool,
    /// `RaidBossDefeatAnnouncements`.
    pub raidboss_defeat_announcements: bool,
    /// `RaidBossInstanceAnnouncements`.
    pub raidboss_instance_announcements: bool,
    /// `GrandBossSpawnAnnouncements`.
    pub grandboss_spawn_announcements: bool,
    /// `GrandBossDefeatAnnouncements`.
    pub grandboss_defeat_announcements: bool,
    /// `GrandBossInstanceAnnouncements`.
    pub grandboss_instance_announcements: bool,
    /// `RaidbossExcludedFromSpawnAnnouncements`.
    pub raidbosses_excluded_from_spawn_announcements: BTreeSet<i32>,
    /// `RaidbossExcludedFromDefeatAnnouncements`.
    pub raidbosses_excluded_from_defeat_announcements: BTreeSet<i32>,
}

/// The on-screen time of `World.broadcastToAllOnlinePlayersOnScreen(text)`.
const ON_SCREEN_MS: i32 = 10_000;

impl World {
    /// The tail of Java `Creature.onSpawn()`: "Custom boss announcements configuration."
    pub(crate) fn boss_announce_spawn(&self, id: ObjectId) {
        let s = &self.custom.settings.boss_announcements;
        self.boss_announce(
            id,
            " has spawned!",
            (
                s.grandboss_spawn_announcements,
                s.grandboss_instance_announcements,
            ),
            (s.raidboss_spawn_announcements, s.raidboss_instance_announcements),
            &s.raidbosses_excluded_from_spawn_announcements,
        );
    }

    /// The tail of Java `Creature.doDie()`: "Custom boss announcements configuration."
    pub(crate) fn boss_announce_defeat(&self, id: ObjectId) {
        let s = &self.custom.settings.boss_announcements;
        self.boss_announce(
            id,
            " has been defeated!",
            (
                s.grandboss_defeat_announcements,
                s.grandboss_instance_announcements,
            ),
            (s.raidboss_defeat_announcements, s.raidboss_instance_announcements),
            &s.raidbosses_excluded_from_defeat_announcements,
        );
    }

    /// `grand` and `raid` are `(enabled, instance announcements)` of the grand boss and of the
    /// raid boss branch.
    fn boss_announce(
        &self,
        id: ObjectId,
        suffix: &str,
        grand: (bool, bool),
        raid: (bool, bool),
        excluded: &BTreeSet<i32>,
    ) {
        let Some(npc) = self.npcs.get(&id) else {
            return;
        };
        let (enabled, instances) = match npc.class {
            NpcClass::GrandBoss => grand,
            // `isRaid()` of the rest: a raid boss.
            class if class.is_raid() => raid,
            _ => return,
        };
        // `(!isInInstance() || ..INSTANCE_ANNOUNCEMENTS) && !isMinion() && !isRaidMinion()`.
        if !enabled
            || (self.is_in_instance(id) && !instances)
            || npc.leader.is_some()
            || self.is_raid_minion(id)
        {
            return;
        }
        let Some(template) = self.npc_world.template(npc.npc_id) else {
            return;
        };
        // NOTE(java-quirk): Java's grand boss branch reads the same exclusion sets (named after
        // raid bosses) as the raid boss branch.
        if excluded.contains(&npc.npc_id) {
            return;
        }
        let text = format!("{}{suffix}", template.name);
        self.broadcast_announcement_text(&text);
        self.broadcast_to_all_online_players(&ExShowScreenMessage::text(&text, ON_SCREEN_MS));
    }
}
