//! Quest 13: Parcel Delivery (Fundin in Rune hands a parcel to Vulcan, for a level 74 character).
// Java: dist/game/data/scripts/quests/Q00013_ParcelDelivery/Q00013_ParcelDelivery.java
//
// Pattern: a single delivery to one NPC (`docs/design/scripts.md`, "Quest patterns"): the start
// NPC gives the package on the start event, and the receiver pays the reward items when the
// package is in hand. One-time: `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const FUNDIN: i32 = 31274;
const VULCAN: i32 = 31539;

// Item
const PACKAGE: i32 = 7263;

// Misc
const MIN_LEVEL: i32 = 74;

/// Java `Q00013_ParcelDelivery`.
pub struct Q00013ParcelDelivery;

impl Script for Q00013ParcelDelivery {
    fn name(&self) -> &'static str {
        "Q00013_ParcelDelivery"
    }

    fn path(&self) -> &'static str {
        "quests/Q00013_ParcelDelivery"
    }

    fn id(&self) -> i32 {
        13
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[FUNDIN]);
        s.add_talk_id(&[FUNDIN, VULCAN]);
        s.register_quest_items(&[PACKAGE]);
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

        let mut htmltext = event.to_owned();
        match event {
            "31274-02.html" => {
                qs.start_quest(ctx);
                ctx.give_items(player, PACKAGE, 1);
            }
            "31539-01.html" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, PACKAGE) {
                    ctx.give_adena(player, 157_834, true);
                    ctx.add_exp_and_sp(player, 589_092, 58_794);
                    qs.exit_quest(ctx, false, true);
                } else {
                    htmltext = "31539-02.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let npc_id = npc.id;

        match qs.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                if npc_id == FUNDIN {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "31274-00.htm"
                    } else {
                        "31274-01.html"
                    }
                    .to_owned();
                }
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    if npc_id == FUNDIN {
                        htmltext = "31274-02.html".to_owned();
                    } else if npc_id == VULCAN {
                        htmltext = "31539-00.html".to_owned();
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00013_ParcelDelivery";

    const NPCS: &str = r#"
        <npc id="31274" type="Folk" level="70" name="Fundin"/>
        <npc id="31539" type="Folk" level="70" name="Vulcan"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7263" type="EtcItem" name="Package"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        fundin: Oid,
        vulcan: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00013ParcelDelivery);
        let pages = Pages::of(&Q00013ParcelDelivery)
            .page("31274-00.htm", &["31274-02.html"])
            .page("31539-00.html", &["31539-01.html"])
            .plain(&["31274-01.html", "31274-02.html", "31539-01.html", "31539-02.html"])
            .npcs(&[FUNDIN, VULCAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            fundin: t.spawn(FUNDIN, 20),
            vulcan: t.spawn(VULCAN, 40),
        };
        (t, npcs)
    }

    #[test]
    fn a_character_below_level_74_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 73, vec![]);
        assert!(t.quest_page(1, npcs.fundin).contains("31274-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_package_from_fundin_to_vulcan() {
        let (mut t, npcs) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, npcs.fundin).contains("31274-00.htm"));
        assert!(t.event(1, NAME, "31274-02.html").contains("31274-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(t.count(1, PACKAGE), 1);

        // Vulcan takes the package: the adena and the exp, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.vulcan).contains("31539-00.html"));
        assert!(t.event(1, NAME, "31539-01.html").contains("31539-01.html"));
        assert_eq!(t.count(1, 57), 157_834);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.fundin)
                .contains("This quest has already been completed.")
        );
    }
}
