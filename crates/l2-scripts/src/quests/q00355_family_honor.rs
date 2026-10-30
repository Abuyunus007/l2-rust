//! Quest 355: Family Honor (Galibredo in the Town of Rune).
// Java: dist/game/data/scripts/quests/Q00355_FamilyHonor/Q00355_FamilyHonor.java
//
// Pattern: a repeatable kill-and-sell quest. Timak orcs drop Galfredo Romer's busts (sold to
// Galibredo for adena) or a Sculptor Berona, which Patrin turns into an ancient statue.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const GALIBREDO: i32 = 30181;
const PATRIN: i32 = 30929;

// Items
const GALFREDO_ROMERS_BUST: i32 = 4252;
const SCULPTOR_BERONA: i32 = 4350;
const ANCIENT_STATUE_PROTOTYPE: i32 = 4351;
const ANCIENT_STATUE_ORIGINAL: i32 = 4352;
const ANCIENT_STATUE_REPLICA: i32 = 4353;
const ANCIENT_STATUE_FORGERY: i32 = 4354;

// Misc
const MIN_LEVEL: i32 = 36;

/// Java `DropInfo`: `(npc id, first chance, second chance)` out of 1000.
const MOBS: [(i32, i32, i32); 4] = [
    (20767, 560, 684), // timak_orc_troop_leader
    (20768, 530, 650), // timak_orc_troop_shaman
    (20769, 420, 516), // timak_orc_troop_warrior
    (20770, 440, 560), // timak_orc_troop_archer
];

/// Java `Q00355_FamilyHonor`.
pub struct Q00355FamilyHonor;

