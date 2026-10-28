//! The teleporters of Valakas: into the Hall of Flames, into the Lair (the first one starts the
//! encounter timer of the Valakas script) and out of it.
// Java: dist/game/data/scripts/ai/others/ValakasTeleporters/ValakasTeleporters.java
//
// Side by side (D-50, D-54): the status of the boss decides who may enter, so with
// `CycleDatabaseWrites` off the `grandboss_data` row is read again first; the entry goes on when the
// answer comes (`ENTER_LAIR_READ`), and the page is shown then. The boss status itself is written
// by `set_grand_boss_status`, which keeps it in memory with the switch off.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const NPCS: [i32; 6] = [
    31384, // Gatekeeper of Fire Dragon: Opening some doors.
    31385, // Heart of Volcano: Teleport into Lair of Valakas.
    31540, // Watcher of Valakas Klein: Teleport into Hall of Flames.
    31686, // Gatekeeper of Fire Dragon: Opens doors to Heart of Volcano.
    31687, // Gatekeeper of Fire Dragon: Opens doors to Heart of Volcano.
    31759, // Teleportation Cubic: Teleport out of Lair of Valakas.
];

// Items
const VACUALITE_FLOATING_STONE: i32 = 7267;
const ENTER_HALL_OF_FLAMES: (i32, i32, i32) = (183813, -115157, -3303);
const TELEPORT_INTO_VALAKAS_LAIR: (i32, i32, i32) = (204328, -111874, 70);
const TELEPORT_OUT_OF_VALAKAS_LAIR: (i32, i32, i32) = (150037, -57720, -2976);

/// Java `Valakas`' grand boss id.
const VALAKAS_ID: i32 = 29028;
/// The event the answer of the re-read row comes back as.
const ENTER_LAIR_READ: &str = "ENTER_LAIR_READ";

/// Java's `static int playerCount` and the entries waiting for the re-read row.
#[derive(Default)]
struct Entries {
    player_count: i32,
    waiting: Vec<(ObjectId, ScriptNpc)>,
}

