//! Naia Cube of the Steel Citadel: lets a command channel into Beleth's chamber.
// Java: dist/game/data/scripts/ai/others/SteelCitadelTeleport/SteelCitadelTeleport.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};

// NPCs
const BELETH: i32 = 29118;
const NAIA_CUBE: i32 = 32376;

// Location
const TELEPORT_CITADEL: (i32, i32, i32) = (16342, 209557, -9352);

// Beleth's chamber: `getZoneById(12018)`.
const BELETH_ZONE: (i32, &str) = (12018, "BossZone");

/// Java `ai.others.SteelCitadelTeleport.SteelCitadelTeleport`.
pub struct SteelCitadelTeleport;

impl Script for SteelCitadelTeleport {
    fn name(&self) -> &'static str {
        "SteelCitadelTeleport"
    }

    fn path(&self) -> &'static str {
        "ai/others/SteelCitadelTeleport"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NAIA_CUBE]);
        s.add_talk_id(&[NAIA_CUBE]);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        // Java unboxes the status: a boss without a row throws, nothing happens here.
        let beleth_status = ctx.world().grand_boss_status(BELETH)?;
        if beleth_status == 3 {
            return Some("32376-02.htm".to_owned());
        }

        if beleth_status > 0 {
            return Some("32376-03.htm".to_owned());
        }

        let channel = ctx.world().command_channel_of(player);
        let min_players = ctx.world().boss_settings().grand_boss.beleth_min_players;
        let allowed = channel.is_some_and(|channel| {
            let world = ctx.world();
            world.command_channel_leader(channel) == Some(player)
                && i64::try_from(world.command_channel_member_count(channel)).unwrap_or(i64::MAX)
                    >= i64::from(min_players)
        });
        let channel = match channel {
            Some(channel) if allowed => channel,
            _ => return Some("32376-02a.htm".to_owned()),
        };

        let (zone_id, zone_type) = BELETH_ZONE;
        if ctx.world().zone_by_id(zone_id, zone_type).is_some() {
            ctx.world_mut().set_grand_boss_status(BELETH, 1);
            let offset = ctx.world().max_offset_on_teleport();
            for party in ctx.world().command_channel_parties(channel) {
                for pl in ctx.world().party_members(party) {
                    if ctx
                        .distance_3d(pl, npc.object_id)
                        .is_some_and(|distance| distance <= 3000.0)
                    {
                        ctx.world_mut().allow_boss_zone_entry(zone_id, pl, 30);
                        let (x, y, z) = TELEPORT_CITADEL;
                        ctx.world_mut().tele_to_location(
                            pl,
                            TeleportTarget::at(x, y, z)
                                .with_instance(0)
                                .with_random_offset(offset),
                        );
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{GrandBossData, ScriptRegistry};

    use super::*;
    use crate::ai::bosses::test_support::{boss_test, row};

    const NPCS: &str = r#"<npc id="32376" type="Folk" level="70" name="Naia Cube"/>"#;

    fn setup(status: i32) -> (crate::testing::ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(SteelCitadelTeleport);
        let zone = r#"<?xml version="1.0" encoding="UTF-8"?>
<list enabled="true"><zone name="Beleth's Chamber" id="12018" type="BossZone" shape="NPoly" minZ="-10412" maxZ="-5112">
<node X="11214" Y="205578" /><node X="20607" Y="205578" /><node X="20648" Y="220271" /><node X="11172" Y="220271" /></zone></list>
"#;
        let rows: Vec<GrandBossData> = vec![row(BELETH, status, 0, (16323, 213170, -9357))];
        let mut t = boss_test(registry, NPCS, "", &[("data/zones/beleth.xml", zone)], rows, true);
        t.enter(1, 80, vec![]);
        let cube = t.spawn(NAIA_CUBE, 20);
        (t, cube)
    }

    fn talk(t: &mut crate::testing::ScriptTest, cube: Oid) -> Option<String> {
        let mut out = None;
        t.world.run_as_script("SteelCitadelTeleport", |ctx| {
            let npc = ctx.script_npc(cube).unwrap();
            out = SteelCitadelTeleport.on_talk(ctx, npc, Oid(1));
        });
        out
    }

    #[test]
    fn the_status_and_the_channel_decide_the_page() {
        let (mut t, cube) = setup(3);
        assert_eq!(talk(&mut t, cube).as_deref(), Some("32376-02.htm"));
        let (mut t, cube) = setup(2);
        assert_eq!(talk(&mut t, cube).as_deref(), Some("32376-03.htm"));
        // Alive but no command channel.
        let (mut t, cube) = setup(0);
        assert_eq!(talk(&mut t, cube).as_deref(), Some("32376-02a.htm"));
        assert_eq!(t.world.grand_boss_status(BELETH), Some(0));
    }
}