impl Script for Q00355FamilyHonor {
    fn name(&self) -> &'static str {
        "Q00355_FamilyHonor"
    }

    fn path(&self) -> &'static str {
        "quests/Q00355_FamilyHonor"
    }

    fn id(&self) -> i32 {
        355
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GALIBREDO]);
        s.add_talk_id(&[GALIBREDO, PATRIN]);
        let ids: Vec<i32> = MOBS.iter().map(|m| m.0).collect();
        s.add_kill_id(&ids);
        s.register_quest_items(&[GALFREDO_ROMERS_BUST]);
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
            "30181-02.htm" | "30181-09.html" | "30929-01.html" | "30929-02.html" => {
                htmltext = Some(event.to_owned());
            }
            "30181-03.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30181-06.html" => {
                let busts = ctx.get_quest_items_count(player, GALFREDO_ROMERS_BUST);
                if busts < 1 {
                    htmltext = Some(event.to_owned());
                } else if busts >= 100 {
                    ctx.give_adena(player, (busts * 120) + 7800, true);
                    ctx.take_items(player, GALFREDO_ROMERS_BUST, -1);
                    htmltext = Some("30181-07.html".to_owned());
                } else {
                    ctx.give_adena(player, (busts * 120) + 2800, true);
                    ctx.take_items(player, GALFREDO_ROMERS_BUST, -1);
                    htmltext = Some("30181-08.html".to_owned());
                }
            }
            "30181-10.html" => {
                let busts = ctx.get_quest_items_count(player, GALFREDO_ROMERS_BUST);
                if busts > 0 {
                    ctx.give_adena(player, busts * 120, true);
                }
                ctx.take_items(player, GALFREDO_ROMERS_BUST, -1);
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "30929-03.html" => {
                let random = ctx.get_random(100);
                if ctx.has_quest_items(player, SCULPTOR_BERONA) {
                    if random < 2 {
                        ctx.give_items(player, ANCIENT_STATUE_PROTOTYPE, 1);
                        htmltext = Some(event.to_owned());
                    } else if random < 32 {
                        ctx.give_items(player, ANCIENT_STATUE_ORIGINAL, 1);
                        htmltext = Some("30929-04.html".to_owned());
                    } else if random < 62 {
                        ctx.give_items(player, ANCIENT_STATUE_REPLICA, 1);
                        htmltext = Some("30929-05.html".to_owned());
                    } else if random < 77 {
                        ctx.give_items(player, ANCIENT_STATUE_FORGERY, 1);
                        htmltext = Some("30929-06.html".to_owned());
                    } else {
                        htmltext = Some("30929-07.html".to_owned());
                    }
                    ctx.take_items(player, SCULPTOR_BERONA, 1);
                } else {
                    htmltext = Some("30929-08.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if qs.is_none() || !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            return;
        }
        let Some(&(_, first_chance, second_chance)) = MOBS.iter().find(|m| m.0 == npc.id) else {
            return;
        };
        let random = ctx.get_random(1000);
        if random < first_chance {
            ctx.give_item_randomly(killer, Some(npc), GALFREDO_ROMERS_BUST, 1, 1, 0, 1.0, true);
        } else if random < second_chance {
            ctx.give_item_randomly(killer, Some(npc), SCULPTOR_BERONA, 1, 1, 0, 1.0, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30181-01.htm"
            } else {
                "30181-04.html"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            if npc.id == GALIBREDO {
                htmltext = if ctx.has_quest_items(player, SCULPTOR_BERONA) {
                    "30181-11.html"
                } else {
                    "30181-05.html"
                }
                .to_owned();
            } else {
                htmltext = "30929-01.html".to_owned();
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
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00355_FamilyHonor";

    const NPCS: &str = r#"
        <npc id="30181" type="Folk" level="50" name="Galibredo"/>
        <npc id="30929" type="Folk" level="50" name="Patrin"/>
        <npc id="20767" type="Monster" level="40" name="Timak Orc Troop Leader"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4252" type="EtcItem" name="Galfredo Romer's Bust"><set name="is_stackable" val="true"/><set name="is_questitem" val="true"/></item>
        <item id="4350" type="EtcItem" name="Sculptor Berona"><set name="is_questitem" val="true"/></item>
        <item id="4351" type="EtcItem" name="Prototype"/>
        <item id="4352" type="EtcItem" name="Original"/>
        <item id="4353" type="EtcItem" name="Replica"/>
        <item id="4354" type="EtcItem" name="Forgery"/>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00355FamilyHonor);
        let pages = Pages::of(&Q00355FamilyHonor)
            .page("30181-01.htm", &["30181-02.htm"])
            .page("30181-02.htm", &["30181-03.htm"])
            .plain(&["30181-03.htm", "30181-04.html", "30181-07.html", "30181-08.html"])
            .page("30181-05.html", &["30181-06.html", "30181-09.html"])
            .page("30181-06.html", &["30181-09.html"])
            .page("30181-09.html", &["30181-10.html"])
            .plain(&["30181-10.html", "30181-11.html"])
            .page("30929-01.html", &["30929-03.html"])
            .plain(&[
                "30929-02.html",
                "30929-03.html",
                "30929-04.html",
                "30929-05.html",
                "30929-06.html",
                "30929-07.html",
                "30929-08.html",
            ])
            .npcs(&[GALIBREDO, PATRIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let galibredo = t.spawn(GALIBREDO, 20);
        let patrin = t.spawn(PATRIN, 40);
        (t, galibredo, patrin)
    }

    fn start(t: &mut ScriptTest, galibredo: Oid, level: i32) {
        t.enter(1, level, vec![]);
        t.quest_page(1, galibredo);
        t.event(1, NAME, "30181-02.htm");
        assert!(t.event(1, NAME, "30181-03.htm").contains("30181-03.htm"));
    }

    #[test]
    fn a_level_35_character_is_sent_away() {
        let (mut t, galibredo, _) = setup();
        t.enter(1, 35, vec![]);
        assert!(t.quest_page(1, galibredo).contains("30181-04.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn start_sell_busts_and_quit() {
        let (mut t, galibredo, _) = setup();
        t.enter(1, 36, vec![item(5, GALFREDO_ROMERS_BUST, 100)]);
        assert!(t.quest_page(1, galibredo).contains("30181-01.htm"));
        assert!(t.event(1, NAME, "30181-02.htm").contains("30181-02.htm"));
        assert!(t.event(1, NAME, "30181-03.htm").contains("30181-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // 100 busts: 100 * 120 + 7800 adena.
        assert!(t.quest_page(1, galibredo).contains("30181-05.html"));
        assert!(t.event(1, NAME, "30181-06.html").contains("30181-07.html"));
        assert_eq!(t.count(1, 57), 19_800);
        assert_eq!(t.count(1, GALFREDO_ROMERS_BUST), 0);

        // Nothing to sell: the plain page; quitting pays nothing more and ends the quest.
        assert!(t.quest_page(1, galibredo).contains("30181-05.html"));
        assert!(t.event(1, NAME, "30181-06.html").contains("30181-06.html"));
        assert!(t.event(1, NAME, "30181-09.html").contains("30181-09.html"));
        t.take_writes();
        assert!(t.event(1, NAME, "30181-10.html").contains("30181-10.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn fewer_than_100_busts_pay_2800_extra() {
        let (mut t, galibredo, _) = setup();
        t.enter(1, 36, vec![item(5, GALFREDO_ROMERS_BUST, 10)]);
        t.quest_page(1, galibredo);
        t.event(1, NAME, "30181-02.htm");
        t.event(1, NAME, "30181-03.htm");
        t.quest_page(1, galibredo);
        assert!(t.event(1, NAME, "30181-06.html").contains("30181-08.html"));
        assert_eq!(t.count(1, 57), 4_000);
    }

    #[test]
    fn patrin_turns_a_berona_into_a_statue_or_nothing() {
        let (mut t, galibredo, patrin) = setup();
        start(&mut t, galibredo, 36);
        t.take_writes();
        assert!(t.quest_page(1, patrin).contains("30929-01.html"));
        assert!(t.event(1, NAME, "30929-03.html").contains("30929-08.html"));

        // With a Berona: it is taken, and one statue (or nothing) comes back.
        let (mut t, galibredo, patrin) = setup();
        t.enter(1, 36, vec![item(5, SCULPTOR_BERONA, 1)]);
        t.quest_page(1, galibredo);
        t.event(1, NAME, "30181-02.htm");
        t.event(1, NAME, "30181-03.htm");
        t.quest_page(1, patrin);
        let page = t.event(1, NAME, "30929-03.html");
        assert!(page.contains("30929-0"), "{page}");
        assert!(!page.contains("30929-08.html"), "{page}");
        assert_eq!(t.count(1, SCULPTOR_BERONA), 0);
        let statues: i64 = (4351..=4354).map(|id| t.count(1, id)).sum();
        assert_eq!(statues, i64::from(!page.contains("30929-07.html")));
    }

    #[test]
    fn timak_orcs_drop_busts_and_beronas() {
        let (mut t, galibredo, _) = setup();
        start(&mut t, galibredo, 36);
        for _ in 0..40 {
            let mob = t.spawn(20767, 60);
            t.kill(1, mob);
        }
        // 56 % busts, 12.4 % beronas: 40 kills give some of both.
        assert!(t.count(1, GALFREDO_ROMERS_BUST) > 5);
        assert!(t.count(1, SCULPTOR_BERONA) >= 1);
    }
}
