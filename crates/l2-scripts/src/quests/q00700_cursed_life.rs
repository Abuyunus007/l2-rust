//! Quest 700: Cursed Life (Orbyu at the Keucereus Alliance Base, the Mutant Birds and Dra Hawks
//! of Gracia).
// Java: dist/game/data/scripts/quests/Q00700_CursedLife/Q00700_CursedLife.java
//
// Pattern: a repeatable kill-and-collect quest like Q00699: Rok drops three kinds of swallowed
// remains in a many-step chance, the other monsters drop one remain by a per-monster chance,
// Orbyu pays per piece with a bonus from ten pieces. Offered after "Good Day to Fly" (Q10273).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ORBYU: i32 = 32560;

// Monsters
const ROK: i32 = 25624;
/// Java `MONSTERS`: id and the thresholds (per mille) of bones, sternum and skull.
const MONSTERS: [(i32, [i32; 3]); 6] = [
    (22602, [15, 139, 965]), // Mutant Bird lvl 1
    (22603, [15, 143, 999]), // Mutant Bird lvl 2
    (25627, [14, 125, 993]), // Mutant Bird lvl 3
    (22604, [5, 94, 994]),   // Dra Hawk lvl 1
    (22605, [5, 99, 993]),   // Dra Hawk lvl 2
    (25628, [3, 73, 991]),   // Dra Hawk lvl 3
];

// Items
const SWALLOWED_BONES: i32 = 13874;
const SWALLOWED_STERNUM: i32 = 13873;
const SWALLOWED_SKULL: i32 = 13872;

// Misc
const MIN_LEVEL: i32 = 75;
const SWALLOWED_BONES_ADENA: i64 = 500;
const SWALLOWED_STERNUM_ADENA: i64 = 5000;
const SWALLOWED_SKULL_ADENA: i64 = 50000;
const BONUS: i64 = 16670;

/// Java `Q00700_CursedLife`.
pub struct Q00700CursedLife;