/// The `ENTER_LAIR` event once the status is known. The returned page is Java's `htmltext`.
fn enter_lair(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> String {
    let mut htmltext = String::new();
    let Some(qs) = ctx.get_quest_state(player, true) else {
        return htmltext;
    };
    // `valakasAI() != null`
    if ctx.world().has_script("Valakas") {
        // Java unboxes the status (null throws for a boss without a row): no answer then.
        let Some(status) = ctx.world().grand_boss_status(VALAKAS_ID) else {
            return htmltext;
        };
        if (status == 0 || status == 1) && qs.get_int(ctx, "allowEnter") == 1 {
            if ctx.state::<Entries>().player_count >= 200 {
                htmltext = "31385-03.htm".to_owned();
            } else {
                qs.unset(ctx, "allowEnter");
                if let Some(zone) = ctx.world().grand_boss_zone_at(212852, -114842, -1632) {
                    ctx.world_mut().allow_boss_zone_entry(zone, player, 30);
                }

                let (x, y, z) = TELEPORT_INTO_VALAKAS_LAIR;
                let (dx, dy) = (ctx.get_random(600), ctx.get_random(600));
                ctx.teleport_player(player, x + dx, y + dy, z, 0, false);
                // NOTE(java-quirk): `playerCount` is never lowered, so after 200 entries nobody
                // enters until the server restarts.
                ctx.state::<Entries>().player_count += 1;

                if status == 0 {
                    let valakas = ctx
                        .world()
                        .grand_boss_npc(VALAKAS_ID)
                        .and_then(|boss| ctx.script_npc(boss));
                    let wait = i64::from(ctx.world().boss_settings().grand_boss.valakas_wait_time) * 60000;
                    ctx.start_quest_timer_of("Valakas", "VALAKAS_START_ENCOUNTER", wait, valakas, None, false);
                    ctx.world_mut().set_grand_boss_status(VALAKAS_ID, 1);
                }
            }
        } else {
            htmltext = if status == 2 { "31385-02.htm" } else { "31385-04.htm" }.to_owned();
        }
    } else {
        htmltext = "31385-01.htm".to_owned();
    }

    htmltext
}

/// Java `ai.others.ValakasTeleporters.ValakasTeleporters`.
pub struct ValakasTeleporters;

impl Script for ValakasTeleporters {
    fn name(&self) -> &'static str {
        "ValakasTeleporters"
    }

    fn path(&self) -> &'static str {
        "ai/others/ValakasTeleporters"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&NPCS);
        s.add_talk_id(&NPCS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == ENTER_LAIR_READ {
            // The re-read row is in: the entries that waited go on, each with its page.
            let waiting = std::mem::take(&mut ctx.state::<Entries>().waiting);
            for (waiting_player, waiting_npc) in waiting {
                let htmltext = enter_lair(ctx, waiting_player);
                ctx.show_result(waiting_player, Some(&htmltext), Some(waiting_npc));
            }
            return None;
        }

        let (npc, player) = (npc?, player?);
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = String::new();

        match event {
            "ENTER" => {
                if ctx.has_quest_items(player, VACUALITE_FLOATING_STONE) {
                    let (x, y, z) = ENTER_HALL_OF_FLAMES;
                    ctx.teleport_player(player, x, y, z, 0, false);
                    qs.set(ctx, "allowEnter", "1");
                } else {
                    htmltext = "31540-06.htm".to_owned();
                }
            }
            "ENTER_LAIR" => {
                if ctx.world().boss_writes_enabled() {
                    htmltext = enter_lair(ctx, player);
                } else {
                    // D-54: the row decides; read it again first.
                    ctx.state::<Entries>().waiting.retain(|(p, _)| *p != player);
                    ctx.state::<Entries>().waiting.push((player, npc));
                    ctx.world_mut()
                        .reread_grand_boss_for_script(VALAKAS_ID, "ValakasTeleporters", ENTER_LAIR_READ);
                    return None;
                }
            }
            "OPEN_DOOR_1" => ctx.open_door(24210004, 0),
            "OPEN_DOOR_2" => ctx.open_door(24210005, 0),
            "OPEN_DOOR_3" => ctx.open_door(24210006, 0),
            "EXIT" => {
                let (x, y, z) = TELEPORT_OUT_OF_VALAKAS_LAIR;
                let (dx, dy) = (ctx.get_random(500), ctx.get_random(500));
                ctx.teleport_player(player, x + dx, y + dy, z, 0, false);
            }
            _ => {}
        }

        Some(htmltext)
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        match npc.id {
            31540 => {
                // Watcher of Valakas Klein
                let player_count = ctx.state::<Entries>().player_count;
                Some(
                    if player_count < 50 {
                        "31540-01.htm"
                    } else if player_count < 100 {
                        "31540-02.htm"
                    } else if player_count < 150 {
                        "31540-03.htm"
                    } else if player_count < 200 {
                        "31540-04.htm"
                    } else {
                        "31540-05.htm"
                    }
                    .to_owned(),
                )
            }
            _ => Some(format!("{}.htm", npc.id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::{PersistCommand, ScriptRegistry};

    use super::*;
    use crate::testing::{ScriptTest, item};

    const NPCS_XML: &str = r#"<npc id="31385" type="Folk" level="70" name="Heart of Volcano"/>
        <npc id="31540" type="Folk" level="70" name="Klein"/>"#;
    const ITEMS: &str = r#"<item id="7267" type="EtcItem" name="Vacualite Floating Stone"><set name="is_questitem" val="true"/></item>"#;

    /// Stand-in for the Valakas script: only its name is read.
    struct Valakas;

    impl Script for Valakas {
        fn name(&self) -> &'static str {
            "Valakas"
        }
        fn path(&self) -> &'static str {
            "ai/bosses/Valakas"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup(with_valakas: bool) -> (ScriptTest, ScriptNpc) {
        let mut registry = ScriptRegistry::new();
        registry.add(ValakasTeleporters);
        if with_valakas {
            registry.add(Valakas);
        }
        let mut t = ScriptTest::new(registry, NPCS_XML, ITEMS, &[]);
        let klein = t.spawn(31540, 20);
        t.enter(1, 80, vec![item(5000, VACUALITE_FLOATING_STONE, 1)]);
        (t, ScriptNpc { object_id: klein, id: 31540 })
    }

    fn event(t: &mut ScriptTest, npc: ScriptNpc, name: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script("ValakasTeleporters", |ctx| {
            out = ValakasTeleporters.on_event(ctx, name, Some(npc), Some(ObjectId(1)));
        });
        out
    }

    #[test]
    fn the_hall_needs_the_stone_and_allows_the_lair() {
        let (mut t, npc) = setup(true);
        assert_eq!(event(&mut t, npc, "ENTER").as_deref(), Some(""));
        let at = t.world.position_of(ObjectId(1)).unwrap();
        assert_eq!((at.0, at.1), (183813, -115157));
        let quests = t.world.player_quests(ObjectId(1)).unwrap();
        let qs = quests.get("ValakasTeleporters").unwrap();
        assert_eq!(qs.get_int("allowEnter"), 1);
    }

    #[test]
    fn without_the_stone_the_klein_page_follows() {
        let (mut t, npc) = setup(true);
        t.enter(2, 80, vec![]);
        let mut out = None;
        t.world.run_as_script("ValakasTeleporters", |ctx| {
            out = ValakasTeleporters.on_event(ctx, "ENTER", Some(npc), Some(ObjectId(2)));
        });
        assert_eq!(out.as_deref(), Some("31540-06.htm"));
    }

    #[test]
    fn the_talk_page_follows_the_player_count_and_other_npcs_get_their_own_page() {
        let (mut t, npc) = setup(true);
        let mut pages = Vec::new();
        t.world.run_as_script("ValakasTeleporters", |ctx| {
            pages.push(ValakasTeleporters.on_talk(ctx, npc, ObjectId(1)));
            let other = ScriptNpc { object_id: npc.object_id, id: 31385 };
            pages.push(ValakasTeleporters.on_talk(ctx, other, ObjectId(1)));
            ctx.state::<Entries>().player_count = 120;
            pages.push(ValakasTeleporters.on_talk(ctx, npc, ObjectId(1)));
        });
        assert_eq!(
            pages,
            [
                Some("31540-01.htm".to_owned()),
                Some("31385.htm".to_owned()),
                Some("31540-03.htm".to_owned())
            ]
        );
    }

    #[test]
    fn the_lair_waits_for_the_re_read_row_when_the_cycle_writes_are_off() {
        let (mut t, npc) = setup(true);
        t.take_boss();
        assert_eq!(event(&mut t, npc, "ENTER_LAIR"), None);
        let asked = t
            .take_boss()
            .iter()
            .filter(|c| matches!(c, PersistCommand::BossRereadRequest { .. }))
            .count();
        assert_eq!(asked, 1);
        // The row has no Valakas here: Java throws on the unboxed status, the player stays.
        t.world.run_as_script("ValakasTeleporters", |ctx| {
            ValakasTeleporters.on_event(ctx, ENTER_LAIR_READ, None, None);
        });
        let at = t.world.position_of(ObjectId(1)).unwrap();
        assert_ne!((at.0, at.1), (204328, -111874));
    }

    #[test]
    fn without_the_valakas_script_the_lair_page_says_so() {
        let (mut t, npc) = setup(false);
        t.world.run_as_script("ValakasTeleporters", |ctx| {
            ctx.state::<Entries>().waiting.push((ObjectId(1), npc));
        });
        let mut page = None;
        t.world.run_as_script("ValakasTeleporters", |ctx| page = Some(enter_lair(ctx, ObjectId(1))));
        assert_eq!(page.as_deref(), Some("31385-01.htm"));
    }
}
