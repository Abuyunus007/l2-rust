//! Quest 701: Proof of Existence (War Mage Artius at the Barrier Orb, Banshee Queen Enira and her floating undead).
// Java: dist/game/data/scripts/quests/Q00701_ProofOfExistence/Q00701_ProofOfExistence.java
//
// Pattern: a repeatable kill-and-collect quest. Enira drops 1 to 18 Banshee Queen's Eyes (a
// five-step chance), the floating undead drop one Deadman's Remains each with their own chance.
// Artius pays per remains, with a bonus for the eye, and the quest is offered after "Good Day to
// Fly" (Q10273).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ARTIUS: i32 = 32559;

// Items
const DEADMANS_REMAINS: i32 = 13875;
const BANSHEE_QUEENS_EYE: i32 = 13876;

// Monsters
const ENIRA: i32 = 25625;

/// Java `MOBS`: the floating undead and their remains chance (per mille).
const MOBS: [(i32, i32); 6] = [
    (22606, 518), // Floating Skull
    (22607, 858), // Floating Skull
    (22608, 482), // Floating Zombie
    (22609, 466), // Floating Zombie
    (25629, 735), // Floating Skull (Enira's Evil Spirit)
    (25630, 391), // Floating Zombie (Enira's Evil Spirit)
];

// Misc
const MIN_LEVEL: i32 = 78;

/// Java `Q00701_ProofOfExistence`.
pub struct Q00701ProofOfExistence;

