//! Quest 618: Into the Flame (Klein in the Town of Gludin area, Hilda in the Hot Springs area; the
//! lava monsters drop vacualite ore, 50 of them make the vacualite that Hilda forges).
// Java: dist/game/data/scripts/quests/Q00618_IntoTheFlame/Q00618_IntoTheFlame.java
//
// Pattern: a three-NPC chain (the start NPC, a second NPC that moves the cond, and the reward),
// the ore drop goes to a random member at cond 2 (`getRandomPartyMember(player, 2)`), and the
// Java quirk that the limit check reads the killer's count.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const KLEIN: i32 = 31540;
const HILDA: i32 = 31271;

// Monsters: the vacualite ore drop chance (per mille) of each lava monster.
const MONSTERS: [(i32, i32); 8] = [
    (21274, 630),
    (21276, 630),
    (21282, 670),
    (21283, 670),
    (21284, 670),
    (21290, 710),
    (21291, 710),
    (21292, 710),
];

// Items
const VACUALITE_ORE: i32 = 7265;
const VACUALITE: i32 = 7266;
const VACUALITE_FLOATING_STONE: i32 = 7267;

// Misc
const MIN_LEVEL: i32 = 60;
const REQUIRED_COUNT: i64 = 50;

/// Java `Q00618_IntoTheFlame`.
pub struct Q00618IntoTheFlame;

