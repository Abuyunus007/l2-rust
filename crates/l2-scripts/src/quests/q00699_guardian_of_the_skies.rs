//! Quest 699: Guardian of the Skies (Engineer Lekon at the Keucereus Alliance Base, the Vulture Riders of Gracia).
// Java: dist/game/data/scripts/quests/Q00699_GuardianOfTheSkies/Q00699_GuardianOfTheSkies.java
//
// Pattern: a repeatable kill-and-collect quest. Master Rider Valdstone drops 60 to 99 feathers
// (a three-step chance); the Vulture Riders drop one with a per-monster chance. Lekon pays per
// feather with a bonus above ten. The quest is offered after "Good Day to Fly" (Q10273).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const LEKON: i32 = 32557;

// Monsters
const VALDSTONE: i32 = 25623;

/// Java `MONSTERS`: the Vulture Riders and their feather chance (per mille).
const MONSTERS: [(i32, i32); 3] = [
    (22614, 840), // Vulture Rider lvl 1
    (22615, 857), // Vulture Rider lvl 2
    (25633, 719), // Vulture Rider lvl 3
];

// Item
const VULTURES_GOLDEN_FEATHER: i32 = 13871;

// Misc
const MIN_LEVEL: i32 = 75;
const VULTURES_GOLDEN_FEATHER_ADENA: i64 = 1500;
const BONUS: i64 = 8335;
const BONUS_COUNT: i64 = 10;

/// Java `Q00699_GuardianOfTheSkies`.
pub struct Q00699GuardianOfTheSkies;

