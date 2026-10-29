//! The Warpgates: players who finished one of the two Hellbound quests may enter the island.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/NPC/Warpgate/Warpgate.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::hellbound::engine;

// NPCs
const WARPGATES: [i32; 6] = [32314, 32315, 32316, 32317, 32318, 32319];

// Locations
const ENTER_LOC: (i32, i32, i32) = (-11272, 236464, -3248);
const REMOVE_LOC: (i32, i32, i32) = (-16555, 209375, -3670);

// Item
const MAP: i32 = 9994;

// Misc
const ZONE: i32 = 40101;

/// Java `Q00130_PathToHellbound.class.getSimpleName()`.
const Q00130_PATH_TO_HELLBOUND: &str = "Q00130_PathToHellbound";
/// Java `Q00133_ThatsBloodyHot.class.getSimpleName()`.
const Q00133_THATS_BLOODY_HOT: &str = "Q00133_ThatsBloodyHot";

/// Java `ai.areas.Hellbound.AI.NPC.Warpgate.Warpgate`.
pub struct Warpgate;

/// Java `canEnter(player)`.
fn can_enter(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    if ctx.world().is_flying(player) {
        return false;
    }

    if ctx.script_settings().hellbound_without_quest {
        return true;
    }

    let completed = |name: &str| {
        ctx.player_quest_state(player, name)
            .is_some_and(|qs| qs.is_completed())
    };
    completed(Q00130_PATH_TO_HELLBOUND) || completed(Q00133_THATS_BLOODY_HOT)
}

impl Script for Warpgate {
    fn name(&self) -> &'static str {
        "Warpgate"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/NPC/Warpgate"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&WARPGATES);
        s.add_first_talk_id(&WARPGATES);
        s.add_talk_id(&WARPGATES);
        s.add_enter_zone_id(&[ZONE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        if event == "enter" {
            if can_enter(ctx, player) {
                ctx.teleport_player(player, ENTER_LOC.0, ENTER_LOC.1, ENTER_LOC.2, 0, true);
            } else {
                return Some("Warpgate-03.html".to_owned());
            }
        } else if event == "TELEPORT" {
            ctx.teleport_player(player, REMOVE_LOC.0, REMOVE_LOC.1, REMOVE_LOC.2, 0, true);
        }

        None
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(
            if engine::is_locked(ctx) {
                "Warpgate-01.html"
            } else {
                "Warpgate-02.html"
            }
            .to_owned(),
        )
    }

    fn on_enter_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, _zone_id: i32) {
        if ctx.world().player(creature).is_some() {
            let player = creature;
            // `player.isOnEvent()` is not modelled (no event is ported that sets it).
            if !can_enter(ctx, player) && !ctx.is_gm(player) {
                ctx.start_quest_timer("TELEPORT", 1000, None, Some(player), false);
            } else if !ctx.world().is_minimap_allowed(player)
                && ctx.has_at_least_one_quest_item(player, &[MAP])
            {
                ctx.world_mut().set_minimap_allowed(player, true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::{QuestLoad, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::ai::areas::hellbound::engine::HellboundEngine;
    use crate::ai::areas::hellbound::testing::{last_page, set_level};
    use crate::testing::{Pages, ScriptTest, item, player_data};

    const NPCS: &str = r#"<npc id="32314" type="Folk" level="70" name="Warpgate"/>"#;
    const ITEMS: &str = r#"<item id="9994" type="EtcItem" name="Map"/>"#;

    /// Java `Q00130_PathToHellbound` as far as the gates need it: a quest of that name.
    struct PathToHellbound;

    impl Script for PathToHellbound {
        fn name(&self) -> &'static str {
            Q00130_PATH_TO_HELLBOUND
        }
        fn path(&self) -> &'static str {
            "quests/Q00130_PathToHellbound"
        }
        fn id(&self) -> i32 {
            130
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(HellboundEngine);
        registry.add(Warpgate);
        registry.add(PathToHellbound);
        let pages = Pages::of(&Warpgate)
            .page("Warpgate-02.html", &["enter"])
            .plain(&["Warpgate-01.html", "Warpgate-03.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let gate = t.spawn(32314, 20);
        (t, gate)
    }

    fn finished_player(id: i32) -> l2_world::PlayerData {
        let mut data = player_data(id, 80, vec![item(900 + id, MAP, 1)]);
        data.profile.quests = QuestLoad {
            states: vec![(Q00130_PATH_TO_HELLBOUND.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        data
    }

    #[test]
    fn the_gate_page_depends_on_the_lock() {
        let (mut t, gate) = setup();
        t.enter(1, 80, vec![]);
        t.talk(1, gate);
        assert!(last_page(&t.take(), 1).contains("Warpgate-01.html"));
        set_level(&mut t, "Warpgate", 1);
        t.talk(1, gate);
        assert!(last_page(&t.take(), 1).contains("Warpgate-02.html"));
    }

    #[test]
    fn only_a_finished_quest_gets_in() {
        let (mut t, gate) = setup();
        set_level(&mut t, "Warpgate", 1);
        t.enter(1, 80, vec![]);
        t.talk(1, gate);
        t.take();
        assert!(t.event(1, "Warpgate", "enter").contains("Warpgate-03.html"));
        t.enter_with(finished_player(2));
        t.talk(2, gate);
        t.take();
        t.bypass(2, "Script Warpgate enter");
        t.ticks(100);
        let (x, y, _) = t.world.position_of(ObjectId(2)).unwrap();
        assert!(
            (x - ENTER_LOC.0).abs() <= 200 && (y - ENTER_LOC.1).abs() <= 200,
            "{x} {y}"
        );
    }

    #[test]
    fn the_zone_throws_out_the_unfinished_and_gives_the_minimap() {
        let (mut t, _gate) = setup();
        t.enter(1, 80, vec![]);
        t.enter_with(finished_player(2));
        t.world.run_as_script("Warpgate", |ctx| {
            Warpgate.on_enter_zone(ctx, ObjectId(1), ZONE);
            assert!(ctx.has_quest_timer("TELEPORT", None, Some(ObjectId(1))));
            Warpgate.on_enter_zone(ctx, ObjectId(2), ZONE);
            assert!(!ctx.has_quest_timer("TELEPORT", None, Some(ObjectId(2))));
        });
        assert!(t.world.is_minimap_allowed(ObjectId(2)));
        assert!(!t.world.is_minimap_allowed(ObjectId(1)));
    }
}
