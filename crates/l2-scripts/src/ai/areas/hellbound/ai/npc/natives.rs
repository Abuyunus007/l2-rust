//! The natives of Hellbound: the starving ones, the insurgents, the traitor and the captives of
//! the castle.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/NPC/Natives/Natives.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::hellbound::engine;

// NPCs
const NATIVE: i32 = 32362;
const INSURGENT: i32 = 32363;
const TRAITOR: i32 = 32364;
const INCASTLE: i32 = 32357;

// Items
const MARK_OF_BETRAYAL: i32 = 9676; // Mark of Betrayal
const BADGES: i32 = 9674; // Darion's Badge

// Misc
const DOORS: [i32; 2] = [19250003, 19250004];

/// Java `ai.areas.Hellbound.AI.NPC.Natives.Natives`.
pub struct Natives;

impl Script for Natives {
    fn name(&self) -> &'static str {
        "Natives"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/NPC/Natives"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[NATIVE]);
        s.add_first_talk_id(&[INSURGENT]);
        s.add_first_talk_id(&[INCASTLE]);
        s.add_start_npc(&[TRAITOR]);
        s.add_start_npc(&[INCASTLE]);
        s.add_talk_id(&[TRAITOR]);
        s.add_talk_id(&[INCASTLE]);
        s.add_spawn_id(&[NATIVE]);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        let mut htmltext = None;
        let hellbound_level = engine::level(ctx);
        match npc.id {
            NATIVE => {
                htmltext = Some(if hellbound_level > 5 {
                    "32362-01.htm"
                } else {
                    "32362.htm"
                });
            }
            INSURGENT => {
                htmltext = Some(if hellbound_level > 5 {
                    "32363-01.htm"
                } else {
                    "32363.htm"
                });
            }
            INCASTLE => {
                htmltext = Some(if hellbound_level < 9 {
                    "32357-01a.htm"
                } else if hellbound_level == 9 {
                    if ctx.is_npc_busy(npc) {
                        "32357-02.htm"
                    } else {
                        "32357-01.htm"
                    }
                } else {
                    "32357-01b.htm"
                });
            }
            _ => {}
        }

