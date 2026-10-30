//! Quest 240: I'm the Only One You Can Trust (Kintaijin, a bounty hunter in the Dion area,
//! sends the characters of level 81 and up to the Stakato's Nest for 25 Stakato Fangs).
// Java: dist/game/data/scripts/quests/Q00240_ImTheOnlyOneYouCanTrust/Q00240_ImTheOnlyOneYouCanTrust.java
//
// Pattern: a party kill, a random member at cond 1 gets the drop (`docs/design/scripts.md`,
// "Quest patterns"), then one-time with adena, exp and the items taken back.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KINTAIJIN: i32 = 32640;

// Monsters: the Spiked and Cannibalistic Stakatos of the Stakato's Nest
const MOBS: [i32; 17] = [
    22617, 22618, 22619, 22620, 22621, 22622, 22623, 22624, 22625, 22626, 22627, 22628, 22629, 22630, 22631,
    22632, 22633,
];

// Item
const STAKATO_FANG: i32 = 14879;

/// Java `Q00240_ImTheOnlyOneYouCanTrust`.
pub struct Q00240ImTheOnlyOneYouCanTrust;

impl Script for Q00240ImTheOnlyOneYouCanTrust {
    fn name(&self) -> &'static str {
        "Q00240_ImTheOnlyOneYouCanTrust"
    }

    fn path(&self) -> &'static str {
        "quests/Q00240_ImTheOnlyOneYouCanTrust"
    }

    fn id(&self) -> i32 {
        240
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KINTAIJIN]);
        s.add_talk_id(&[KINTAIJIN]);
        s.add_kill_id(&MOBS);
        s.register_quest_items(&[STAKATO_FANG]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };
        if event.eq_ignore_ascii_case("32640-3.htm") {
            qs.start_quest(ctx);
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // The fang goes to a random party member who is at cond 1 (the killer if alone).
        let Some(member) = ctx.get_random_party_member_cond(killer, 1) else {
            return;
        };
        let Some(qs) = ctx.get_quest_state(member, false) else {
            return;
        };
        ctx.give_items(member, STAKATO_FANG, 1);
        if ctx.get_quest_items_count(member, STAKATO_FANG) >= 25 {
            qs.set_cond(ctx, 2, true);
        } else {
            ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= 81 {
                    "32640-1.htm"
                } else {
                    "32640-0.htm"
                }
                .to_owned();
            }
            STARTED => {
                if qs.get_cond(ctx) == 1 {
                    htmltext = if ctx.has_quest_items(player, STAKATO_FANG) {
                        "32640-9.html"
                    } else {
                        "32640-8.html"
                    }
                    .to_owned();
                } else if qs.get_cond(ctx) == 2 && ctx.get_quest_items_count(player, STAKATO_FANG) >= 25 {
                    ctx.give_adena(player, 147200, true);
                    ctx.take_items(player, STAKATO_FANG, -1);
                    ctx.add_exp_and_sp(player, 589542, 36800);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "32640-10.html".to_owned();
                }
            }
            COMPLETED => htmltext = "32640-11.html".to_owned(),
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00240_ImTheOnlyOneYouCanTrust";

    const NPCS: &str = r#"
        <npc id="32640" type="Folk" level="70" name="Kintaijin"/>
        <npc id="22617" type="Monster" level="78" name="Spiked Stakato"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14879" type="EtcItem" name="Stakato Fang"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00240ImTheOnlyOneYouCanTrust);
        let pages = Pages::of(&Q00240ImTheOnlyOneYouCanTrust)
            .page("32640-1.htm", &["32640-4.htm", "32640-5.html", "32640-6.html"])
            .page("32640-2.htm", &["32640-3.htm"])
            .page("32640-6.html", &["32640-2.htm"])
            .plain(&[
                "32640-0.htm",
                "32640-3.htm",
                "32640-8.html",
                "32640-9.html",
                "32640-10.html",
                "32640-11.html",
            ])
            .page("32640-4.htm", &["32640-3.htm"])
            .page("32640-5.html", &["32640-4.htm", "32640-7.html"])
            .page("32640-7.html", &["32640-6.html", "32640-2.htm"])
            .npcs(&[KINTAIJIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kintaijin = t.spawn(KINTAIJIN, 20);
        (t, kintaijin)
    }

    /// Kills one Stakato of the nest, as `killer`.
    fn kill_one(t: &mut ScriptTest, killer: i32) {
        let mob = t.spawn(22617, 20);
        t.kill(killer, mob);
    }

    #[test]
    fn a_character_below_level_81_is_sent_away() {
        let (mut t, kintaijin) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, kintaijin).contains("32640-0.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_mission_is_accepted_through_the_story() {
        let (mut t, kintaijin) = setup();
        t.enter(1, 81, vec![]);
        assert!(t.quest_page(1, kintaijin).contains("32640-1.htm"));
        assert!(t.event(1, NAME, "32640-6.html").contains("32640-6.html"));
        assert!(t.event(1, NAME, "32640-2.htm").contains("32640-2.htm"));
        assert!(t.event(1, NAME, "32640-3.htm").contains("32640-3.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, kintaijin).contains("32640-8.html"));
    }

    #[test]
    fn twenty_five_fangs_bring_the_reward() {
        let (mut t, kintaijin) = setup();
        t.enter(1, 81, vec![]);
        t.quest_page(1, kintaijin);
        t.event(1, NAME, "32640-6.html");
        t.event(1, NAME, "32640-2.htm");
        t.event(1, NAME, "32640-3.htm");
        t.take_writes();

        for _ in 0..24 {
            kill_one(&mut t, 1);
        }
        assert_eq!(t.count(1, STAKATO_FANG), 24);
        assert!(t.take_writes().is_empty());
        assert!(t.quest_page(1, kintaijin).contains("32640-9.html"));

        // The 25th fang sets cond 2 and Kintaijin pays.
        kill_one(&mut t, 1);
        assert_eq!(t.count(1, STAKATO_FANG), 25);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, kintaijin).contains("32640-10.html"));
        assert_eq!(t.count(1, 57), 147200);
        assert_eq!(t.count(1, STAKATO_FANG), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, kintaijin).contains("32640-11.html"));
    }

    #[test]
    fn the_fang_goes_to_the_party_member_at_cond_1() {
        let (mut t, kintaijin) = setup();
        // Player 1 has no quest; player 2 takes it and is in the party of player 1.
        t.enter(1, 81, vec![]);
        t.enter(2, 81, vec![]);
        t.party(1, 2);
        t.quest_page(2, kintaijin);
        t.event(2, NAME, "32640-6.html");
        t.event(2, NAME, "32640-2.htm");
        t.event(2, NAME, "32640-3.htm");
        t.take_writes();

        kill_one(&mut t, 1);
        assert_eq!((t.count(2, STAKATO_FANG), t.count(1, STAKATO_FANG)), (1, 0));
    }
}
