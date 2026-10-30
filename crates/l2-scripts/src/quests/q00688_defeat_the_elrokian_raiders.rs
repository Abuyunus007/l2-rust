//! Quest 688: Defeat the Elrokian Raiders! (Dinn on the Talking Island wharf, the Elrokian raiders of the Primeval Plains).
// Java: dist/game/data/scripts/quests/Q00688_DefeatTheElrokianRaiders/Q00688_DefeatTheElrokianRaiders.java
//
// Pattern: a repeatable kill-and-collect quest. The necklaces drop from the raiders to the party
// member the random pick returns (`get_random_party_member_cond(player, 1)`), and Dinn pays per
// necklace or, with 100 of them, a donation (a coin toss between two sums).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ELROKI: i32 = 22214;
const DINN: i32 = 32105;

// Item
const DINOSAUR_FANG_NECKLACE: i32 = 8785;

// Misc
const MIN_LEVEL: i32 = 75;
const DROP_RATE: i32 = 448;

/// Java `Q00688_DefeatTheElrokianRaiders`.
pub struct Q00688DefeatTheElrokianRaiders;

impl Script for Q00688DefeatTheElrokianRaiders {
    fn name(&self) -> &'static str {
        "Q00688_DefeatTheElrokianRaiders"
    }

    fn path(&self) -> &'static str {
        "quests/Q00688_DefeatTheElrokianRaiders"
    }

    fn id(&self) -> i32 {
        688
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DINN]);
        s.add_talk_id(&[DINN]);
        s.add_kill_id(&[ELROKI]);
        s.register_quest_items(&[DINOSAUR_FANG_NECKLACE]);
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
            "32105-02.htm" | "32105-10.html" => htmltext = Some(event.to_owned()),
            "32105-03.html" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32105-06.html" => {
                if ctx.has_quest_items(player, DINOSAUR_FANG_NECKLACE) {
                    let count = ctx.get_quest_items_count(player, DINOSAUR_FANG_NECKLACE);
                    ctx.give_adena(player, 3000 * count, true);
                    ctx.take_items(player, DINOSAUR_FANG_NECKLACE, -1);
                    htmltext = Some(event.to_owned());
                }
            }
            "donation" => {
                if ctx.get_quest_items_count(player, DINOSAUR_FANG_NECKLACE) < 100 {
                    htmltext = Some("32105-07.html".to_owned());
                } else {
                    if ctx.get_random(1000) < 500 {
                        ctx.give_adena(player, 450_000, true);
                        htmltext = Some("32105-08.html".to_owned());
                    } else {
                        ctx.give_adena(player, 150_000, true);
                        htmltext = Some("32105-09.html".to_owned());
                    }
                    ctx.take_items(player, DINOSAUR_FANG_NECKLACE, 100);
                }
            }
            "32105-11.html" => {
                if ctx.has_quest_items(player, DINOSAUR_FANG_NECKLACE) {
                    // Java pays here without taking them: `exitQuest` removes the registered
                    // quest items (the necklaces) right after.
                    let count = ctx.get_quest_items_count(player, DINOSAUR_FANG_NECKLACE);
                    ctx.give_adena(player, 3000 * count, true);
                }
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.get_random_party_member_cond(killer, 1) else {
            return;
        };
        if ctx.get_random(1000) < DROP_RATE {
            ctx.reward_items(member, DINOSAUR_FANG_NECKLACE, 1);
            ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "32105-01.htm"
                } else {
                    "32105-04.html"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if ctx.has_quest_items(player, DINOSAUR_FANG_NECKLACE) {
                    "32105-05.html"
                } else {
                    "32105-12.html"
                }
                .to_owned();
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00688_DefeatTheElrokianRaiders";

    const NPCS: &str = r#"
        <npc id="32105" type="Folk" level="1" name="Dinn"/>
        <npc id="22214" type="Monster" level="78" name="Elroki"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8785" type="EtcItem" name="Dinosaur Fang Necklace"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00688DefeatTheElrokianRaiders);
        let pages = Pages::of(&Q00688DefeatTheElrokianRaiders)
            .page("32105-01.htm", &["32105-02.htm"])
            .page("32105-02.htm", &["32105-03.html"])
            .plain(&["32105-03.html", "32105-04.html"])
            .page("32105-05.html", &["32105-06.html", "donation"])
            .page("32105-06.html", &["32105-10.html", "32105-11.html"])
            .plain(&["32105-07.html"])
            .page("32105-08.html", &["32105-10.html", "32105-11.html"])
            .page("32105-09.html", &["32105-10.html", "32105-11.html"])
            .plain(&["32105-10.html", "32105-11.html"])
            .page("32105-12.html", &["32105-10.html", "32105-11.html"])
            .npcs(&[DINN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let dinn = t.spawn(DINN, 20);
        (t, dinn)
    }

    fn give_necklaces(t: &mut ScriptTest, count: i64) {
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), DINOSAUR_FANG_NECKLACE, count);
        });
    }

    #[test]
    fn a_level_74_character_is_refused() {
        let (mut t, dinn) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, dinn).contains("32105-04.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_level_75_character_starts_the_quest() {
        let (mut t, dinn) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, dinn).contains("32105-01.htm"));
        assert!(t.event(1, NAME, "32105-02.htm").contains("32105-02.htm"));
        assert!(t.event(1, NAME, "32105-03.html").contains("32105-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, dinn).contains("32105-12.html"));
    }

    #[test]
    fn the_raiders_drop_necklaces_and_dinn_pays_per_necklace() {
        let (mut t, dinn) = setup();
        t.enter(1, 75, vec![]);
        t.quest_page(1, dinn);
        t.event(1, NAME, "32105-02.htm");
        t.event(1, NAME, "32105-03.html");
        t.take_writes();

        let mut kills = 0;
        while t.count(1, DINOSAUR_FANG_NECKLACE) < 3 {
            let elroki = t.spawn(ELROKI, 60);
            t.kill(1, elroki);
            kills += 1;
            assert!(kills < 200, "the raiders never dropped a necklace");
        }
        assert!(t.quest_page(1, dinn).contains("32105-05.html"));

        let necklaces = t.count(1, DINOSAUR_FANG_NECKLACE);
        assert!(t.event(1, NAME, "32105-06.html").contains("32105-06.html"));
        assert_eq!(t.count(1, DINOSAUR_FANG_NECKLACE), 0);
        assert_eq!(t.count(1, 57), 3000 * necklaces);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_hundred_necklaces_get_the_merchants_donation() {
        let (mut t, dinn) = setup();
        t.enter(1, 75, vec![]);
        t.quest_page(1, dinn);
        t.event(1, NAME, "32105-02.htm");
        t.event(1, NAME, "32105-03.html");
        t.take_writes();

        give_necklaces(&mut t, 99);
        assert!(t.quest_page(1, dinn).contains("32105-05.html"));
        assert!(t.event(1, NAME, "donation").contains("32105-07.html"));
        assert_eq!(t.count(1, 57), 0);

        give_necklaces(&mut t, 1);
        assert!(t.quest_page(1, dinn).contains("32105-05.html"));
        assert!(t.event(1, NAME, "donation").contains("32105-0"));
        let donation = t.count(1, 57);
        assert!(donation == 450_000 || donation == 150_000, "{donation}");
        assert_eq!(t.count(1, DINOSAUR_FANG_NECKLACE), 0);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn quitting_pays_the_necklaces_and_ends_the_quest() {
        let (mut t, dinn) = setup();
        t.enter(1, 75, vec![]);
        t.quest_page(1, dinn);
        t.event(1, NAME, "32105-02.htm");
        t.event(1, NAME, "32105-03.html");
        t.take_writes();

        // 101 necklaces: the donation takes 100 and leaves one; the quit page (08/09) pays it.
        give_necklaces(&mut t, 101);
        assert!(t.quest_page(1, dinn).contains("32105-05.html"));
        assert!(t.event(1, NAME, "donation").contains("32105-0"));
        let donation = t.count(1, 57);
        assert_eq!(t.count(1, DINOSAUR_FANG_NECKLACE), 1);
        t.take_writes();

        assert!(t.event(1, NAME, "32105-11.html").contains("32105-11.html"));
        assert_eq!(t.count(1, 57), donation + 3000);
        // The exit removes the registered quest item (the necklace).
        assert_eq!(t.count(1, DINOSAUR_FANG_NECKLACE), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
