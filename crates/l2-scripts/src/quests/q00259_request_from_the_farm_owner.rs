//! Quest 259: Request from the Farm Owner (Edmond in the Talking Island farm collects spider skins
//! for Marius, who trades them for consumables; the rest of the skins are paid in adena).
// Java: dist/game/data/scripts/quests/Q00259_RequestFromTheFarmOwner/Q00259_RequestFromTheFarmOwner.java
//
// Pattern: a kill count with every kill giving the item, a repeatable quest, and a second NPC
// that trades the items (`docs/design/scripts.md`, "Quest patterns").

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// Npcs
const EDMOND: i32 = 30497;
const MARIUS: i32 = 30405;

// Monsters: Giant Spider, Talon Spider, Blade Spider
const MONSTERS: [i32; 3] = [20103, 20106, 20108];

// Items
const SPIDER_SKIN: i32 = 1495;

// Misc
const MIN_LEVEL: i32 = 15;
const SKIN_COUNT: i64 = 10;
const SKIN_REWARD: i64 = 25;
const SKIN_BONUS: i64 = 250;

/// Java `CONSUMABLES`: the item (id, count) Marius gives for a page.
fn consumable(event: &str) -> Option<(i32, i64)> {
    match event {
        "30405-04.html" => Some((1061, 2)),   // Greater Healing Potion
        "30405-05.html" => Some((17, 250)),   // Wooden Arrow
        "30405-05a.html" => Some((1835, 60)), // Soulshot: No Grade
        "30405-05c.html" => Some((2509, 30)), // Spiritshot: No Grade
        _ => None,
    }
}

/// Java `Q00259_RequestFromTheFarmOwner`.
pub struct Q00259RequestFromTheFarmOwner;