impl Script for Q00701ProofOfExistence {
    fn name(&self) -> &'static str {
        "Q00701_ProofOfExistence"
    }

    fn path(&self) -> &'static str {
        "quests/Q00701_ProofOfExistence"
    }

    fn id(&self) -> i32 {
        701
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ARTIUS]);
        s.add_talk_id(&[ARTIUS]);
        s.add_kill_id(&MOBS.map(|(id, _)| id));
        s.add_kill_id(&[ENIRA]);
        s.register_quest_items(&[DEADMANS_REMAINS, BANSHEE_QUEENS_EYE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let st = ctx.get_quest_state(player, false)?;

        match event {
            "32559-03.htm" | "32559-08.html" => Some(event.to_owned()),
            "32559-04.htm" => {
                st.start_quest(ctx);
                Some(event.to_owned())
            }
            "32559-09.html" => {
                st.exit_quest(ctx, true, true);
                Some(event.to_owned())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java draws a member only as a gate (no member, no drop); the items go to the killer.
        if ctx.get_random_party_member_cond(killer, 1).is_none() {
            return;
        }

        if npc.id == ENIRA {
            let chance = ctx.get_random(1000);
            let count = if chance < 708 {
                ctx.get_random(2) + 1
            } else if chance < 978 {
                ctx.get_random(3) + 3
            } else if chance < 994 {
                ctx.get_random(4) + 6
            } else if chance < 998 {
                ctx.get_random(4) + 10
            } else {
                ctx.get_random(5) + 14
            };
            ctx.give_items(killer, BANSHEE_QUEENS_EYE, i64::from(count));
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        } else if let Some((_, chance)) = MOBS.iter().find(|(id, _)| *id == npc.id)
            && ctx.get_random(1000) < *chance
        {
            ctx.give_items(killer, DEADMANS_REMAINS, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let st = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match st.state(ctx) {
            CREATED => {
                let good_day_to_fly_done = ctx
                    .player_quest_state(player, "Q10273_GoodDayToFly")
                    .is_some_and(|q| q.is_completed());
                htmltext = if ctx.player_level(player) >= MIN_LEVEL && good_day_to_fly_done {
                    "32559-01.htm"
                } else {
                    "32559-02.htm"
                }
                .to_owned();
            }
            STARTED => {
                if ctx.has_quest_items(player, BANSHEE_QUEENS_EYE) {
                    let remains = ctx.get_quest_items_count(player, DEADMANS_REMAINS);
                    let eyes = ctx.get_quest_items_count(player, BANSHEE_QUEENS_EYE);
                    ctx.give_adena(player, remains * 2500 + eyes * 50_000 + 23_835, true);
                    ctx.take_items(player, BANSHEE_QUEENS_EYE, -1);
                    ctx.take_items(player, DEADMANS_REMAINS, -1);
                    htmltext = "32559-07.html".to_owned();
                } else if ctx.has_quest_items(player, DEADMANS_REMAINS) {
                    let remains = ctx.get_quest_items_count(player, DEADMANS_REMAINS);
                    ctx.give_adena(player, remains * 2500, true);
                    ctx.take_items(player, DEADMANS_REMAINS, -1);
                    htmltext = "32559-06.html".to_owned();
                } else {
                    htmltext = "32559-05.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::COMPLETED;
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00701_ProofOfExistence";
    const GOOD_DAY_TO_FLY: &str = "Q10273_GoodDayToFly";

    /// Stand-in for Q10273 (Good Day to Fly, not ported yet): the world only keeps the rows of a
    /// known script.
    struct GoodDayToFlyStandIn;

    impl Script for GoodDayToFlyStandIn {
        fn name(&self) -> &'static str {
            GOOD_DAY_TO_FLY
        }

        fn path(&self) -> &'static str {
            "quests/Q10273_GoodDayToFly"
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    const NPCS: &str = r#"
        <npc id="32559" type="Folk" level="1" name="War Mage Artius"/>
        <npc id="22606" type="Monster" level="78" name="Floating Skull"/>
        <npc id="25625" type="RaidBoss" level="80" name="Banshee Queen Enira"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13875" type="EtcItem" name="Deadman's Remains"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="13876" type="EtcItem" name="Banshee Queen's Eye"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00701ProofOfExistence);
        registry.add(GoodDayToFlyStandIn);
        let pages = Pages::of(&Q00701ProofOfExistence)
            .page("32559-01.htm", &["32559-03.htm"])
            .page("32559-03.htm", &["32559-04.htm"])
            .plain(&["32559-02.htm", "32559-04.htm"])
            .page("32559-05.html", &["32559-08.html", "32559-09.html"])
            .page("32559-06.html", &["32559-08.html", "32559-09.html"])
            .page("32559-07.html", &["32559-08.html", "32559-09.html"])
            .plain(&["32559-08.html", "32559-09.html"])
            .npcs(&[ARTIUS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let artius = t.spawn(ARTIUS, 20);
        (t, artius)
    }

    /// Java `Q10273_GoodDayToFly` completed by the player.
    fn complete_good_day_to_fly(t: &mut ScriptTest) {
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.quest_state_of(GOOD_DAY_TO_FLY, Oid(1), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
    }

    #[test]
    fn the_quest_needs_good_day_to_fly_and_level_78() {
        let (mut t, artius) = setup();
        t.enter(1, 78, vec![]);
        assert!(t.quest_page(1, artius).contains("32559-02.htm"));
        complete_good_day_to_fly(&mut t);
        assert!(t.quest_page(1, artius).contains("32559-01.htm"));
    }

    #[test]
    fn a_level_77_character_is_refused() {
        let (mut t, artius) = setup();
        t.enter(1, 77, vec![]);
        complete_good_day_to_fly(&mut t);
        assert!(t.quest_page(1, artius).contains("32559-02.htm"));
    }

    #[test]
    fn the_undead_and_enira_drop_and_artius_pays() {
        let (mut t, artius) = setup();
        t.enter(1, 80, vec![]);
        complete_good_day_to_fly(&mut t);
        t.quest_page(1, artius);
        assert!(t.event(1, NAME, "32559-03.htm").contains("32559-03.htm"));
        assert!(t.event(1, NAME, "32559-04.htm").contains("32559-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Nothing yet: 05, and the quit link is there.
        assert!(t.quest_page(1, artius).contains("32559-05.html"));

        // The floating skulls drop their remains.
        let mut kills = 0;
        while t.count(1, DEADMANS_REMAINS) < 3 {
            let skull = t.spawn(22606, 60);
            t.kill(1, skull);
            kills += 1;
            assert!(kills < 100, "the undead never dropped their remains");
        }
        let remains = t.count(1, DEADMANS_REMAINS);
        assert!(t.quest_page(1, artius).contains("32559-06.html"));
        assert_eq!(t.count(1, DEADMANS_REMAINS), 0);
        assert_eq!(t.count(1, 57), remains * 2500);
        assert!(t.take_writes().is_empty());

        // Enira drops 1 to 18 eyes; with the eyes and the remains the reward has the bonus.
        let enira = t.spawn(ENIRA, 60);
        t.kill(1, enira);
        let eyes = t.count(1, BANSHEE_QUEENS_EYE);
        assert!((1..=18).contains(&eyes), "{eyes}");
        // The remains the skulls dropped after the last payment, if any.
        let fresh_remains = t.count(1, DEADMANS_REMAINS);
        assert!(t.quest_page(1, artius).contains("32559-07.html"));
        assert_eq!(
            t.count(1, 57),
            remains * 2500 + (fresh_remains * 2500) + eyes * 50_000 + 23_835
        );
        assert_eq!(
            (t.count(1, BANSHEE_QUEENS_EYE), t.count(1, DEADMANS_REMAINS)),
            (0, 0)
        );
        assert!(t.take_writes().is_empty());

        // "That's all for me": the quest ends.
        assert!(t.event(1, NAME, "32559-09.html").contains("32559-09.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
