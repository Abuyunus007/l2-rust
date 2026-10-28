//! Quest 35: Find Glittering Jewelry (Ellie and Felton in the Talking Island).
// Java: dist/game/data/scripts/quests/Q00035_FindGlitteringJewelry/Q00035_FindGlitteringJewelry.java
//
// Pattern: a kill-count chain with a party-member check (`docs/design/scripts.md`, "Quest
// patterns"): the alligators give rough jewels to the killer at cond 2 with a 50% roll, and the
// member at cond 2 moves on to cond 3 when the tenth comes in.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ELLIE: i32 = 30091;
const FELTON: i32 = 30879;

// Monster
const ALLIGATOR: i32 = 20135;

// Items
const SILVER_NUGGET: i32 = 1873;
const ORIHARUKON: i32 = 1893;
const THONS: i32 = 4044;
const JEWEL_BOX: i32 = 7077;
const ROUGH_JEWEL: i32 = 7162;

// Misc
const MIN_LEVEL: i32 = 60;
const JEWEL_COUNT: i64 = 10;
const ORIHARUKON_COUNT: i64 = 5;
const NUGGET_COUNT: i64 = 500;
const THONS_COUNT: i64 = 150;

/// Java `Q00035_FindGlitteringJewelry`.
pub struct Q00035FindGlitteringJewelry;

