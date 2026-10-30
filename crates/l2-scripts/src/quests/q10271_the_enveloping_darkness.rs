//! Quest 10271: The Enveloping Darkness (Orbyu sends the adventurer to El and the medibal corpse for a document).
// Java: dist/game/data/scripts/quests/Q10271_TheEnvelopingDarkness/Q10271_TheEnvelopingDarkness.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ORBYU: i32 = 32560;
const EL: i32 = 32556;
const MEDIBAL_CORPSE: i32 = 32528;

// Item
const MEDIBAL_DOCUMENT: i32 = 13852;

/// Java `Q10269_ToTheSeedOfDestruction.class.getSimpleName()`: the quest before this one.
const Q10269: &str = "Q10269_ToTheSeedOfDestruction";

/// Java `Q10271_TheEnvelopingDarkness`.
pub struct Q10271TheEnvelopingDarkness;

impl Script for Q10271TheEnvelopingDarkness {
    fn name(&self) -> &'static str {
        "Q10271_TheEnvelopingDarkness"
    }

    fn path(&self) -> &'static str {
        "quests/Q10271_TheEnvelopingDarkness"
    }

    fn id(&self) -> i32 {
        10271
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ORBYU]);
        s.add_talk_id(&[ORBYU, EL, MEDIBAL_CORPSE]);
        s.register_quest_items(&[MEDIBAL_DOCUMENT]);
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

        match event {
            "32560-05.html" => qs.start_quest(ctx),
            "32556-06.html" => qs.set_cond(ctx, 2, true),
            "32556-09.html" => {
                if ctx.has_quest_items(player, MEDIBAL_DOCUMENT) {
                    ctx.take_items(player, MEDIBAL_DOCUMENT, -1);
                    qs.set_cond(ctx, 4, true);
                }
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = ctx.get_no_quest_msg(player);
        let qs = ctx.get_quest_state(player, true)?;

        match npc.id {
            ORBYU => match qs.state(ctx) {
                CREATED => {
                    let previous_done = ctx
                        .player_quest_state(player, Q10269)
                        .is_some_and(|s| s.is_completed());
                    htmltext = if ctx.player_level(player) >= 75 && previous_done {
                        "32560-01.htm"
                    } else {
                        "32560-02.html"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    // NOTE(java-quirk): Java's own TODO says this page should most probably differ.
                    1 => htmltext = "32560-05.html".to_owned(),
                    2 => htmltext = "32560-06.html".to_owned(),
                    3 => htmltext = "32560-07.html".to_owned(),
                    4 => {
                        htmltext = "32560-08.html".to_owned();
                        ctx.give_adena(player, 62_516, true);
                        ctx.add_exp_and_sp(player, 377_403, 37_867);
                        qs.exit_quest(ctx, false, true);
                    }
                    _ => {}
                },
                COMPLETED => htmltext = "32560-03.html".to_owned(),
                _ => {}
            },
            EL => {
                if qs.is_completed(ctx) {
                    htmltext = "32556-02.html".to_owned();
                } else if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "32556-01.html".to_owned(),
                        2 => htmltext = "32556-07.html".to_owned(),
                        3 => htmltext = "32556-08.html".to_owned(),
                        4 => htmltext = "32556-09.html".to_owned(),
                        _ => {}
                    }
                }
            }
            MEDIBAL_CORPSE => {
                if qs.is_completed(ctx) {
                    htmltext = "32528-02.html".to_owned();
                } else if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        2 => {
                            htmltext = "32528-01.html".to_owned();
                            qs.set_cond(ctx, 3, true);
                            ctx.give_items(player, MEDIBAL_DOCUMENT, 1);
                        }
                        3 | 4 => htmltext = "32528-03.html".to_owned(),
                        _ => {}
                    }
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
    use l2_world::script::COMPLETED;
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q10271_TheEnvelopingDarkness";

    const NPCS: &str = r#"
        <npc id="32560" type="Folk" level="80" name="Orbyu"/>
        <npc id="32556" type="Folk" level="80" name="El"/>
        <npc id="32528" type="Folk" level="80" name="Medibal Corpse"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13852" type="EtcItem" name="Medibal Document"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q10269_ToTheSeedOfDestruction`, as far as this quest reads it: its state.
    struct SeedOfDestruction;

    impl Script for SeedOfDestruction {
        fn name(&self) -> &'static str {
            Q10269
        }
        fn path(&self) -> &'static str {
            "quests/Q10269_ToTheSeedOfDestruction"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10271TheEnvelopingDarkness);
        registry.add(SeedOfDestruction);
        let pages = Pages::of(&Q10271TheEnvelopingDarkness)
            .page("32560-01.htm", &["32560-04.htm"])
            .page("32560-04.htm", &["32560-05.html"])
            .plain(&["32560-02.html", "32560-03.html", "32560-05.html", "32560-06.html"])
            .plain(&["32560-07.html", "32560-08.html"])
            .page("32556-01.html", &["32556-03.html"])
            .page("32556-03.html", &["32556-04.html"])
            .page("32556-04.html", &["32556-05.html"])
            .page("32556-05.html", &["32556-06.html"])
            .plain(&["32556-02.html", "32556-06.html", "32556-07.html"])
            .page("32556-08.html", &["32556-09.html"])
            .plain(&["32556-09.html", "32528-01.html", "32528-02.html", "32528-03.html"])
            .npcs(&[ORBYU, EL, MEDIBAL_CORPSE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let orbyu = t.spawn(ORBYU, 20);
        let el = t.spawn(EL, 40);
        let corpse = t.spawn(MEDIBAL_CORPSE, 60);
        (t, orbyu, el, corpse)
    }

    fn complete_seed_of_destruction(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(Q10269, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(player), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
    }

    #[test]
    fn the_quest_needs_the_seed_of_destruction_and_level_75() {
        let (mut t, orbyu, _, _) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, orbyu).contains("32560-02.html"));
        complete_seed_of_destruction(&mut t, 1);
        t.take_writes();
        assert!(t.quest_page(1, orbyu).contains("32560-01.htm"));
    }

    #[test]
    fn el_then_the_corpse_then_el_then_orbyu_pays() {
        let (mut t, orbyu, el, corpse) = setup();
        t.enter(1, 75, vec![]);
        complete_seed_of_destruction(&mut t, 1);
        t.take_writes();
        assert!(t.quest_page(1, orbyu).contains("32560-01.htm"));
        assert!(t.event(1, NAME, "32560-04.htm").contains("32560-04.htm"));
        assert!(t.event(1, NAME, "32560-05.html").contains("32560-05.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );

        // El sends the adventurer to the corpse: cond 2 on `32556-06.html`.
        assert!(t.quest_page(1, el).contains("32556-01.html"));
        assert!(t.event(1, NAME, "32556-03.html").contains("32556-03.html"));
        assert!(t.event(1, NAME, "32556-04.html").contains("32556-04.html"));
        assert!(t.event(1, NAME, "32556-05.html").contains("32556-05.html"));
        assert!(t.event(1, NAME, "32556-06.html").contains("32556-06.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The corpse gives the document and cond 3.
        assert!(t.quest_page(1, corpse).contains("32528-01.html"));
        assert_eq!(t.count(1, MEDIBAL_DOCUMENT), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // El takes the document: cond 4, the document goes.
        assert!(t.quest_page(1, el).contains("32556-08.html"));
        assert!(t.event(1, NAME, "32556-09.html").contains("32556-09.html"));
        assert_eq!(t.count(1, MEDIBAL_DOCUMENT), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Orbyu pays: adena and `exitQuest(false, true)`.
        assert!(t.quest_page(1, orbyu).contains("32560-08.html"));
        assert_eq!(t.count(1, 57), 62_516);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(t.quest_page(1, orbyu).contains("32560-03.html"));
    }
}