impl Script for Q00699GuardianOfTheSkies {
    fn name(&self) -> &'static str {
        "Q00699_GuardianOfTheSkies"
    }

    fn path(&self) -> &'static str {
        "quests/Q00699_GuardianOfTheSkies"
    }

    fn id(&self) -> i32 {
        699
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LEKON]);
        s.add_talk_id(&[LEKON]);
        s.add_kill_id(&[VALDSTONE]);
        s.add_kill_id(&MONSTERS.map(|(id, _)| id));
        s.register_quest_items(&[VULTURES_GOLDEN_FEATHER]);
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
            "32557-03.htm" | "32557-08.html" => htmltext = Some(event.to_owned()),
            "32557-04.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32557-09.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java checks only that the killer has a quest state, whatever its state.
        if ctx.get_quest_state(killer, false).is_none() {
            return;
        }
        if npc.id == VALDSTONE {
            let amount = match ctx.get_random(1000) {
                chance if chance < 215 => ctx.get_random(10) + 90,
                chance if chance < 446 => ctx.get_random(10) + 80,
                chance if chance < 715 => ctx.get_random(10) + 70,
                _ => ctx.get_random(10) + 60,
            };
            ctx.give_items(killer, VULTURES_GOLDEN_FEATHER, i64::from(amount));
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        } else if let Some((_, chance)) = MONSTERS.iter().find(|(id, _)| *id == npc.id)
            && ctx.get_random(1000) < *chance
        {
            ctx.give_items(killer, VULTURES_GOLDEN_FEATHER, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                let good_day_to_fly_done = ctx
                    .player_quest_state(player, "Q10273_GoodDayToFly")
                    .is_some_and(|q| q.is_completed());
                htmltext = if good_day_to_fly_done && ctx.player_level(player) >= MIN_LEVEL {
                    "32557-01.htm"
                } else {
                    "32557-02.htm"
                }
                .to_owned();
            }
            STARTED => {
                let feathers = ctx.get_quest_items_count(player, VULTURES_GOLDEN_FEATHER);
                if feathers > 0 {
                    let bonus = if feathers > BONUS_COUNT { BONUS } else { 0 };
                    ctx.give_adena(player, feathers * VULTURES_GOLDEN_FEATHER_ADENA + bonus, true);
                    ctx.take_items(player, VULTURES_GOLDEN_FEATHER, -1);
                    htmltext = if feathers > BONUS_COUNT {
                        "32557-07.html"
                    } else {
                        "32557-06.html"
                    }
                    .to_owned();
                } else {
                    htmltext = "32557-05.html".to_owned();
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

    const NAME: &str = "Q00699_GuardianOfTheSkies";
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
        <npc id="32557" type="Folk" level="1" name="Engineer Lekon"/>
        <npc id="25623" type="RaidBoss" level="78" name="Master Rider Valdstone"/>
        <npc id="22614" type="Monster" level="78" name="Vulture Rider"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13871" type="EtcItem" name="Vulture's Golden Feather"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00699GuardianOfTheSkies);
        registry.add(GoodDayToFlyStandIn);
        let pages = Pages::of(&Q00699GuardianOfTheSkies)
            .page("32557-01.htm", &["32557-03.htm"])
            .page("32557-03.htm", &["32557-04.htm"])
            .plain(&["32557-02.htm", "32557-04.htm"])
            .page("32557-05.html", &["32557-08.html", "32557-09.html"])
            .page("32557-06.html", &["32557-08.html", "32557-09.html"])
            .page("32557-07.html", &["32557-08.html", "32557-09.html"])
            .plain(&["32557-08.html", "32557-09.html"])
            .npcs(&[LEKON]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let lekon = t.spawn(LEKON, 20);
        (t, lekon)
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
    fn the_quest_needs_good_day_to_fly_and_level_75() {
        let (mut t, lekon) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, lekon).contains("32557-02.htm"));

        complete_good_day_to_fly(&mut t);
        assert!(t.quest_page(1, lekon).contains("32557-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_level_74_character_is_refused_even_after_good_day_to_fly() {
        let (mut t, lekon) = setup();
        t.enter(1, 74, vec![]);
        complete_good_day_to_fly(&mut t);
        assert!(t.quest_page(1, lekon).contains("32557-02.htm"));
    }

    #[test]
    fn valdstone_drops_feathers_and_lekon_pays_with_the_bonus() {
        let (mut t, lekon) = setup();
        t.enter(1, 75, vec![]);
        complete_good_day_to_fly(&mut t);
        assert!(t.quest_page(1, lekon).contains("32557-01.htm"));
        t.event(1, NAME, "32557-03.htm");
        assert!(t.event(1, NAME, "32557-04.htm").contains("32557-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Nothing to hand in yet.
        assert!(t.quest_page(1, lekon).contains("32557-05.html"));
        assert!(t.take_writes().is_empty());

        // Valdstone drops 60 to 99 feathers.
        let valdstone = t.spawn(VALDSTONE, 60);
        t.kill(1, valdstone);
        let feathers = t.count(1, VULTURES_GOLDEN_FEATHER);
        assert!((60..=99).contains(&feathers), "{feathers}");

        // More than ten feathers: the bonus and page 07.
        assert!(t.quest_page(1, lekon).contains("32557-07.html"));
        assert_eq!(t.count(1, VULTURES_GOLDEN_FEATHER), 0);
        assert_eq!(t.count(1, 57), feathers * 1500 + 8335);
        assert!(t.take_writes().is_empty());

        // "No, I've had enough": the quest ends.
        assert!(t.event(1, NAME, "32557-09.html").contains("32557-09.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_few_feathers_get_no_bonus() {
        let (mut t, lekon) = setup();
        t.enter(1, 75, vec![]);
        complete_good_day_to_fly(&mut t);
        t.quest_page(1, lekon);
        t.event(1, NAME, "32557-03.htm");
        t.event(1, NAME, "32557-04.htm");
        t.take_writes();

        // A Vulture Rider drops a feather with an 84% chance per kill: loop until one drops.
        let mut kills = 0;
        while t.count(1, VULTURES_GOLDEN_FEATHER) == 0 {
            let rider = t.spawn(22614, 60);
            t.kill(1, rider);
            kills += 1;
            assert!(kills < 100, "the riders never dropped a feather");
        }
        let feathers = t.count(1, VULTURES_GOLDEN_FEATHER);
        assert!(feathers <= 10);
        assert!(t.quest_page(1, lekon).contains("32557-06.html"));
        assert_eq!(t.count(1, 57), feathers * 1500);
    }
}