impl Script for Q00618IntoTheFlame {
    fn name(&self) -> &'static str {
        "Q00618_IntoTheFlame"
    }

    fn path(&self) -> &'static str {
        "quests/Q00618_IntoTheFlame"
    }

    fn id(&self) -> i32 {
        618
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KLEIN]);
        s.add_talk_id(&[HILDA, KLEIN]);
        let mobs: Vec<i32> = MONSTERS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[VACUALITE_ORE, VACUALITE]);
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
            "31540-03.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "31540-05.html" => {
                if !ctx.has_quest_items(player, VACUALITE) {
                    htmltext = Some("31540-03.htm".to_owned());
                } else {
                    ctx.give_items(player, VACUALITE_FLOATING_STONE, 1);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31271-02.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31271-05.html" => {
                if ctx.get_quest_items_count(player, VACUALITE_ORE) == REQUIRED_COUNT && qs.is_cond(ctx, 3) {
                    ctx.take_items(player, VACUALITE_ORE, -1);
                    ctx.give_items(player, VACUALITE, 1);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("31271-03.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMember(player, 2)`: a member at cond 2.
        let Some(member) = ctx.get_random_party_member_cond(killer, 2) else {
            return;
        };
        let Some(qs) = ctx.get_quest_state(member, false) else {
            return;
        };
        let Some(&(_, chance)) = MONSTERS.iter().find(|(id, _)| *id == npc.id) else {
            return;
        };
        // NOTE(java-quirk): the limit is read from the killer, not from the member who gets it.
        if ctx.get_quest_items_count(killer, VACUALITE_ORE) < REQUIRED_COUNT && ctx.get_random(1000) < chance
        {
            ctx.give_items(member, VACUALITE_ORE, 1);
            if ctx.get_quest_items_count(member, VACUALITE_ORE) >= REQUIRED_COUNT {
                qs.set_cond(ctx, 3, true);
            } else {
                ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if npc.id == KLEIN {
            if qs.is_created(ctx) {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "31540-01.html".to_owned()
                } else {
                    "31540-02.htm".to_owned()
                };
            } else if qs.is_started(ctx) {
                htmltext = if qs.is_cond(ctx, 4) {
                    "31540-04.html".to_owned()
                } else {
                    "31540-03.htm".to_owned()
                };
            }
        } else if npc.id == HILDA {
            match qs.get_cond(ctx) {
                1 => htmltext = "31271-01.html".to_owned(),
                2 => htmltext = "31271-03.html".to_owned(),
                3 => htmltext = "31271-04.html".to_owned(),
                4 => htmltext = "31271-06.html".to_owned(),
                _ => {}
            }
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

    const NAME: &str = "Q00618_IntoTheFlame";

    const NPCS: &str = r#"
        <npc id="31540" type="Folk" level="70" name="Klein"/>
        <npc id="31271" type="Folk" level="70" name="Hilda"/>
        <npc id="21274" type="Monster" level="60" name="Lava Basilisk"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7265" type="EtcItem" name="Vacualite Ore"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7266" type="EtcItem" name="Vacualite"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7267" type="EtcItem" name="Vacualite Floating Stone"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00618IntoTheFlame);
        let pages = Pages::of(&Q00618IntoTheFlame)
            .page("31540-02.htm", &["31540-03.htm"])
            .page("31540-03.htm", &[])
            .plain(&["31540-01.html", "31540-05.html"])
            .page("31540-04.html", &["31540-05.html"])
            .page("31271-01.html", &["31271-02.html"])
            .plain(&["31271-02.html", "31271-03.html", "31271-06.html"])
            .page("31271-04.html", &["31271-05.html"])
            .plain(&["31271-05.html"])
            .npcs(&[KLEIN, HILDA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let klein = t.spawn(KLEIN, 20);
        let hilda = t.spawn(HILDA, 40);
        (t, klein, hilda)
    }

    /// A lava basilisk (ore chance 630 per mille) dies next to the player.
    fn kill_one(t: &mut ScriptTest) {
        let mob = t.spawn(21274, 60);
        t.kill(1, mob);
    }

    #[test]
    fn a_character_below_level_60_is_sent_away() {
        let (mut t, klein, _) = setup();
        t.enter(1, 59, vec![]);
        assert!(t.quest_page(1, klein).contains("31540-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_ore_is_forged_into_vacualite_and_the_stone_ends_the_quest() {
        let (mut t, klein, hilda) = setup();
        t.enter(1, 60, vec![]);
        assert!(t.quest_page(1, klein).contains("31540-02.htm"));
        assert!(t.event(1, NAME, "31540-03.htm").contains("31540-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Hilda at cond 1 hands over the ore job: cond 2.
        assert!(t.quest_page(1, hilda).contains("31271-01.html"));
        assert!(t.event(1, NAME, "31271-02.html").contains("31271-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Ore at cond 2: 49 by the script, the next one makes cond 3.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), VACUALITE_ORE, 49);
        });
        t.take();
        let mut kills = 0;
        while t.count(1, VACUALITE_ORE) < REQUIRED_COUNT {
            kill_one(&mut t);
            kills += 1;
            assert!(kills < 100, "no ore dropped");
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Hilda at cond 3: the vacualite, cond 4.
        assert!(t.quest_page(1, hilda).contains("31271-04.html"));
        assert!(t.event(1, NAME, "31271-05.html").contains("31271-05.html"));
        assert_eq!(t.count(1, VACUALITE_ORE), 0);
        assert_eq!(t.count(1, VACUALITE), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        assert!(t.quest_page(1, hilda).contains("31271-06.html"));

        // Klein at cond 4: the floating stone, the quest ends.
        assert!(t.quest_page(1, klein).contains("31540-04.html"));
        assert!(t.event(1, NAME, "31540-05.html").contains("31540-05.html"));
        assert_eq!(t.count(1, VACUALITE_FLOATING_STONE), 1);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn klein_sends_the_character_back_without_vacualite() {
        let (mut t, klein, _) = setup();
        t.enter(1, 60, vec![]);
        t.quest_page(1, klein);
        t.event(1, NAME, "31540-03.htm");
        t.take_writes();
        // Cond 4 without the vacualite: the start page again, nothing written.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 4, false);
        });
        t.take_writes();
        assert!(t.quest_page(1, klein).contains("31540-04.html"));
        assert!(t.event(1, NAME, "31540-05.html").contains("31540-03.htm"));
        assert!(t.take_writes().is_empty());
    }
}
