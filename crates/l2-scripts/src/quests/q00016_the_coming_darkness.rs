//! Quest 16: The Coming Darkness (the Hierarch and five evil altars, for a level 62 character).
// Java: dist/game/data/scripts/quests/Q00016_TheComingDarkness/Q00016_TheComingDarkness.java
//
// Pattern: an ordered visit to five NPCs with a crystal per altar (`docs/design/scripts.md`,
// "Quest patterns"): the Hierarch gives five crystals, each altar takes one when it is the next
// altar in order (cond = altar index), and the Hierarch pays the exp at cond 6. One-time:
// `exit_quest(ctx, false, true)`.
//
// NOTE(java-quirk): the Java TODO says the altars' AI is incomplete (the original quest is not
// retail-like yet). Only the quest's own steps are ported here.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HIERARCH: i32 = 31517;
const EVIL_ALTAR_1: i32 = 31512;
const EVIL_ALTAR_2: i32 = 31513;
const EVIL_ALTAR_3: i32 = 31514;
const EVIL_ALTAR_4: i32 = 31515;
const EVIL_ALTAR_5: i32 = 31516;

// Item
const CRYSTAL_OF_SEAL: i32 = 7167;

// Misc
const MIN_LEVEL: i32 = 62;
/// Java `Q00017_LightAndDarkness.class.getSimpleName()`: the next quest, whose unfinished state
/// keeps the Hierarch from talking about this one.
const LIGHT_AND_DARKNESS: &str = "Q00017_LightAndDarkness";

/// Java `Q00016_TheComingDarkness`.
pub struct Q00016TheComingDarkness;

