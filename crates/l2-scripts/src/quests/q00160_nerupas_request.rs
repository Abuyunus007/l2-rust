//! Quest 160: Nerupa's Request (an Elf carries a spider silk through four NPCs for a leaf).
// Java: dist/game/data/scripts/quests/Q00160_NerupasRequest/Q00160_NerupasRequest.java
//
// Pattern: a delivery chain where each NPC swaps one quest item for the next (cond 2, 3, 4).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const NERUPA: i32 = 30370;
const UNOREN: i32 = 30147;
const CREAMEES: i32 = 30149;
const JULIA: i32 = 30152;

// Items
const SILVERY_SPIDERSILK: i32 = 1026;
const UNOS_RECEIPT: i32 = 1027;
const CELS_TICKET: i32 = 1028;
const NIGHTSHADE_LEAF: i32 = 1029;

// Reward
const LESSER_HEALING_POTION: i32 = 1060;

// Misc
const MIN_LEVEL: i32 = 3;

/// Java `Q00160_NerupasRequest`.
pub struct Q00160NerupasRequest;

impl Script for Q00160NerupasRequest {
    fn name(&self) -> &'static str {
        "Q00160_NerupasRequest"
    }

    fn path(&self) -> &'static str {
        "quests/Q00160_NerupasRequest"
    }

    fn id(&self) -> i32 {
        160
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NERUPA]);
        s.add_talk_id(&[NERUPA, UNOREN, CREAMEES, JULIA]);
        s.register_quest_items(&[SILVERY_SPIDERSILK, UNOS_RECEIPT, CELS_TICKET, NIGHTSHADE_LEAF]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false);
        if let Some(qs) = qs
            && event == "30370-04.htm"
        {
            qs.start_quest(ctx);
            if !ctx.has_quest_items(player, SILVERY_SPIDERSILK) {
                ctx.give_items(player, SILVERY_SPIDERSILK, 1);
            }
            return Some(event.to_owned());
        }
        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                if npc.id == NERUPA {
                    htmltext = if ctx.player_race(player) != race::ELF {
                        "30370-01.htm".to_owned()
                    } else if ctx.player_level(player) < MIN_LEVEL {
                        "30370-02.htm".to_owned()
                    } else {
                        "30370-03.htm".to_owned()
                    };
                }
            }
            STARTED => match npc.id {
                NERUPA => {
                    if ctx
                        .has_at_least_one_quest_item(player, &[SILVERY_SPIDERSILK, UNOS_RECEIPT, CELS_TICKET])
                    {
                        htmltext = "30370-05.html".to_owned();
                    } else if ctx.has_quest_items(player, NIGHTSHADE_LEAF) {
                        ctx.give_items(player, LESSER_HEALING_POTION, 5);
                        ctx.add_exp_and_sp(player, 1_000, 0);
                        qs.exit_quest(ctx, false, true);
                        htmltext = "30370-06.html".to_owned();
                    }
                }
                UNOREN => {
                    if ctx.has_quest_items(player, SILVERY_SPIDERSILK) {
                        ctx.take_items(player, SILVERY_SPIDERSILK, -1);
                        if !ctx.has_quest_items(player, UNOS_RECEIPT) {
                            ctx.give_items(player, UNOS_RECEIPT, 1);
                        }
                        qs.set_cond(ctx, 2, true);
                        htmltext = "30147-01.html".to_owned();
                    } else if ctx.has_quest_items(player, UNOS_RECEIPT) {
                        htmltext = "30147-02.html".to_owned();
                    } else if ctx.has_quest_items(player, NIGHTSHADE_LEAF) {
                        htmltext = "30147-03.html".to_owned();
                    }
                }
                CREAMEES => {
                    if ctx.has_quest_items(player, UNOS_RECEIPT) {
                        ctx.take_items(player, UNOS_RECEIPT, -1);
                        if !ctx.has_quest_items(player, CELS_TICKET) {
                            ctx.give_items(player, CELS_TICKET, 1);
                        }
                        qs.set_cond(ctx, 3, true);
                        htmltext = "30149-01.html".to_owned();
                    } else if ctx.has_quest_items(player, CELS_TICKET) {
                        htmltext = "30149-02.html".to_owned();
                    } else if ctx.has_quest_items(player, NIGHTSHADE_LEAF) {
                        htmltext = "30149-03.html".to_owned();
                    }
                }
                JULIA => {
                    if ctx.has_quest_items(player, CELS_TICKET) {
                        ctx.take_items(player, CELS_TICKET, -1);
                        if !ctx.has_quest_items(player, NIGHTSHADE_LEAF) {
                            ctx.give_items(player, NIGHTSHADE_LEAF, 1);
                        }
                        qs.set_cond(ctx, 4, true);
                        htmltext = "30152-01.html".to_owned();
                    } else if ctx.has_quest_items(player, NIGHTSHADE_LEAF) {
                        htmltext = "30152-02.html".to_owned();
                    }
                }
                _ => {}
            },
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

    const NAME: &str = "Q00160_NerupasRequest";

    const NPCS: &str = r#"
        <npc id="30370" type="Folk" level="1" name="Nerupa"/>
        <npc id="30147" type="Folk" level="1" name="Unoren"/>
        <npc id="30149" type="Folk" level="1" name="Creamees"/>
        <npc id="30152" type="Folk" level="1" name="Julia"/>"#;

    const ITEMS: &str = r#"
        <item id="1026" type="EtcItem" name="Silvery Spidersilk"><set name="is_questitem" val="true"/></item>
        <item id="1027" type="EtcItem" name="Uno's Receipt"><set name="is_questitem" val="true"/></item>
        <item id="1028" type="EtcItem" name="Cel's Ticket"><set name="is_questitem" val="true"/></item>
        <item id="1029" type="EtcItem" name="Nightshade Leaf"><set name="is_questitem" val="true"/></item>
        <item id="1060" type="EtcItem" name="Lesser Healing Potion"><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        nerupa: Oid,
        unoren: Oid,
        creamees: Oid,
        julia: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00160NerupasRequest);
        let pages = Pages::of(&Q00160NerupasRequest)
            .page("30370-03.htm", &["30370-04.htm"])
            .plain(&[
                "30370-01.htm",
                "30370-02.htm",
                "30370-04.htm",
                "30370-05.html",
                "30370-06.html",
                "30147-01.html",
                "30147-02.html",
                "30147-03.html",
                "30149-01.html",
                "30149-02.html",
                "30149-03.html",
                "30152-01.html",
                "30152-02.html",
            ])
            .npcs(&[NERUPA, UNOREN, CREAMEES, JULIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            nerupa: t.spawn(NERUPA, 20),
            unoren: t.spawn(UNOREN, 30),
            creamees: t.spawn(CREAMEES, 40),
            julia: t.spawn(JULIA, 50),
        };
        (t, npcs)
    }

    #[test]
    fn only_an_elf_of_level_3_starts_the_quest() {
        let (mut t, npcs) = setup();
        t.enter(1, 3, vec![]);
        assert!(t.quest_page(1, npcs.nerupa).contains("30370-01.htm"));
        t.enter_as(2, 2, race::ELF, 0, vec![]);
        assert!(t.quest_page(2, npcs.nerupa).contains("30370-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_silk_travels_to_julia_and_the_leaf_buys_five_potions() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 3, race::ELF, 0, vec![]);
        assert!(t.quest_page(1, npcs.nerupa).contains("30370-03.htm"));
        assert!(t.event(1, NAME, "30370-04.htm").contains("30370-04.htm"));
        assert_eq!(t.count(1, SILVERY_SPIDERSILK), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.nerupa).contains("30370-05.html"));

        assert!(t.quest_page(1, npcs.unoren).contains("30147-01.html"));
        assert_eq!(t.count(1, UNOS_RECEIPT), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.unoren).contains("30147-02.html"));

        assert!(t.quest_page(1, npcs.creamees).contains("30149-01.html"));
        assert_eq!(t.count(1, CELS_TICKET), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        assert!(t.quest_page(1, npcs.julia).contains("30152-01.html"));
        assert_eq!(t.count(1, NIGHTSHADE_LEAF), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        assert!(t.quest_page(1, npcs.julia).contains("30152-02.html"));
        assert!(t.quest_page(1, npcs.creamees).contains("30149-03.html"));
        assert!(t.quest_page(1, npcs.unoren).contains("30147-03.html"));

        assert!(t.quest_page(1, npcs.nerupa).contains("30370-06.html"));
        assert_eq!(t.count(1, LESSER_HEALING_POTION), 5);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