impl Script for Q00035FindGlitteringJewelry {
    fn name(&self) -> &'static str {
        "Q00035_FindGlitteringJewelry"
    }

    fn path(&self) -> &'static str {
        "quests/Q00035_FindGlitteringJewelry"
    }

    fn id(&self) -> i32 {
        35
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ELLIE]);
        s.add_talk_id(&[ELLIE, FELTON]);
        s.add_kill_id(&[ALLIGATOR]);
        s.register_quest_items(&[ROUGH_JEWEL]);
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
            "30091-03.htm" => {
                qs.start_quest(ctx);
            }
            "30879-02.html" => {
                qs.set_cond(ctx, 2, true);
            }
            "30091-07.html" => {
                if ctx.get_quest_items_count(player, ROUGH_JEWEL) < JEWEL_COUNT {
                    return Some("30091-08.html".to_owned());
                }
                ctx.take_items(player, ROUGH_JEWEL, -1);
                qs.set_cond(ctx, 4, true);
            }
            "30091-11.html" => {
                if ctx.get_quest_items_count(player, ORIHARUKON) >= ORIHARUKON_COUNT
                    && ctx.get_quest_items_count(player, SILVER_NUGGET) >= NUGGET_COUNT
                    && ctx.get_quest_items_count(player, THONS) >= THONS_COUNT
                {
                    ctx.take_items(player, ORIHARUKON, ORIHARUKON_COUNT);
                    ctx.take_items(player, SILVER_NUGGET, NUGGET_COUNT);
                    ctx.take_items(player, THONS, THONS_COUNT);
                    ctx.give_items(player, JEWEL_BOX, 1);
                    qs.exit_quest(ctx, false, true);
                } else {
                    htmltext = Some("30091-12.html".to_owned());
                }
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        if let Some(member) = ctx.get_random_party_member_cond(player, 2)
            && ctx.get_random_boolean()
        {
            ctx.give_items(player, ROUGH_JEWEL, 1);
            if ctx.get_quest_items_count(player, ROUGH_JEWEL) >= JEWEL_COUNT {
                if let Some(qs) = ctx.get_quest_state(member, false) {
                    qs.set_cond(ctx, 3, true);
                }
            } else {
                ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ELLIE => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30091-01.htm"
                    } else {
                        "30091-02.html"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "30091-04.html".to_owned(),
                    3 => {
                        htmltext = if ctx.get_quest_items_count(player, ROUGH_JEWEL) >= JEWEL_COUNT {
                            "30091-06.html"
                        } else {
                            "30091-05.html"
                        }
                        .to_owned();
                    }
                    4 => {
                        htmltext = if ctx.get_quest_items_count(player, ORIHARUKON) >= ORIHARUKON_COUNT
                            && ctx.get_quest_items_count(player, SILVER_NUGGET) >= NUGGET_COUNT
                            && ctx.get_quest_items_count(player, THONS) >= THONS_COUNT
                        {
                            "30091-09.html"
                        } else {
                            "30091-10.html"
                        }
                        .to_owned();
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            FELTON => {
                if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30879-01.html".to_owned();
                    } else if qs.is_cond(ctx, 2) {
                        htmltext = "30879-03.html".to_owned();
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
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00035_FindGlitteringJewelry";

    const NPCS: &str = r#"
        <npc id="30091" type="Folk" level="70" name="Ellie"/>
        <npc id="30879" type="Folk" level="70" name="Felton"/>
        <npc id="20135" type="Monster" level="30" name="Alligator"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1873" type="EtcItem" name="Silver Nugget"><set name="is_stackable" val="true"/></item>
        <item id="1893" type="EtcItem" name="Oriharukon"><set name="is_stackable" val="true"/></item>
        <item id="4044" type="EtcItem" name="Thons"><set name="is_stackable" val="true"/></item>
        <item id="7077" type="EtcItem" name="Jewel Box"/>
        <item id="7162" type="EtcItem" name="Rough Jewel"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        ellie: Oid,
        felton: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00035FindGlitteringJewelry);
        let pages = Pages::of(&Q00035FindGlitteringJewelry)
            .page("30091-01.htm", &["30091-03.htm"])
            .page("30091-06.html", &["30091-07.html"])
            .page("30091-09.html", &["30091-11.html"])
            .page("30879-01.html", &["30879-02.html"])
            .plain(&[
                "30091-02.html",
                "30091-03.htm",
                "30091-04.html",
                "30091-05.html",
                "30091-07.html",
                "30091-08.html",
                "30091-10.html",
                "30091-11.html",
                "30091-12.html",
                "30879-02.html",
                "30879-03.html",
            ])
            .npcs(&[ELLIE, FELTON]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            ellie: t.spawn(ELLIE, 20),
            felton: t.spawn(FELTON, 40),
        };
        (t, npcs)
    }

    /// Kills alligators until the player has 10 rough jewels; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..400 {
            let alligator = t.spawn(ALLIGATOR, 80);
            t.kill(1, alligator);
            if t.count(1, ROUGH_JEWEL) >= JEWEL_COUNT {
                return kills;
            }
        }
        panic!("no 10 rough jewels after 400 kills");
    }

    #[test]
    fn a_level_59_character_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 59, vec![]);
        assert!(t.quest_page(1, npcs.ellie).contains("30091-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_jewel_box_for_the_materials() {
        let (mut t, npcs) = setup();
        t.enter(
            1,
            60,
            vec![
                item(1001, ORIHARUKON, 5),
                item(1002, SILVER_NUGGET, 500),
                item(1003, THONS, 150),
            ],
        );
        assert!(t.quest_page(1, npcs.ellie).contains("30091-01.htm"));
        assert!(t.event(1, NAME, "30091-03.htm").contains("30091-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.ellie).contains("30091-04.html"));

        // Felton sends the player out for the jewels: cond 2.
        assert!(t.quest_page(1, npcs.felton).contains("30879-01.html"));
        assert!(t.event(1, NAME, "30879-02.html").contains("30879-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.felton).contains("30879-03.html"));

        // The alligators give rough jewels to the killer; the tenth sets cond 3.
        hunt(&mut t);
        assert_eq!(t.count(1, ROUGH_JEWEL), 10);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Ellie takes the jewels: cond 4 (the page with the first refusal first).
        assert!(t.quest_page(1, npcs.ellie).contains("30091-06.html"));
        assert!(t.event(1, NAME, "30091-07.html").contains("30091-07.html"));
        assert_eq!(t.count(1, ROUGH_JEWEL), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // The materials are all there: the jewel box, the end.
        assert!(t.quest_page(1, npcs.ellie).contains("30091-09.html"));
        assert!(t.event(1, NAME, "30091-11.html").contains("30091-11.html"));
        assert_eq!(t.count(1, JEWEL_BOX), 1);
        assert_eq!(
            (
                t.count(1, ORIHARUKON),
                t.count(1, SILVER_NUGGET),
                t.count(1, THONS)
            ),
            (0, 0, 0)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.ellie)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn too_few_jewels_or_materials_are_refused() {
        let (mut t, npcs) = setup();
        t.enter(1, 60, vec![item(1001, ORIHARUKON, 5)]);
        t.quest_page(1, npcs.ellie);
        t.event(1, NAME, "30091-03.htm");
        t.take_writes();

        // Ellie's `30091-07.html` with no jewels: the page for it, cond stays 1. The real page
        // is only linked at cond 3, so the event runs directly (`run_as_script`).
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00035FindGlitteringJewelry.on_event(ctx, "30091-07.html", None, Some(Oid(1)));
        });
        assert_eq!(out.as_deref(), Some("30091-08.html"));
        assert!(t.take_writes().is_empty());

        // The materials are missing: the page for it, nothing taken.
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00035FindGlitteringJewelry.on_event(ctx, "30091-11.html", None, Some(Oid(1)));
        });
        assert_eq!(out.as_deref(), Some("30091-12.html"));
        assert_eq!(t.count(1, ORIHARUKON), 5);
        assert_eq!(t.count(1, JEWEL_BOX), 0);
        assert!(t.take_writes().is_empty());
    }
}
