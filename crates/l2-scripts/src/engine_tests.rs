//! Tests of the S-API-05 engine fixes and additions, run through a probe script the way a
//! quest calls them.

use l2_util::ObjectId as Oid;
use l2_world::{Script, ScriptRegistry, ScriptSetup};

use crate::testing::{ScriptTest, X, Y, pages_to};

const NAME: &str = "Probe";
const NPC_ID: i32 = 31_000;
const NPCS: &str = r#"<npc id="31000" type="Folk" level="70" name="Probe"/>"#;

struct Probe;

impl Script for Probe {
    fn name(&self) -> &'static str {
        NAME
    }

    fn path(&self) -> &'static str {
        "test/Probe"
    }

    fn id(&self) -> i32 {
        30_000
    }

    fn setup(&self, _s: &mut ScriptSetup) {}
}

fn setup() -> ScriptTest {
    let mut registry = ScriptRegistry::new();
    registry.add(Probe);
    let mut t = ScriptTest::new(registry, NPCS, "", &[]);
    t.enter(1, 40, vec![]);
    t
}

#[test]
fn is_cond_0_is_true_for_a_fresh_and_for_a_removed_state() {
    let mut t = setup();
    t.world.run_as_script(NAME, |ctx| {
        let qs = ctx.get_quest_state(Oid(1), true).unwrap();
        // Java `isCond(0)` of a new `QuestState` (`_cond = 0`).
        assert!(qs.is_cond(ctx, 0));
        assert!(!qs.is_cond(ctx, 1));
        assert!(qs.is_memo_state(ctx, 0));
        qs.start_quest(ctx);
        assert!(qs.is_cond(ctx, 1));
        assert!(!qs.is_cond(ctx, 0));
        // `exitQuest(true)` removes the state: the handle reads like an empty one.
        qs.exit_quest(ctx, true, false);
        assert!(qs.is_cond(ctx, 0));
        assert!(!qs.is_cond(ctx, 1));
    });
}

#[test]
fn is_inside_radius_2d_is_strict_like_java() {
    let mut t = setup();
    // Exactly 100 east of the player: Java `calculateDistance2D(..) < 100` is false.
    let at_100 = t.spawn(NPC_ID, 100);
    let at_99 = t.spawn(NPC_ID, 99);
    t.world.run_as_script(NAME, |ctx| {
        assert!(!ctx.is_inside_radius_2d(Oid(1), at_100, 100));
        assert!(ctx.is_inside_radius_2d(Oid(1), at_100, 101));
        assert!(ctx.is_inside_radius_2d(Oid(1), at_99, 100));
        assert!(!ctx.is_inside_radius_2d_xy(Oid(1), X + 100, Y, 100));
        assert!(ctx.is_inside_radius_2d_xy(Oid(1), X + 60, Y + 79, 100));
        // Not in the world.
        assert!(!ctx.is_inside_radius_2d(Oid(1), Oid(424_242), 100_000));
    });
}

/// S-Q-18 saw a click on Chamberlain Logan (35274) answer with `MoveToPawn` + `ActionFailed`
/// and no page. The castle chamberlains are built-in castle scripts (`residence/npc/`), whose
/// `onFirstTalk` page (`chamberlain-04.html` for a stranger) lives in
/// `ai/others/CastleChamberlain/`; the quest test world had no such file, and Java's
/// `showHtmlFile` sends nothing for a missing page. With the page (the real datapack has it) the
/// click shows it, so the real server does not have the problem.
#[test]
fn a_click_on_a_castle_chamberlain_shows_its_first_talk_page() {
    let npcs = r#"<npc id="35274" type="Merchant" level="75" name="Logan"/>"#;
    let page = (
        "data/scripts/ai/others/CastleChamberlain/chamberlain-04.html",
        "<html><body>chamberlain-04 %objectId%</body></html>",
    );
    for with_page in [true, false] {
        let mut registry = ScriptRegistry::new();
        registry.add(Probe);
        let pages: &[(&str, &str)] = if with_page { &[page] } else { &[] };
        let mut t = ScriptTest::new(registry, npcs, "", pages);
        t.enter(1, 40, vec![]);
        let logan = t.spawn(35_274, 20);
        t.talk(1, logan);
        let shown = pages_to(&t.take(), 1);
        if with_page {
            assert_eq!(
                shown,
                [format!("<html><body>chamberlain-04 {}</body></html>", logan.0)]
            );
        } else {
            // The S-Q-18 observation: nothing, as Java's `showHtmlFile` of a missing page.
            assert!(shown.is_empty(), "{shown:?}");
        }
    }
}