impl Script for Q00259RequestFromTheFarmOwner {
    fn name(&self) -> &'static str {
        "Q00259_RequestFromTheFarmOwner"
    }

    fn path(&self) -> &'static str {
        "quests/Q00259_RequestFromTheFarmOwner"
    }

    fn id(&self) -> i32 {
        259
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[EDMOND]);
        s.add_talk_id(&[EDMOND, MARIUS]);
        s.add_kill_id(&MONSTERS);
        s.register_quest_items(&[SPIDER_SKIN]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // Java returns null (no page) when there is no quest state.
        let qs = ctx.get_quest_state(player, false)?;
        match event {
            "30405-03.html" | "30405-05b.html" | "30405-05d.html" | "30497-07.html" => Some(event.to_owned()),
            "30405-04.html" | "30405-05.html" | "30405-05a.html" | "30405-05c.html" => {
                if ctx.get_quest_items_count(player, SPIDER_SKIN) >= SKIN_COUNT {
                    if let Some((item, count)) = consumable(event) {
                        ctx.give_items(player, item, count);
                    }
                    ctx.take_items(player, SPIDER_SKIN, SKIN_COUNT);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "30405-06.html" => Some(
                if ctx.get_quest_items_count(player, SPIDER_SKIN) >= SKIN_COUNT {
                    event
                } else {
                    "30405-07.html"
                }
                .to_owned(),
            ),
            "30497-03.html" => {
                qs.start_quest(ctx);
                Some(event.to_owned())
            }
            "30497-06.html" => {
                qs.exit_quest(ctx, true, true);
                Some(event.to_owned())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.get_quest_state(killer, false).is_some() {
            ctx.give_items(killer, SPIDER_SKIN, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            EDMOND => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30497-02.htm"
                    } else {
                        "30497-01.html"
                    }
                    .to_owned();
                }
                STARTED => {
                    if ctx.has_quest_items(player, SPIDER_SKIN) {
                        let skins = ctx.get_quest_items_count(player, SPIDER_SKIN);
                        let bonus = if skins >= SKIN_COUNT { SKIN_BONUS } else { 0 };
                        ctx.give_adena(player, skins * SKIN_REWARD + bonus, true);
                        ctx.take_items(player, SPIDER_SKIN, -1);
                        htmltext = "30497-05.html".to_owned();
                    } else {
                        htmltext = "30497-04.html".to_owned();
                    }
                }
                _ => {}
            },
            MARIUS => {
                htmltext = if ctx.get_quest_items_count(player, SPIDER_SKIN) >= SKIN_COUNT {
                    "30405-02.html"
                } else {
                    "30405-01.html"
                }
                .to_owned();
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

    const NAME: &str = "Q00259_RequestFromTheFarmOwner";

    const NPCS: &str = r#"
        <npc id="30497" type="Folk" level="70" name="Edmond"/>
        <npc id="30405" type="Folk" level="70" name="Marius"/>
        <npc id="20103" type="Monster" level="15" name="Giant Spider"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1495" type="EtcItem" name="Spider Skin"><set name="is_questitem" val="true"/></item>
        <item id="1061" type="EtcItem" name="Greater Healing Potion"><set name="is_stackable" val="true"/></item>
        <item id="17" type="EtcItem" name="Wooden Arrow"><set name="is_stackable" val="true"/></item>
        <item id="1835" type="EtcItem" name="Soulshot: No Grade"><set name="is_stackable" val="true"/></item>
        <item id="2509" type="EtcItem" name="Spiritshot: No Grade"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00259RequestFromTheFarmOwner);
        let pages = Pages::of(&Q00259RequestFromTheFarmOwner)
            .page("30497-02.htm", &["30497-03.html"])
            .plain(&["30497-01.html", "30497-03.html", "30497-06.html", "30497-07.html"])
            .page("30497-04.html", &["30497-06.html", "30497-07.html"])
            .page("30497-05.html", &["30497-06.html", "30497-07.html"])
            .page(
                "30405-02.html",
                &[
                    "30405-03.html",
                    "30405-04.html",
                    "30405-05.html",
                    "30405-05a.html",
                    "30405-05c.html",
                ],
            )
            .plain(&["30405-01.html", "30405-03.html", "30405-07.html"])
            .page("30405-04.html", &["30405-06.html"])
            .page("30405-05.html", &["30405-06.html"])
            .page("30405-05a.html", &["30405-05b.html", "30405-06.html"])
            .plain(&["30405-05b.html"])
            .page("30405-05c.html", &["30405-05d.html", "30405-06.html"])
            .plain(&["30405-05d.html"])
            .page(
                "30405-06.html",
                &[
                    "30405-03.html",
                    "30405-04.html",
                    "30405-05.html",
                    "30405-05a.html",
                    "30405-05c.html",
                ],
            )
            .npcs(&[EDMOND, MARIUS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let edmond = t.spawn(EDMOND, 20);
        let marius = t.spawn(MARIUS, 40);
        (t, edmond, marius)
    }

    /// Kills `n` spiders of `killer`.
    fn kill_spiders(t: &mut ScriptTest, killer: i32, n: usize) {
        for _ in 0..n {
            let spider = t.spawn(20103, 20);
            t.kill(killer, spider);
        }
    }

    #[test]
    fn a_character_below_level_15_is_sent_away() {
        let (mut t, edmond, _) = setup();
        t.enter(1, 14, vec![]);
        assert!(t.quest_page(1, edmond).contains("30497-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_skins_are_traded_for_a_potion_and_paid_in_adena() {
        let (mut t, edmond, marius) = setup();
        t.enter(1, 15, vec![]);
        assert!(t.quest_page(1, edmond).contains("30497-02.htm"));
        assert!(t.event(1, NAME, "30497-03.html").contains("30497-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Ten skins: Marius pays the potion, the skins are taken.
        kill_spiders(&mut t, 1, 10);
        assert_eq!(t.count(1, SPIDER_SKIN), 10);
        assert!(t.quest_page(1, marius).contains("30405-02.html"));
        assert!(t.event(1, NAME, "30405-04.html").contains("30405-04.html"));
        assert_eq!((t.count(1, 1061), t.count(1, SPIDER_SKIN)), (2, 0));
        assert!(t.take_writes().is_empty());

        // Edmond has no skins left.
        assert!(t.quest_page(1, edmond).contains("30497-04.html"));

        // Three more skins: Edmond pays 3 x 25 adena, and the quest ends (`exitQuest(true)`).
        kill_spiders(&mut t, 1, 3);
        assert!(t.quest_page(1, edmond).contains("30497-05.html"));
        assert_eq!(t.count(1, 57), 75);
        assert_eq!(t.count(1, SPIDER_SKIN), 0);
        assert!(t.event(1, NAME, "30497-06.html").contains("30497-06.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn ten_skins_or_more_bring_the_bonus_and_marius_has_no_trade_below_ten() {
        let (mut t, edmond, marius) = setup();
        t.enter(1, 15, vec![]);
        t.quest_page(1, edmond);
        t.event(1, NAME, "30497-03.html");
        t.take_writes();

        kill_spiders(&mut t, 1, 9);
        // Below ten skins the trade is refused: Marius shows the first page.
        assert!(t.quest_page(1, marius).contains("30405-01.html"));
        assert_eq!(t.count(1, SPIDER_SKIN), 9);

        // Ten skins: the bonus of 250 on top of 10 x 25.
        kill_spiders(&mut t, 1, 1);
        assert!(t.quest_page(1, edmond).contains("30497-05.html"));
        assert_eq!(t.count(1, 57), 10 * 25 + 250);
    }
}