impl Script for Q00016TheComingDarkness {
    fn name(&self) -> &'static str {
        "Q00016_TheComingDarkness"
    }

    fn path(&self) -> &'static str {
        "quests/Q00016_TheComingDarkness"
    }

    fn id(&self) -> i32 {
        16
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HIERARCH]);
        s.add_talk_id(&[
            HIERARCH,
            EVIL_ALTAR_1,
            EVIL_ALTAR_2,
            EVIL_ALTAR_3,
            EVIL_ALTAR_4,
            EVIL_ALTAR_5,
        ]);
        s.register_quest_items(&[CRYSTAL_OF_SEAL]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // NOTE(java-quirk): without a quest state `onEvent` returns the event itself (not null),
        // so the event's page is shown anyway.
        let Some(player) = player else {
            return Some(event.to_owned());
        };
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(event.to_owned());
        };

        let cond = qs.get_cond(ctx);
        match event {
            "31517-02.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, CRYSTAL_OF_SEAL, 5);
            }
            "31512-01.html" | "31513-01.html" | "31514-01.html" | "31515-01.html" | "31516-01.html" => {
                let npc_id = event.replace("-01.html", "").parse::<i32>().unwrap_or_default();
                if cond == npc_id - 31511 && ctx.has_quest_items(player, CRYSTAL_OF_SEAL) {
                    ctx.take_items(player, CRYSTAL_OF_SEAL, 1);
                    qs.set_cond(ctx, cond + 1, true);
                }
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        // Java: `player.getQuestState(Q00017...)`: a Light and Darkness state that is not
        // completed keeps the Hierarch's answer on the "come back later" page.
        if let Some(qs2) = ctx.player_quest_state(player, LIGHT_AND_DARKNESS)
            && !qs2.is_completed()
        {
            return Some("31517-04.html".to_owned());
        }

        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);

        match qs.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "31517-00.htm"
                } else {
                    "31517-05.html"
                }
                .to_owned();
            }
            STARTED => {
                let npc_id = npc.id;
                if npc_id == HIERARCH {
                    if qs.is_cond(ctx, 6) {
                        ctx.add_exp_and_sp(player, 865_187, 69_172);
                        qs.exit_quest(ctx, false, true);
                        htmltext = "31517-03.html".to_owned();
                    } else {
                        htmltext = "31517-02a.html".to_owned();
                    }
                } else if npc_id - 31511 == qs.get_cond(ctx) {
                    htmltext = format!("{npc_id}-00.html");
                } else {
                    htmltext = format!("{npc_id}-01.html");
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00016_TheComingDarkness";

    const NPCS: &str = r#"
        <npc id="31517" type="Folk" level="70" name="Hierarch"/>
        <npc id="31512" type="Folk" level="70" name="Evil Altar 1"/>
        <npc id="31513" type="Folk" level="70" name="Evil Altar 2"/>
        <npc id="31514" type="Folk" level="70" name="Evil Altar 3"/>
        <npc id="31515" type="Folk" level="70" name="Evil Altar 4"/>
        <npc id="31516" type="Folk" level="70" name="Evil Altar 5"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7167" type="EtcItem" name="Crystal of Seal"><set name="is_questitem" val="true"/></item>"#;

    const ALTARS: [i32; 5] = [31512, 31513, 31514, 31515, 31516];

    struct Npcs {
        hierarch: Oid,
        altars: [Oid; 5],
    }

    /// Java `Q00017_LightAndDarkness` as far as this quest needs it: a script of that name with
    /// no NPCs, whose state the Hierarch looks at.
    struct LightAndDarkness;

    impl Script for LightAndDarkness {
        fn name(&self) -> &'static str {
            "Q00017_LightAndDarkness"
        }
        fn path(&self) -> &'static str {
            "quests/Q00017_LightAndDarkness"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Npcs) {
        setup_with(false)
    }

    /// `light_and_darkness`: the player is in Light and Darkness (started) as well.
    fn setup_with(light_and_darkness: bool) -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00016TheComingDarkness);
        if light_and_darkness {
            registry.add(LightAndDarkness);
        }
        let mut pages = Pages::of(&Q00016TheComingDarkness)
            .page("31517-00.htm", &["31517-01.htm"])
            .page("31517-01.htm", &["31517-02.htm"])
            .plain(&[
                "31517-02.htm",
                "31517-02a.html",
                "31517-03.html",
                "31517-04.html",
                "31517-05.html",
            ]);
        for altar in ALTARS {
            pages = pages
                .page(&format!("{altar}-00.html"), &[&format!("{altar}-01.html")])
                .plain(&[&format!("{altar}-01.html")]);
        }
        let pages = pages.npcs(&[HIERARCH, 31512, 31513, 31514, 31515, 31516]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            hierarch: t.spawn(HIERARCH, 20),
            altars: [
                t.spawn(ALTARS[0], 40),
                t.spawn(ALTARS[1], 60),
                t.spawn(ALTARS[2], 80),
                t.spawn(ALTARS[3], 100),
                t.spawn(ALTARS[4], 120),
            ],
        };
        (t, npcs)
    }

    #[test]
    fn the_hierarch_waits_until_light_and_darkness_is_over() {
        let (mut t, npcs) = setup_with(true);
        t.enter(1, 62, vec![]);
        // Light and Darkness is started for the player (its own state, by its own start path).
        t.world.run_as_script("Q00017_LightAndDarkness", |ctx| {
            if let Some(qs) = ctx.get_quest_state(ObjectId(1), true) {
                qs.start_quest(ctx);
            }
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.hierarch).contains("31517-04.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_level_gate_for_the_start() {
        let (mut t, npcs) = setup();
        t.enter(1, 61, vec![]);
        assert!(t.quest_page(1, npcs.hierarch).contains("31517-05.html"));
        assert!(t.take_writes().is_empty());
        t.enter(2, 62, vec![]);
        assert!(t.quest_page(2, npcs.hierarch).contains("31517-00.htm"));
    }

    #[test]
    fn the_five_altars_in_order_then_the_exp() {
        let (mut t, npcs) = setup();
        t.enter(1, 62, vec![]);
        assert!(t.quest_page(1, npcs.hierarch).contains("31517-00.htm"));
        assert!(t.event(1, NAME, "31517-01.htm").contains("31517-01.htm"));
        assert!(t.event(1, NAME, "31517-02.htm").contains("31517-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(t.count(1, CRYSTAL_OF_SEAL), 5);
        assert!(t.quest_page(1, npcs.hierarch).contains("31517-02a.html"));

        // Each altar takes one crystal, in order: the altar of the current cond gives its page.
        for (i, altar) in npcs.altars.into_iter().enumerate() {
            let id = ALTARS[i];
            let cond = i as i32 + 1;
            assert!(t.quest_page(1, altar).contains(&format!("{id}-00.html")));
            assert!(
                t.event(1, NAME, &format!("{id}-01.html"))
                    .contains(&format!("{id}-01.html"))
            );
            assert_eq!(t.count(1, CRYSTAL_OF_SEAL), 4 - i as i64);
            assert_eq!(
                t.take_writes(),
                [rows::update(NAME, "cond", &(cond + 1).to_string())]
            );
        }

        // The Hierarch at cond 6: the exp, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.hierarch).contains("31517-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.hierarch)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn an_altar_out_of_turn_only_shows_its_page() {
        let (mut t, npcs) = setup();
        t.enter(1, 62, vec![]);
        t.quest_page(1, npcs.hierarch);
        t.event(1, NAME, "31517-01.htm");
        t.event(1, NAME, "31517-02.htm");
        t.take_writes();
        assert!(t.quest_page(1, npcs.altars[1]).contains("31513-01.html"));
        assert_eq!(t.count(1, CRYSTAL_OF_SEAL), 5);
        assert!(t.take_writes().is_empty());
    }
}
