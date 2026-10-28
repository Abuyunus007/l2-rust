//! Quest 182: New Recruits (Kekropus in the Kamael village sends non-Kamael to the Menacing
//! Machine in Nornil's Garden).
// Java: dist/game/data/scripts/quests/Q00182_NewRecruits/Q00182_NewRecruits.java
//
// Pattern: a race- and class-restricted quest (`docs/design/scripts.md`, "Quest patterns"):
// `ctx.player_race(p)` against `race::*` and `ctx.player_class_level(p)` (Java
// `getPlayerClass().level()`, 0 = no class change yet) decide the first page. The reward is the
// player's choice of two items (two events).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const KEKROPUS: i32 = 32138;
const MENACING_MACHINE: i32 = 32258;

/// Java `Q00182_NewRecruits`.
pub struct Q00182NewRecruits;

impl Script for Q00182NewRecruits {
    fn name(&self) -> &'static str {
        "Q00182_NewRecruits"
    }

    fn path(&self) -> &'static str {
        "quests/Q00182_NewRecruits"
    }

    fn id(&self) -> i32 {
        182
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KEKROPUS]);
        s.add_talk_id(&[KEKROPUS, MENACING_MACHINE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let htmltext = Some(event.to_owned());
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return htmltext;
        };
        // Java reads `npc.getId()` without a null check (a bypass always has the NPC).
        let npc = npc?;
        if npc.id == KEKROPUS {
            if event.eq_ignore_ascii_case("32138-03.html") {
                qs.start_quest(ctx);
            }
        } else if npc.id == MENACING_MACHINE {
            if event.eq_ignore_ascii_case("32258-04.html") {
                ctx.give_items(player, 847, 2);
                qs.exit_quest(ctx, false, true);
            } else if event.eq_ignore_ascii_case("32258-05.html") {
                ctx.give_items(player, 890, 2);
                qs.exit_quest(ctx, false, true);
            }
        }
        // NOTE(java-quirk): the event is the page whatever happened, so the reward pages can be
        // asked for again after the quest is done (with no second reward).
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let npc_id = npc.id;
        if npc_id == KEKROPUS {
            match qs.state(ctx) {
                CREATED => {
                    let level = ctx.player_level(player);
                    htmltext = if ctx.player_race(player) == race::KAMAEL {
                        "32138-00.html"
                    } else if (17..=21).contains(&level) && ctx.player_class_level(player) == 0 {
                        "32138-01.htm"
                    } else {
                        "32138-00b.html"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "32138-04.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            }
        } else if npc_id == MENACING_MACHINE && qs.is_started(ctx) {
            htmltext = "32258-01.html".to_owned();
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::player_class;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00182_NewRecruits";

    const NPCS: &str = r#"
        <npc id="32138" type="Folk" level="70" name="Kekropus"/>
        <npc id="32258" type="Folk" level="70" name="Menacing Machine"/>"#;

    const ITEMS: &str = r#"
        <item id="847" type="Armor" name="Red Crescent Earring"><set name="bodypart" val="rear;lear"/></item>
        <item id="890" type="Armor" name="Ring of Devotion"><set name="bodypart" val="rfinger;lfinger"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00182NewRecruits);
        let pages = Pages::of(&Q00182NewRecruits)
            .page("32138-01.htm", &["32138-02.htm"])
            .page("32138-02.htm", &["32138-03.html"])
            .page("32258-01.html", &["32258-02.html"])
            .page("32258-02.html", &["32258-03.html"])
            .page("32258-03.html", &["32258-04.html", "32258-05.html"])
            .plain(&[
                "32138-00.html",
                "32138-00b.html",
                "32138-03.html",
                "32138-04.html",
            ])
            // The reward pages link to `NornilsGarden exit`, another script.
            .plain(&["32258-04.html", "32258-05.html"])
            .npcs(&[KEKROPUS, MENACING_MACHINE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kekropus = t.spawn(KEKROPUS, 20);
        let machine = t.spawn(MENACING_MACHINE, 40);
        (t, kekropus, machine)
    }

    #[test]
    fn kamael_too_high_and_class_changed_characters_are_refused() {
        let (mut t, kekropus, _) = setup();
        t.enter_as(1, 18, race::KAMAEL, player_class::MALE_SOLDIER, vec![]);
        assert!(t.quest_page(1, kekropus).contains("32138-00.html"));
        t.enter_as(2, 22, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, kekropus).contains("32138-00b.html"));
        t.enter_as(3, 20, race::HUMAN, player_class::WARRIOR, vec![]);
        assert!(t.quest_page(3, kekropus).contains("32138-00b.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_human_fighter_chooses_the_ring() {
        let (mut t, kekropus, machine) = setup();
        t.enter_as(1, 17, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, kekropus).contains("32138-01.htm"));
        t.event(1, NAME, "32138-02.htm");
        assert!(t.event(1, NAME, "32138-03.html").contains("32138-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, kekropus).contains("32138-04.html"));

        assert!(t.quest_page(1, machine).contains("32258-01.html"));
        t.event(1, NAME, "32258-02.html");
        t.event(1, NAME, "32258-03.html");
        assert!(t.event(1, NAME, "32258-05.html").contains("32258-05.html"));
        assert_eq!((t.count(1, 890), t.count(1, 847)), (2, 0));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, kekropus)
                .contains("This quest has already been completed.")
        );
    }
}
