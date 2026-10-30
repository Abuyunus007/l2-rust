//! Quest 10282: To the Seed of Annihilation (Commander Kbaldir sends the order to Officer Klemis).
// Java: dist/game/data/scripts/quests/Q10282_ToTheSeedOfAnnihilation/Q10282_ToTheSeedOfAnnihilation.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const KBALDIR: i32 = 32733;
const KLEMIS: i32 = 32734;

// Item
const SOA_ORDERS: i32 = 15512;

/// Java `Q10282_ToTheSeedOfAnnihilation`.
pub struct Q10282ToTheSeedOfAnnihilation;

impl Script for Q10282ToTheSeedOfAnnihilation {
    fn name(&self) -> &'static str {
        "Q10282_ToTheSeedOfAnnihilation"
    }

    fn path(&self) -> &'static str {
        "quests/Q10282_ToTheSeedOfAnnihilation"
    }

    fn id(&self) -> i32 {
        10282
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KBALDIR]);
        s.add_talk_id(&[KBALDIR, KLEMIS]);
        s.register_quest_items(&[SOA_ORDERS]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // Every link answers with its own page (`return htmltext`), with or without a state.
        if let Some(qs) = ctx.get_quest_state(player, false) {
            match event {
                "32733-07.htm" => {
                    qs.start_quest(ctx);
                    ctx.give_items(player, SOA_ORDERS, 1);
                }
                "32734-02.htm" => {
                    ctx.give_adena(player, 212_182, true);
                    ctx.add_exp_and_sp(player, 1_148_480, 99_110);
                    qs.exit_quest(ctx, false, false);
                }
                _ => {}
            }
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let npc_id = npc.id;
        match qs.state(ctx) {
            COMPLETED => {
                if npc_id == KBALDIR {
                    htmltext = "32733-09.htm".to_owned();
                } else if npc_id == KLEMIS {
                    htmltext = "32734-03.htm".to_owned();
                }
            }
            CREATED => {
                htmltext = if ctx.player_level(player) < 84 {
                    "32733-00.htm"
                } else {
                    "32733-01.htm"
                }
                .to_owned();
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    if npc_id == KBALDIR {
                        htmltext = "32733-08.htm".to_owned();
                    } else if npc_id == KLEMIS {
                        htmltext = "32734-01.htm".to_owned();
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

    const NAME: &str = "Q10282_ToTheSeedOfAnnihilation";

    const NPCS: &str = r#"
        <npc id="32733" type="Folk" level="85" name="Commander Kbaldir"/>
        <npc id="32734" type="Folk" level="85" name="Officer Klemis"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15512" type="EtcItem" name="SoA Orders"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10282ToTheSeedOfAnnihilation);
        let pages = Pages::of(&Q10282ToTheSeedOfAnnihilation)
            .plain(&["32733-00.htm", "32733-05.htm", "32733-07.htm", "32733-08.htm"])
            .plain(&["32733-09.htm", "32734-02.htm", "32734-03.htm"])
            .page("32733-01.htm", &["32733-02.htm"])
            .page("32733-02.htm", &["32733-03.htm", "32733-04.htm"])
            .page("32733-03.htm", &["32733-04.htm"])
            .page("32733-04.htm", &["32733-07.htm"])
            .page("32733-06.htm", &["32733-07.htm"])
            .page("32734-01.htm", &["32734-02.htm"])
            .npcs(&[KBALDIR, KLEMIS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kbaldir = t.spawn(KBALDIR, 20);
        let klemis = t.spawn(KLEMIS, 40);
        (t, kbaldir, klemis)
    }

    #[test]
    fn a_character_below_level_84_is_sent_away() {
        let (mut t, kbaldir, _) = setup();
        t.enter(1, 83, vec![]);
        assert!(t.quest_page(1, kbaldir).contains("32733-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_order_from_kbaldir_to_klemis_and_the_reward() {
        let (mut t, kbaldir, klemis) = setup();
        t.enter(1, 84, vec![]);
        assert!(t.quest_page(1, kbaldir).contains("32733-01.htm"));
        assert!(t.event(1, NAME, "32733-02.htm").contains("32733-02.htm"));
        assert!(t.event(1, NAME, "32733-04.htm").contains("32733-04.htm"));

        // Accept: `startQuest()` and the order.
        assert!(t.event(1, NAME, "32733-07.htm").contains("32733-07.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert_eq!(t.count(1, SOA_ORDERS), 1);
        assert!(t.quest_page(1, kbaldir).contains("32733-08.htm"));
        assert!(t.quest_page(1, klemis).contains("32734-01.htm"));

        // Klemis takes the order: the reward and `exitQuest(false)`, which plays no sound.
        assert!(t.event(1, NAME, "32734-02.htm").contains("32734-02.htm"));
        assert_eq!(t.count(1, 57), 212_182);
        assert_eq!(t.count(1, SOA_ORDERS), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(t.quest_page(1, kbaldir).contains("32733-09.htm"));
        assert!(t.quest_page(1, klemis).contains("32734-03.htm"));
    }
}