impl Script for Q00700CursedLife {
    fn name(&self) -> &'static str {
        "Q00700_CursedLife"
    }

    fn path(&self) -> &'static str {
        "quests/Q00700_CursedLife"
    }

    fn id(&self) -> i32 {
        700
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ORBYU]);
        s.add_talk_id(&[ORBYU]);
        s.add_kill_id(&[ROK]);
        s.add_kill_id(&MONSTERS.map(|(id, _)| id));
        s.register_quest_items(&[SWALLOWED_BONES, SWALLOWED_STERNUM, SWALLOWED_SKULL]);
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
            "32560-02.htm" => {
                let good_day_to_fly_done = ctx
                    .player_quest_state(player, "Q10273_GoodDayToFly")
                    .is_some_and(|q| q.is_completed());
                htmltext = Some(if ctx.player_level(player) < MIN_LEVEL || !good_day_to_fly_done {
                    "32560-03.htm".to_owned()
                } else {
                    event.to_owned()
                });
            }
            "32560-04.htm" | "32560-09.html" => htmltext = Some(event.to_owned()),
            "32560-05.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32560-10.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => htmltext = "32560-01.htm".to_owned(),
            STARTED => {
                let bones = ctx.get_quest_items_count(player, SWALLOWED_BONES);
                let ribs = ctx.get_quest_items_count(player, SWALLOWED_STERNUM);
                let skulls = ctx.get_quest_items_count(player, SWALLOWED_SKULL);
                let sum = bones + ribs + skulls;
                if sum > 0 {
                    let bonus = if sum >= 10 { BONUS } else { 0 };
                    ctx.give_adena(
                        player,
                        bones * SWALLOWED_BONES_ADENA
                            + ribs * SWALLOWED_STERNUM_ADENA
                            + skulls * SWALLOWED_SKULL_ADENA
                            + bonus,
                        true,
                    );
                    ctx.take_items_many(player, -1, &[SWALLOWED_BONES, SWALLOWED_STERNUM, SWALLOWED_SKULL]);
                    htmltext = if sum < 10 {
                        "32560-07.html"
                    } else {
                        "32560-08.html"
                    }
                    .to_owned();
                } else {
                    htmltext = "32560-06.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java checks only that the killer has a quest state, whatever its state.
        if ctx.get_quest_state(killer, false).is_none() {
            return;
        }
        if npc.id == ROK {
            let chance = ctx.get_random(1000);
            let amount = if chance < 700 {
                1
            } else if chance < 885 {
                2
            } else if chance < 949 {
                3
            } else if chance < 966 {
                ctx.get_random(5) + 4
            } else if chance < 985 {
                ctx.get_random(9) + 4
            } else if chance < 993 {
                ctx.get_random(7) + 13
            } else if chance < 997 {
                ctx.get_random(15) + 9
            } else if chance < 999 {
                ctx.get_random(23) + 53
            } else {
                ctx.get_random(49) + 76
            };
            ctx.give_items(killer, SWALLOWED_BONES, i64::from(amount));

            let chance = ctx.get_random(1000);
            let amount = if chance < 520 {
                1
            } else if chance < 771 {
                2
            } else if chance < 836 {
                3
            } else if chance < 985 {
                ctx.get_random(2) + 4
            } else if chance < 995 {
                ctx.get_random(4) + 5
            } else {
                ctx.get_random(8) + 6
            };
            ctx.give_items(killer, SWALLOWED_STERNUM, i64::from(amount));

            let chance = ctx.get_random(1000);
            let amount = if chance < 185 {
                ctx.get_random(2) + 1
            } else if chance < 370 {
                ctx.get_random(6) + 2
            } else if chance < 570 {
                ctx.get_random(6) + 7
            } else if chance < 850 {
                ctx.get_random(6) + 12
            } else {
                ctx.get_random(6) + 17
            };
            ctx.give_items(killer, SWALLOWED_SKULL, i64::from(amount));
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        } else if let Some((_, chances)) = MONSTERS.iter().find(|(id, _)| *id == npc.id) {
            let chance = ctx.get_random(1000);
            let item = if chance < chances[0] {
                SWALLOWED_BONES
            } else if chance < chances[1] {
                SWALLOWED_STERNUM
            } else if chance < chances[2] {
                SWALLOWED_SKULL
            } else {
                return;
            };
            ctx.give_items(killer, item, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::COMPLETED;

    use super::*;
    use crate::quests::q10273_good_day_to_fly::Q10273GoodDayToFly;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00700_CursedLife";
    const GOOD_DAY_TO_FLY: &str = "Q10273_GoodDayToFly";

    const NPCS: &str = r#"
        <npc id="32560" type="Folk" level="1" name="Orbyu"/>
        <npc id="25624" type="RaidBoss" level="78" name="Rok"/>
        <npc id="22602" type="Monster" level="78" name="Mutant Bird"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13872" type="EtcItem" name="Swallowed Skull"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="13873" type="EtcItem" name="Swallowed Sternum"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="13874" type="EtcItem" name="Swallowed Bones"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00700CursedLife);
        registry.add(Q10273GoodDayToFly);
        let pages = Pages::of(&Q00700CursedLife)
            .page("32560-01.htm", &["32560-02.htm"])
            .page("32560-02.htm", &["32560-04.htm"])
            .page("32560-04.htm", &["32560-05.htm"])
            .plain(&["32560-03.htm", "32560-05.htm"])
            .page("32560-06.html", &["32560-09.html"])
            .page("32560-07.html", &["32560-09.html"])
            .page("32560-08.html", &["32560-09.html"])
            .page("32560-09.html", &["32560-10.html"])
            .plain(&["32560-10.html"])
            .npcs(&[ORBYU]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let orbyu = t.spawn(ORBYU, 20);
        (t, orbyu)
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
        let (mut t, orbyu) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, orbyu).contains("32560-01.htm"));
        assert!(t.event(1, NAME, "32560-02.htm").contains("32560-03.htm"));

        complete_good_day_to_fly(&mut t);
        t.quest_page(1, orbyu);
        assert!(t.event(1, NAME, "32560-02.htm").contains("32560-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_level_74_character_is_refused_even_after_good_day_to_fly() {
        let (mut t, orbyu) = setup();
        t.enter(1, 74, vec![]);
        complete_good_day_to_fly(&mut t);
        t.quest_page(1, orbyu);
        assert!(t.event(1, NAME, "32560-02.htm").contains("32560-03.htm"));
    }

    #[test]
    fn rok_drops_remains_and_orbyu_pays_with_the_bonus() {
        let (mut t, orbyu) = setup();
        t.enter(1, 75, vec![]);
        complete_good_day_to_fly(&mut t);
        t.quest_page(1, orbyu);
        t.event(1, NAME, "32560-02.htm");
        t.event(1, NAME, "32560-04.htm");
        assert!(t.event(1, NAME, "32560-05.htm").contains("32560-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Nothing to hand in yet.
        assert!(t.quest_page(1, orbyu).contains("32560-06.html"));

        // Rok drops at least one of each kind: ten pieces or more for sure only when it adds up.
        let rok = t.spawn(ROK, 60);
        t.kill(1, rok);
        let (bones, ribs, skulls) = (
            t.count(1, SWALLOWED_BONES),
            t.count(1, SWALLOWED_STERNUM),
            t.count(1, SWALLOWED_SKULL),
        );
        assert!(bones >= 1 && ribs >= 1 && skulls >= 1);
        let sum = bones + ribs + skulls;
        let bonus = if sum >= 10 { BONUS } else { 0 };
        let page = if sum >= 10 {
            "32560-08.html"
        } else {
            "32560-07.html"
        };
        t.take_writes();
        assert!(t.quest_page(1, orbyu).contains(page));
        assert_eq!(t.count(1, 57), bones * 500 + ribs * 5000 + skulls * 50000 + bonus);
        assert_eq!(t.count(1, SWALLOWED_BONES), 0);

        // "No, I've had enough": the quest ends.
        t.event(1, NAME, "32560-09.html");
        assert!(t.event(1, NAME, "32560-10.html").contains("32560-10.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_mutant_bird_drops_one_remain_by_chance() {
        let (mut t, orbyu) = setup();
        t.enter(1, 75, vec![]);
        complete_good_day_to_fly(&mut t);
        t.quest_page(1, orbyu);
        t.event(1, NAME, "32560-02.htm");
        t.event(1, NAME, "32560-04.htm");
        t.event(1, NAME, "32560-05.htm");
        t.take_writes();

        // 96.5% of the kills drop a skull, sternum or bones piece at most one each.
        let mut kills = 0;
        let total = |t: &ScriptTest| {
            t.count(1, SWALLOWED_BONES) + t.count(1, SWALLOWED_STERNUM) + t.count(1, SWALLOWED_SKULL)
        };
        while total(&t) == 0 {
            let bird = t.spawn(22602, 60);
            t.kill(1, bird);
            kills += 1;
            assert!(kills < 100, "the birds never dropped anything");
        }
        assert!(total(&t) <= kills);
    }
}
