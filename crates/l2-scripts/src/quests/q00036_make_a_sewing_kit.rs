//! Quest 36: Make a Sewing Kit (Ferris in the Talking Island's Gludin side, golems in the hunt).
// Java: dist/game/data/scripts/quests/Q00036_MakeASewingKit/Q00036_MakeASewingKit.java
//
// Pattern: a kill-count chain with a party-member check (`docs/design/scripts.md`, "Quest
// patterns"): the enchanted iron golems give iron to the killer at cond 1 with a 50% roll, the
// member at cond 1 moves on to cond 2 at the fifth, and Ferris takes the iron and then the
// frames and oriharukon.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const FERRIS: i32 = 30847;

// Monster
const ENCHANTED_IRON_GOLEM: i32 = 20566;

// Items
const ARTISANS_FRAME: i32 = 1891;
const ORIHARUKON: i32 = 1893;
const SEWING_KIT: i32 = 7078;
const ENCHANTED_IRON: i32 = 7163;

// Misc
const MIN_LEVEL: i32 = 60;
const IRON_COUNT: i64 = 5;
const COUNT: i64 = 10;

/// Java `Q00036_MakeASewingKit`.
pub struct Q00036MakeASewingKit;

impl Script for Q00036MakeASewingKit {
    fn name(&self) -> &'static str {
        "Q00036_MakeASewingKit"
    }

    fn path(&self) -> &'static str {
        "quests/Q00036_MakeASewingKit"
    }

    fn id(&self) -> i32 {
        36
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[FERRIS]);
        s.add_talk_id(&[FERRIS]);
        s.add_kill_id(&[ENCHANTED_IRON_GOLEM]);
        s.register_quest_items(&[ENCHANTED_IRON]);
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

        let mut htmltext = Some(event.to_owned());
        match event {
            "30847-03.htm" => {
                qs.start_quest(ctx);
            }
            "30847-06.html" => {
                if ctx.get_quest_items_count(player, ENCHANTED_IRON) < IRON_COUNT {
                    return Some(ctx.get_no_quest_msg(player));
                }
                ctx.take_items(player, ENCHANTED_IRON, -1);
                qs.set_cond(ctx, 3, true);
            }
            "30847-09.html" => {
                if ctx.get_quest_items_count(player, ARTISANS_FRAME) >= COUNT
                    && ctx.get_quest_items_count(player, ORIHARUKON) >= COUNT
                {
                    ctx.take_items(player, ARTISANS_FRAME, 10);
                    ctx.take_items(player, ORIHARUKON, 10);
                    ctx.give_items(player, SEWING_KIT, 1);
                    qs.exit_quest(ctx, false, true);
                } else {
                    htmltext = Some("30847-10.html".to_owned());
                }
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        if let Some(member) = ctx.get_random_party_member_cond(player, 1)
            && ctx.get_random_boolean()
        {
            ctx.give_items(player, ENCHANTED_IRON, 1);
            if ctx.get_quest_items_count(player, ENCHANTED_IRON) >= IRON_COUNT {
                if let Some(qs) = ctx.get_quest_state(member, false) {
                    qs.set_cond(ctx, 2, true);
                }
            } else {
                ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30847-01.htm"
                } else {
                    "30847-02.html"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "30847-04.html".to_owned(),
                2 => htmltext = "30847-05.html".to_owned(),
                3 => {
                    htmltext = if ctx.get_quest_items_count(player, ARTISANS_FRAME) >= COUNT
                        && ctx.get_quest_items_count(player, ORIHARUKON) >= COUNT
                    {
                        "30847-07.html"
                    } else {
                        "30847-08.html"
                    }
                    .to_owned();
                }
                _ => {}
            },
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
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
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00036_MakeASewingKit";

    const NPCS: &str = r#"
        <npc id="30847" type="Folk" level="70" name="Ferris"/>
        <npc id="20566" type="Monster" level="40" name="Enchanted Iron Golem"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1891" type="EtcItem" name="Artisan's Frame"><set name="is_stackable" val="true"/></item>
        <item id="1893" type="EtcItem" name="Oriharukon"><set name="is_stackable" val="true"/></item>
        <item id="7078" type="EtcItem" name="Sewing Kit"/>
        <item id="7163" type="EtcItem" name="Enchanted Iron"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00036MakeASewingKit);
        let pages = Pages::of(&Q00036MakeASewingKit)
            .page("30847-01.htm", &["30847-03.htm"])
            .page("30847-05.html", &["30847-06.html"])
            .page("30847-07.html", &["30847-09.html"])
            .plain(&[
                "30847-02.html",
                "30847-03.htm",
                "30847-04.html",
                "30847-06.html",
                "30847-08.html",
                "30847-09.html",
                "30847-10.html",
            ])
            .npcs(&[FERRIS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let ferris = t.spawn(FERRIS, 20);
        (t, ferris)
    }

    /// Kills golems until the player has 5 enchanted iron; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..400 {
            let golem = t.spawn(ENCHANTED_IRON_GOLEM, 80);
            t.kill(1, golem);
            if t.count(1, ENCHANTED_IRON) >= IRON_COUNT {
                return kills;
            }
        }
        panic!("no 5 enchanted iron after 400 kills");
    }

    #[test]
    fn a_level_59_character_is_sent_away() {
        let (mut t, ferris) = setup();
        t.enter(1, 59, vec![]);
        assert!(t.quest_page(1, ferris).contains("30847-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn five_iron_then_the_frames_and_the_oriharukon_make_the_kit() {
        let (mut t, ferris) = setup();
        t.enter(
            1,
            60,
            vec![item(1001, ARTISANS_FRAME, 10), item(1002, ORIHARUKON, 10)],
        );
        assert!(t.quest_page(1, ferris).contains("30847-01.htm"));
        assert!(t.event(1, NAME, "30847-03.htm").contains("30847-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, ferris).contains("30847-04.html"));

        // The golems give iron to the killer; the fifth sets cond 2.
        hunt(&mut t);
        assert_eq!(t.count(1, ENCHANTED_IRON), 5);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Ferris takes the iron: cond 3, the iron goes.
        assert!(t.quest_page(1, ferris).contains("30847-05.html"));
        assert!(t.event(1, NAME, "30847-06.html").contains("30847-06.html"));
        assert_eq!(t.count(1, ENCHANTED_IRON), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // The frames and the oriharukon: the kit, the end.
        assert!(t.quest_page(1, ferris).contains("30847-07.html"));
        assert!(t.event(1, NAME, "30847-09.html").contains("30847-09.html"));
        assert_eq!(t.count(1, SEWING_KIT), 1);
        assert_eq!((t.count(1, ARTISANS_FRAME), t.count(1, ORIHARUKON)), (0, 0));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, ferris)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn too_few_frames_are_refused() {
        let (mut t, ferris) = setup();
        t.enter(
            1,
            60,
            vec![item(1001, ARTISANS_FRAME, 9), item(1002, ORIHARUKON, 10)],
        );
        t.quest_page(1, ferris);
        t.event(1, NAME, "30847-03.htm");
        t.take_writes();

        // The real `30847-09.html` is linked at cond 3 only, so the event runs directly.
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00036MakeASewingKit.on_event(ctx, "30847-09.html", None, Some(Oid(1)));
        });
        assert_eq!(out.as_deref(), Some("30847-10.html"));
        assert_eq!(t.count(1, SEWING_KIT), 0);
        assert_eq!(t.count(1, ARTISANS_FRAME), 9);
        assert!(t.take_writes().is_empty());
    }
}
