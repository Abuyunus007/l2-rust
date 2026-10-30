//! Quest 634: In Search of Fragments of Dimension (the Dimensional Gate Keepers of the Dimensional
//! Rift; the monsters of the Holy Lands and the Sepulcher drop dimensional fragments).
// Java: dist/game/data/scripts/quests/Q00634_InSearchOfFragmentsOfDimension/Q00634_InSearchOfFragmentsOfDimension.java
//
// Pattern: a repeatable kill drop of a number that depends on the monster's level
// (`(int) (0.15 * level + 1.6)`), given to a random started party member (counted three times),
// with a 10 % roll per kill.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// Gate keepers
const DIMENSIONAL_GATE_KEEPER: [i32; 14] = [
    31494, 31495, 31496, 31497, 31498, 31499, 31500, 31501, 31502, 31503, 31504, 31505, 31506, 31507,
];

// Item
const DIMENSIONAL_FRAGMENT: i32 = 7079;

// Misc
const MIN_LEVEL: i32 = 20;

/// Java `MOBS`: the monsters that can drop the fragments.
const MOBS: [i32; 49] = [
    21208, 21209, 21210, 21211, 21212, 21213, 21214, 21215, 21216, 21217, 21218, 21219, 21220, 21221, 21222,
    21223, 21224, 21225, 21226, 21227, 21228, 21229, 21230, 21231, 21232, 21233, 21234, 21235, 21236, 21237,
    21238, 21239, 21240, 21241, 21242, 21243, 21244, 21245, 21246, 21247, 21248, 21249, 21250, 21251, 21252,
    21253, 21254, 21255, 21256,
];

/// Java `Q00634_InSearchOfFragmentsOfDimension`.
pub struct Q00634InSearchOfFragmentsOfDimension;

impl Script for Q00634InSearchOfFragmentsOfDimension {
    fn name(&self) -> &'static str {
        "Q00634_InSearchOfFragmentsOfDimension"
    }

    fn path(&self) -> &'static str {
        "quests/Q00634_InSearchOfFragmentsOfDimension"
    }

    fn id(&self) -> i32 {
        634
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&DIMENSIONAL_GATE_KEEPER);
        s.add_talk_id(&DIMENSIONAL_GATE_KEEPER);
        s.add_kill_id(&MOBS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        let mut htmltext = None;
        match event {
            "31494-02.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "31494-05.html" | "31494-06.html" => {
                if qs.is_started(ctx) {
                    htmltext = Some(event.to_owned());
                }
            }
            "31494-07.html" => {
                if qs.is_started(ctx) {
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMemberState(player, -1, 3, npc)`: the killer counts three times.
        let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) else {
            return;
        };
        // Java `(int) ((0.15 * npc.getLevel()) + 1.6)`.
        let level = ctx.world().stats(npc.object_id).map_or(0, |s| s.level());
        let amount = (0.15 * f64::from(level) + 1.6) as i64;
        if ctx.get_random(100) < 10 {
            ctx.give_item_randomly(
                qs.player(),
                Some(npc),
                DIMENSIONAL_FRAGMENT,
                amount,
                amount,
                0,
                1.0,
                true,
            );
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "31494-01.htm".to_owned()
            } else {
                "31494-03.htm".to_owned()
            };
        } else if state == STARTED {
            htmltext = "31494-04.html".to_owned();
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00634_InSearchOfFragmentsOfDimension";

    const NPCS: &str = r#"
        <npc id="31494" type="Folk" level="70" name="Dimensional Gate Keeper"/>
        <npc id="21208" type="Monster" level="70" name="Hallowed Watchman"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7079" type="EtcItem" name="Dimensional Fragment"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00634InSearchOfFragmentsOfDimension);
        let pages = Pages::of(&Q00634InSearchOfFragmentsOfDimension)
            .page("31494-01.htm", &["31494-02.htm"])
            .plain(&[
                "31494-02.htm",
                "31494-03.htm",
                "31494-05.html",
                "31494-06.html",
                "31494-07.html",
            ])
            .page(
                "31494-04.html",
                &["31494-05.html", "31494-06.html", "31494-07.html"],
            )
            .npcs(&[31494]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let gate = t.spawn(31494, 20);
        (t, gate)
    }

    #[test]
    fn a_character_below_level_20_is_sent_away() {
        let (mut t, gate) = setup();
        t.enter(1, 19, vec![]);
        assert!(t.quest_page(1, gate).contains("31494-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_fragments_come_in_level_sized_piles_and_the_quest_ends() {
        let (mut t, gate) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, gate).contains("31494-01.htm"));
        assert!(t.event(1, NAME, "31494-02.htm").contains("31494-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, gate).contains("31494-04.html"));

        // A level 70 monster gives (int) (0.15 * 70 + 1.6) = 12 fragments, with a 10 % roll.
        let mut kills = 0;
        while t.count(1, DIMENSIONAL_FRAGMENT) == 0 {
            let mob = t.spawn(21208, 40);
            t.kill(1, mob);
            kills += 1;
            assert!(kills < 200, "no fragments dropped");
        }
        assert_eq!(t.count(1, DIMENSIONAL_FRAGMENT), 12);
        assert!(t.take_writes().is_empty());

        assert!(t.quest_page(1, gate).contains("31494-04.html"));
        assert!(t.event(1, NAME, "31494-05.html").contains("31494-05.html"));
        // Page 05 has no links: back to the talk, then the exit.
        assert!(t.quest_page(1, gate).contains("31494-04.html"));
        assert!(t.event(1, NAME, "31494-07.html").contains("31494-07.html"));
        assert_eq!(t.count(1, DIMENSIONAL_FRAGMENT), 12, "after the exit");
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