        htmltext.map(str::to_owned)
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        let npc = npc?;
        if npc.id == TRAITOR {
            if event.eq_ignore_ascii_case("open_door") {
                let player = player?;
                if ctx.get_quest_items_count(player, MARK_OF_BETRAYAL) >= 10 {
                    ctx.take_items(player, MARK_OF_BETRAYAL, 10);
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::AlrightNowLeodasIsYours,
                        &[],
                    );
                    engine::update_trust(ctx, -50, true);

                    for door_id in DOORS {
                        if ctx.world().door_by_door_id(door_id).is_some() {
                            ctx.open_door(door_id, 0);
                        }
                    }

                    ctx.cancel_quest_timers("close_doors");
                    ctx.start_quest_timer("close_doors", 1_800_000, Some(npc), Some(player), false); // 30 min
                } else if ctx.has_quest_items(player, MARK_OF_BETRAYAL) {
                    htmltext = Some("32364-01.htm".to_owned());
                } else {
                    htmltext = Some("32364-02.htm".to_owned());
                }
            } else if event.eq_ignore_ascii_case("close_doors") {
                for door_id in DOORS {
                    if ctx.world().door_by_door_id(door_id).is_some() {
                        ctx.close_door(door_id, 0);
                    }
                }
            }
        } else if npc.id == NATIVE && event.eq_ignore_ascii_case("hungry_death") {
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::HunHungry, &[]);
            ctx.world_mut().do_die(npc.object_id, None);
        } else if npc.id == INCASTLE {
            if event.eq_ignore_ascii_case("FreeSlaves") {
                let player = player?;
                if ctx.get_quest_items_count(player, BADGES) >= 5 {
                    ctx.take_items(player, BADGES, 5);
                    ctx.set_npc_busy(npc, true); // Prevent Native from take items more, than once
                    engine::update_trust(ctx, 100, true);
                    htmltext = Some("32357-02.htm".to_owned());
                    ctx.start_quest_timer("delete_me", 3000, Some(npc), None, false);
                } else {
                    htmltext = Some("32357-02a.htm".to_owned());
                }
            } else if event.eq_ignore_ascii_case("delete_me") {
                ctx.set_npc_busy(npc, false); // TODO: Does it really need?
                ctx.delete_npc(npc);
            }
        }

        htmltext
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if npc.id == NATIVE && engine::level(ctx) < 6 {
            ctx.start_quest_timer("hungry_death", 600_000, Some(npc), None, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::engine::HellboundEngine;
    use crate::ai::areas::hellbound::testing::{last_page, set_level, spawn};
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="32362" type="Folk" level="70" name="Native"/>
        <npc id="32363" type="Folk" level="70" name="Insurgent"/>
        <npc id="32364" type="Folk" level="70" name="Traitor"/>
        <npc id="32357" type="Folk" level="70" name="Native in the Castle"/>"#;
    const ITEMS: &str = r#"
        <item id="9674" type="EtcItem" name="Darion's Badge"><set name="is_stackable" val="true"/></item>
        <item id="9676" type="EtcItem" name="Mark of Betrayal"><set name="is_stackable" val="true"/></item>"#;

    fn setup(items: Vec<l2_world::ItemData>) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(HellboundEngine);
        registry.add(Natives);
        let pages = Pages::of(&Natives)
            .page("32364-01.htm", &[])
            .page("32357-01.htm", &["FreeSlaves"])
            .page("32364-00.htm", &["open_door"])
            .plain(&[
                "32362.htm",
                "32362-01.htm",
                "32357-01a.htm",
                "32357-02.htm",
                "32357-02a.htm",
                "32364-02.htm",
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        t.enter(1, 80, items);
        t
    }

    #[test]
    fn the_native_first_page_depends_on_the_level_and_it_starves_before_level_six() {
        let mut t = setup(vec![]);
        let native = spawn(&mut t, NATIVE, 20);
        t.talk(1, native);
        assert!(last_page(&t.take(), 1).contains("32362.htm"));
        t.world.run_as_script("Natives", |ctx| {
            assert!(ctx.has_quest_timer("hungry_death", ctx.script_npc(native), None));
        });
        t.wait_ms(600_100);
        assert!(t.world.is_dead(native) || !t.in_world(native));
        set_level(&mut t, "Natives", 6);
        let other = spawn(&mut t, NATIVE, 40);
        t.world.run_as_script("Natives", |ctx| {
            assert!(!ctx.has_quest_timer("hungry_death", ctx.script_npc(other), None));
        });
        t.talk(1, other);
        assert!(last_page(&t.take(), 1).contains("32362-01.htm"));
    }

    #[test]
    fn the_castle_native_is_freed_for_five_badges_once() {
        let mut t = setup(vec![item(100, BADGES, 7)]);
        let native = spawn(&mut t, INCASTLE, 20);
        t.talk(1, native);
        assert!(last_page(&t.take(), 1).contains("32357-01a.htm"));
        set_level(&mut t, "Natives", 9);
        t.talk(1, native);
        assert!(last_page(&t.take(), 1).contains("32357-01.htm"));
        assert!(t.event(1, "Natives", "FreeSlaves").contains("32357-02.htm"));
        assert_eq!(t.count(1, BADGES), 2);
        // `updateTrust(100, true)` at level 9: no limits yet in the test, so 100 points.
        assert_eq!(t.world.global_variables().get_int("HBTrust", 0), 100);
        t.world.run_as_script("Natives", |ctx| {
            let npc = ctx.script_npc(native).unwrap();
            assert!(ctx.is_npc_busy(npc));
        });
        // Three seconds later the native is gone.
        t.wait_ms(3_100);
        assert!(!t.in_world(native));
    }

    #[test]
    fn the_traitor_takes_ten_marks_and_costs_trust() {
        let mut t = setup(vec![item(100, MARK_OF_BETRAYAL, 12)]);
        set_level(&mut t, "Natives", 3);
        t.world
            .run_as_script("Natives", |ctx| ctx.set_global_variable("HBTrust", "1000"));
        let traitor = spawn(&mut t, TRAITOR, 20);
        t.world.run_as_script("Natives", |ctx| {
            let npc = ctx.script_npc(traitor).unwrap();
            // No door in the test world: the call only takes the marks and the trust.
            assert_eq!(
                Natives.on_event(ctx, "open_door", Some(npc), Some(ObjectId(1))),
                None
            );
            assert!(ctx.has_quest_timer("close_doors", Some(npc), Some(ObjectId(1))));
            // Two marks are left: the other page.
            assert_eq!(
                Natives
                    .on_event(ctx, "open_door", Some(npc), Some(ObjectId(1)))
                    .as_deref(),
                Some("32364-01.htm")
            );
        });
        assert_eq!(t.count(1, MARK_OF_BETRAYAL), 2);
        assert_eq!(t.world.global_variables().get_int("HBTrust", 0), 950);
    }
}
