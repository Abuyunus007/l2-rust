//! Quest 10288: Secret Mission (Dominic in the Gludio area, Aquilani, Greymore).
// Java: dist/game/data/scripts/quests/Q10288_SecretMission/Q10288_SecretMission.java
//
// Pattern: a delivery chain (`docs/design/scripts.md`, "Quest patterns"): Dominic starts the
// quest, the letter goes to Aquilani (cond 2) and then to Greymore, who pays. Aquilani's first
// talk changes after the quest and teleports a finished character.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const DOMINIC: i32 = 31350;
const AQUILANI: i32 = 32780;
const GREYMORE: i32 = 32757;

// Item
const LETTER: i32 = 15529;

// Location (Java `TELEPORT`)
const TELEPORT: (i32, i32, i32) = (118_833, -80_589, -2_688);

/// Java `Q10288_SecretMission`.
pub struct Q10288SecretMission;

impl Script for Q10288SecretMission {
    fn name(&self) -> &'static str {
        "Q10288_SecretMission"
    }

    fn path(&self) -> &'static str {
        "quests/Q10288_SecretMission"
    }

    fn id(&self) -> i32 {
        10288
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[AQUILANI, DOMINIC]);
        s.add_first_talk_id(&[AQUILANI]);
        s.add_talk_id(&[DOMINIC, GREYMORE, AQUILANI]);
        s.register_quest_items(&[LETTER]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        let mut htmltext = event.to_owned();
        match event {
            "31350-03.html" => {
                if ctx.player_level(player) < 82 {
                    htmltext = "31350-02b.html".to_owned();
                }
            }
            "31350-05.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, LETTER, 1);
            }
            "32780-03.html" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, LETTER) {
                    qs.set_cond(ctx, 2, true);
                }
            }
            "32757-03.html" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, LETTER) {
                    ctx.give_adena(player, 106_583, true);
                    ctx.add_exp_and_sp(player, 417_788, 46_320);
                    qs.exit_quest(ctx, false, true);
                }
            }
            "teleport" => {
                if npc.is_some_and(|n| n.id == AQUILANI) && qs.is_completed(ctx) {
                    ctx.teleport_player(player, TELEPORT.0, TELEPORT.1, TELEPORT.2, 0, false);
                    return None;
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, false);
        // dialog only changes when you talk to Aquilani after quest completion
        if let Some(qs) = qs
            && qs.is_completed(ctx)
        {
            return Some("32780-05.html".to_owned());
        }
        Some("32780.html".to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            DOMINIC => match qs.state(ctx) {
                CREATED => htmltext = "31350-01.htm".to_owned(),
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "31350-06.html".to_owned();
                    }
                }
                COMPLETED => htmltext = "31350-07.html".to_owned(),
                _ => {}
            },
            AQUILANI => {
                if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, LETTER) {
                        htmltext = "32780-01.html".to_owned();
                    } else if qs.is_cond(ctx, 2) {
                        htmltext = "32780-04.html".to_owned();
                    }
                }
            }
            GREYMORE => {
                if qs.is_started(ctx) && qs.is_cond(ctx, 2) && ctx.has_quest_items(player, LETTER) {
                    return Some("32757-01.html".to_owned());
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
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, pages_to, rows};

    const NAME: &str = "Q10288_SecretMission";

    const NPCS: &str = r#"
        <npc id="31350" type="Folk" level="80" name="Dominic"/>
        <npc id="32780" type="Folk" level="80" name="Aquilani"/>
        <npc id="32757" type="Folk" level="80" name="Greymore"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15529" type="EtcItem" name="Secret Letter"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10288SecretMission);
        let pages = Pages::of(&Q10288SecretMission)
            .page("31350-01.htm", &["31350-03.html", "31350-02.html"])
            .page("31350-03.html", &["31350-04.htm"])
            .page("31350-04.htm", &["31350-05.htm"])
            .plain(&[
                "31350-02.html",
                "31350-02b.html",
                "31350-05.htm",
                "31350-06.html",
                "31350-07.html",
            ])
            // The first-talk page of Aquilani has the quest list link (`bypass Script`).
            .file(
                "data/scripts/quests/Q10288_SecretMission/32780.html",
                r#"<html><body>32780.html<a action="bypass Script">Quest</a></body></html>"#,
            )
            .page("32780-01.html", &["32780-02.html"])
            .page("32780-02.html", &["32780-03.html"])
            .plain(&["32780-03.html", "32780-04.html"])
            .page("32780-05.html", &["32780-06.html"])
            .page("32780-06.html", &["teleport"])
            .page("32757-01.html", &["32757-02.html"])
            .page("32757-02.html", &["32757-03.html"])
            .plain(&["32757-03.html"])
            .npcs(&[DOMINIC, AQUILANI, GREYMORE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let dominic = t.spawn(DOMINIC, 20);
        let aquilani = t.spawn(AQUILANI, 30);
        let greymore = t.spawn(GREYMORE, 40);
        (t, dominic, aquilani, greymore)
    }

    #[test]
    fn a_character_under_82_is_sent_back() {
        let (mut t, dominic, _, _) = setup();
        t.enter(1, 81, vec![]);
        assert!(t.quest_page(1, dominic).contains("31350-01.htm"));
        assert!(t.event(1, NAME, "31350-03.html").contains("31350-02b.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn dominic_gives_the_letter_and_aquilani_takes_it_on() {
        let (mut t, dominic, aquilani, _) = setup();
        t.enter(1, 82, vec![]);
        assert!(t.quest_page(1, dominic).contains("31350-01.htm"));
        assert!(t.event(1, NAME, "31350-03.html").contains("31350-03.html"));
        assert!(t.event(1, NAME, "31350-04.htm").contains("31350-04.htm"));
        assert!(t.event(1, NAME, "31350-05.htm").contains("31350-05.htm"));
        assert_eq!(t.count(1, LETTER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Aquilani's first talk is the plain page before the quest ends.
        t.talk(1, aquilani);
        let out = t.take();
        assert!(pages_to(&out, 1).iter().any(|p| p.contains("32780.html")));
        assert!(t.quest_page(1, aquilani).contains("32780-01.html"));
        assert!(t.event(1, NAME, "32780-02.html").contains("32780-02.html"));
        assert!(t.event(1, NAME, "32780-03.html").contains("32780-03.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
    }

    #[test]
    fn greymore_pays_the_letter_and_the_quest_ends() {
        let (mut t, dominic, aquilani, greymore) = setup();
        t.enter(1, 82, vec![]);
        assert!(t.quest_page(1, dominic).contains("31350-01.htm"));
        t.event(1, NAME, "31350-03.html");
        t.event(1, NAME, "31350-04.htm");
        t.event(1, NAME, "31350-05.htm");
        assert!(t.quest_page(1, aquilani).contains("32780-01.html"));
        t.event(1, NAME, "32780-02.html");
        t.event(1, NAME, "32780-03.html");
        t.take_writes();

        assert!(t.quest_page(1, greymore).contains("32757-01.html"));
        assert!(t.event(1, NAME, "32757-02.html").contains("32757-02.html"));
        t.take();
        assert!(t.event(1, NAME, "32757-03.html").contains("32757-03.html"));
        assert_eq!(t.count(1, 57), 106_583);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, dominic).contains("31350-07.html"));
    }

    #[test]
    fn a_finished_character_gets_the_other_aquilani_page_and_the_teleport() {
        let (mut t, dominic, aquilani, greymore) = setup();
        t.enter(1, 82, vec![]);
        t.quest_page(1, dominic);
        t.event(1, NAME, "31350-03.html");
        t.event(1, NAME, "31350-04.htm");
        t.event(1, NAME, "31350-05.htm");
        t.quest_page(1, aquilani);
        t.event(1, NAME, "32780-02.html");
        t.event(1, NAME, "32780-03.html");
        t.quest_page(1, greymore);
        t.event(1, NAME, "32757-02.html");
        t.event(1, NAME, "32757-03.html");
        t.take();

        t.talk(1, aquilani);
        let out = t.take();
        assert!(pages_to(&out, 1).iter().any(|p| p.contains("32780-05.html")));
        assert!(t.event(1, NAME, "32780-06.html").contains("32780-06.html"));
        assert!(t.event(1, NAME, "teleport").is_empty());
        // Z is corrected to the ground by the world (geodata), like Java's teleport; X and Y are the target.
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (118_833, -80_589));
    }
}
