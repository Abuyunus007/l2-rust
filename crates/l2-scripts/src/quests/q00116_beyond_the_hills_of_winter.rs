//! Quest 116: Beyond the Hills of Winter (Filaur in the Heine area, the supplier Obi).
// Java: dist/game/data/scripts/quests/Q00116_BeyondTheHillsOfWinter/Q00116_BeyondTheHillsOfWinter.java
//
// Pattern: a delivery with a memo state (`setMemoState`) that tells the two steps apart, and a
// reward the player picks from two links (`bypass -h Script ... MATERIAL` / `ADENA`). The
// `hasAllItems(player, true, ...)` of Java compares counts, not presence.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const FILAUR: i32 = 30535;
const OBI: i32 = 32052;

// Items (Java `ItemHolder`: id and count)
const THIEF_KEY: (i32, i64) = (1661, 10);
const BANDAGE: (i32, i64) = (1833, 20);
const ENERGY_STONE: (i32, i64) = (5589, 5);
const SUPPLYING_GOODS: i32 = 8098;

// Reward
const SOULSHOT_D: i32 = 1463;

// Misc
const MIN_LEVEL: i32 = 30;

/// Java `hasAllItems(player, true, ...)`: each holder's count is at least its count.
fn has_all_items(ctx: &ScriptCtx<'_>, player: ObjectId, items: &[(i32, i64)]) -> bool {
    items
        .iter()
        .all(|&(id, count)| ctx.get_quest_items_count(player, id) >= count)
}

/// Java `Q00116_BeyondTheHillsOfWinter`.
pub struct Q00116BeyondTheHillsOfWinter;

