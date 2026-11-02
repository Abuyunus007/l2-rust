//! The raid boss information NPCs: a link per boss that marks its spawn on the radar.
// Java: dist/game/data/scripts/custom/RaidbossInfo/RaidbossInfo.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// @formatter:off
const NPC: &[i32] = &[
    31729, 31730, 31731, 31732, 31733, 31734, 31735, 31736, 31737, 31738, 31739, 31740, 31741, 31742, 31743,
    31744, 31745, 31746, 31747, 31748, 31750, 31751, 31752, 31755, 31756, 31757, 31758, 31759, 31760, 31761,
    31762, 31763, 31764, 31765, 31766, 31767, 31768, 31769, 31770, 31771, 31772, 31773, 31774, 31775, 31776,
    31777, 31778, 31779, 31780, 31781, 31782, 31783, 31784, 31785, 31786, 31787, 31788, 31789, 31790, 31791,
    31792, 31793, 31794, 31795, 31796, 31797, 31798, 31799, 31800, 31801, 31802, 31803, 31804, 31805, 31806,
    31807, 31808, 31809, 31810, 31811, 31812, 31813, 31814, 31815, 31816, 31817, 31818, 31819, 31820, 31821,
    31822, 31823, 31824, 31825, 31826, 31827, 31828, 31829, 31830, 31831, 31832, 31833, 31834, 31835, 31836,
    31837, 31838, 31839, 31840, 31841, 32337, 32338, 32339, 32340,
];
// @formatter:on

/// Java `custom.RaidbossInfo.RaidbossInfo`.
pub struct RaidbossInfo;

impl Script for RaidbossInfo {
    fn name(&self) -> &'static str {
        "RaidbossInfo"
    }

    fn path(&self) -> &'static str {
        "custom/RaidbossInfo"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(NPC);
        s.add_talk_id(NPC);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = Some(event.to_owned());
        // `StringUtil.isNumeric(event)`.
        if !event.is_empty() && event.chars().all(|c| c.is_ascii_digit()) {
            htmltext = None;
            let boss_id: i32 = event.parse().ok()?;
            let player = player?;
            // `RAIDS`: every NPC template of the type `RaidBoss`, built in the constructor.
            if ctx.world().npc_template_is_type(boss_id, "RaidBoss") {
                // `SpawnTable.getAnySpawn(bossId)`: the first spawn of the table.
                if let Some((x, y, z)) = ctx.world().any_spawn_location(boss_id) {
                    ctx.add_radar(player, x, y, z);
                }
            }
        }

        htmltext
    }

    fn on_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("info.htm".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to};

    const INFO_NPC: i32 = 31729;
    const NPCS: &str = r#"<npc id="31729" type="Folk" level="70" name="Info"/>
        <npc id="25001" type="RaidBoss" level="70" name="Boss"/>"#;

    #[test]
    fn the_npc_list_is_the_java_one() {
        assert_eq!(NPC.len(), 114);
        assert!(NPC.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn the_info_page_comes_and_a_page_event_echoes() {
        let mut registry = ScriptRegistry::new();
        registry.add(RaidbossInfo);
        let pages = Pages::of(&RaidbossInfo)
            .page("info.htm", &["level20.htm"])
            .plain(&["level20.htm"])
            .npcs_by_name(&[INFO_NPC]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        t.enter(1, 40, vec![]);
        let npc = t.spawn(INFO_NPC, 20);
        assert!(t.script_page(1, npc, "RaidbossInfo").contains("info.htm"));
        assert!(t.event(1, "RaidbossInfo", "level20.htm").contains("level20.htm"));
    }

    #[test]
    fn a_boss_without_a_spawn_marks_nothing() {
        let mut registry = ScriptRegistry::new();
        registry.add(RaidbossInfo);
        let pages = Pages::of(&RaidbossInfo)
            .page("info.htm", &["25001"])
            .npcs_by_name(&[INFO_NPC]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        t.enter(1, 40, vec![]);
        let npc = t.spawn(INFO_NPC, 20);
        t.script_page(1, npc, "RaidbossInfo");
        t.bypass(1, "Script RaidbossInfo 25001");
        assert!(!names_to(&t.take(), 1).iter().any(|n| n == "RADAR_CONTROL"));
        // A script spawn is not in the spawn table: still no marker.
        t.world.spawn_npc_at(25001, 0, 0, 0, 0);
        t.bypass(1, "Script RaidbossInfo 25001");
        assert!(!names_to(&t.take(), 1).iter().any(|n| n == "RADAR_CONTROL"));
    }

    #[test]
    fn a_boss_with_a_spawn_is_marked_on_the_radar() {
        let mut registry = ScriptRegistry::new();
        registry.add(RaidbossInfo);
        let pages = Pages::of(&RaidbossInfo)
            .page("info.htm", &["25001", "25002"])
            .npcs_by_name(&[INFO_NPC]);
        let npcs = format!(r#"{NPCS}<npc id="25002" type="Folk" level="70" name="NotABoss"/>"#);
        let mut t = ScriptTest::with_pages(registry, &npcs, "", pages);
        // The raid boss manager put the spawn of the boss into the spawn table; the boss is not
        // due for an hour, so it does not stand in the world.
        let due = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
            + 3_600_000;
        t.world.spawn_raid_bosses(vec![l2_world::RaidBossData {
            boss_id: 25001,
            amount: 1,
            loc_x: 1000,
            loc_y: 2000,
            loc_z: -300,
            heading: 0,
            respawn_delay: 3600,
            respawn_random: 0,
            respawn_time: due,
            current_hp: 100.0,
            current_mp: 50.0,
        }]);
        t.enter(1, 40, vec![]);
        let npc = t.spawn(INFO_NPC, 20);
        t.talk(1, npc);
        t.take();
        t.bypass(1, "Script RaidbossInfo");
        t.take();
        t.bypass(1, "Script RaidbossInfo 25001");
        let out = t.take();
        assert_eq!(
            names_to(&out, 1).iter().filter(|n| *n == "RADAR_CONTROL").count(),
            2
        );
        // A template that is not a raid boss is not in `RAIDS`.
        t.talk(1, npc);
        t.bypass(1, "Script RaidbossInfo");
        t.take();
        t.bypass(1, "Script RaidbossInfo 25002");
        assert!(!names_to(&t.take(), 1).iter().any(|n| n == "RADAR_CONTROL"));
    }
}