impl Script for Q00116BeyondTheHillsOfWinter {
    fn name(&self) -> &'static str {
        "Q00116_BeyondTheHillsOfWinter"
    }

    fn path(&self) -> &'static str {
        "quests/Q00116_BeyondTheHillsOfWinter"
    }

    fn id(&self) -> i32 {
        116
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[FILAUR]);
        s.add_talk_id(&[FILAUR, OBI]);
        s.register_quest_items(&[SUPPLYING_GOODS]);
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
            "30535-02.htm" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                htmltext = Some(event.to_owned());
            }
            "30535-05.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    ctx.give_items(player, SUPPLYING_GOODS, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "32052-02.html" => {
                if qs.is_memo_state(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "MATERIAL" => {
                if qs.is_memo_state(ctx, 2) {
                    ctx.reward_items(player, SOULSHOT_D, 1740);
                    ctx.add_exp_and_sp(player, 82_792, 4981);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some("32052-03.html".to_owned());
                }
            }
            "ADENA" => {
                if qs.is_memo_state(ctx, 2) {
                    ctx.give_adena(player, 17_387, true);
                    ctx.add_exp_and_sp(player, 82_792, 4981);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some("32052-03.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            COMPLETED => {
                if npc.id == FILAUR {
                    htmltext = ctx.get_already_completed_msg(player);
                }
            }
            CREATED => {
                if npc.id == FILAUR {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30535-01.htm"
                    } else {
                        "30535-03.htm"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                FILAUR => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = if has_all_items(ctx, player, &[THIEF_KEY, BANDAGE, ENERGY_STONE]) {
                            "30535-04.html"
                        } else {
                            "30535-06.html"
                        }
                        .to_owned();
                    } else if qs.is_memo_state(ctx, 2) {
                        htmltext = "30535-07.html".to_owned();
                    }
                }
                OBI => {
                    if qs.is_memo_state(ctx, 2) && ctx.has_quest_items(player, SUPPLYING_GOODS) {
                        htmltext = "32052-01.html".to_owned();
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
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"
        <npc id="30535" type="Folk" level="30" name="Filaur"/>
        <npc id="32052" type="Folk" level="30" name="Obi"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1463" type="EtcItem" name="Soulshot D"><set name="is_stackable" val="true"/></item>
        <item id="1661" type="EtcItem" name="Thief Key"><set name="is_stackable" val="true"/></item>
        <item id="1833" type="EtcItem" name="Bandage"><set name="is_stackable" val="true"/></item>
        <item id="5589" type="EtcItem" name="Energy Stone"><set name="is_stackable" val="true"/></item>
        <item id="8098" type="EtcItem" name="Supplying Goods"><set name="is_questitem" val="true"/></item>"#;

    const NAME: &str = "Q00116_BeyondTheHillsOfWinter";

    struct Npcs {
        filaur: Oid,
        obi: Oid,
    }

    /// The three items Filaur asks for, at their counts.
    fn materials() -> Vec<l2_world::ItemData> {
        vec![item(101, 1661, 10), item(102, 1833, 20), item(103, 5589, 5)]
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00116BeyondTheHillsOfWinter);
        let pages = Pages::of(&Q00116BeyondTheHillsOfWinter)
            .page("30535-01.htm", &["30535-02.htm"])
            .plain(&["30535-02.htm", "30535-03.htm", "30535-06.html", "30535-07.html"])
            .page("30535-04.html", &["30535-05.html"])
            .plain(&["30535-05.html"])
            .page("32052-01.html", &["32052-02.html"])
            .page("32052-02.html", &["ADENA", "MATERIAL"])
            .plain(&["32052-03.html"])
            .npcs(&[FILAUR, OBI])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            filaur: t.spawn(FILAUR, 20),
            obi: t.spawn(OBI, 40),
        };
        (t, npcs)
    }

    #[test]
    fn a_character_below_level_30_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 29, vec![]);
        assert!(t.quest_page(1, npcs.filaur).contains("30535-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn obi_has_nothing_to_say_before_the_quest() {
        let (mut t, npcs) = setup();
        t.enter(1, 30, vec![]);
        assert!(t.quest_page(1, npcs.obi).contains("noquest"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn filaur_asks_for_the_materials_and_gives_the_goods() {
        let (mut t, npcs) = setup();
        t.enter(1, 30, vec![]);
        assert!(t.quest_page(1, npcs.filaur).contains("30535-01.htm"));

        // Accept: `startQuest()`, `setMemoState(1)`.
        assert!(t.event(1, NAME, "30535-02.htm").contains("30535-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                insert(NAME, "cond", "1"),
                insert(NAME, "<state>", "Started"),
                insert(NAME, "memoState", "1"),
            ]
        );

        // Without the materials Filaur sends the player back.
        assert!(t.quest_page(1, npcs.filaur).contains("30535-06.html"));
    }

    #[test]
    fn the_materials_then_the_goods_then_the_reward_by_adena() {
        let (mut t, npcs) = setup();
        t.enter(1, 30, materials());
        t.quest_page(1, npcs.filaur);
        t.event(1, NAME, "30535-02.htm");
        t.take_writes();

        // With every material, Filaur hands over the goods (`setMemoState(2)`, cond 2).
        assert!(t.quest_page(1, npcs.filaur).contains("30535-04.html"));
        assert!(t.event(1, NAME, "30535-05.html").contains("30535-05.html"));
        assert_eq!(t.count(1, SUPPLYING_GOODS), 1);
        assert_eq!(
            t.take_writes(),
            [update(NAME, "memoState", "2"), update(NAME, "cond", "2")]
        );
        assert!(t.quest_page(1, npcs.filaur).contains("30535-07.html"));

        // Obi takes the goods; the reward is the adena (`ADENA`).
        assert!(t.quest_page(1, npcs.obi).contains("32052-01.html"));
        assert!(t.event(1, NAME, "32052-02.html").contains("32052-02.html"));
        assert!(t.event(1, NAME, "ADENA").contains("32052-03.html"));
        assert_eq!(t.count(1, 57), 17_387);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
        assert!(t.quest_page(1, npcs.filaur).contains("This quest has already been completed."));
    }

    #[test]
    fn the_soulshots_are_the_other_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 30, materials());
        t.quest_page(1, npcs.filaur);
        t.event(1, NAME, "30535-02.htm");
        t.quest_page(1, npcs.filaur);
        t.event(1, NAME, "30535-05.html");
        t.quest_page(1, npcs.obi);
        t.event(1, NAME, "32052-02.html");
        t.take_writes();

        assert!(t.event(1, NAME, "MATERIAL").contains("32052-03.html"));
        assert_eq!(t.count(1, SOULSHOT_D), 1740);
        assert_eq!(t.count(1, 57), 0);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
    }
}
