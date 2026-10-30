//! Quest 381: Let's Become a Royal Member (Sorint and Sandra in the Town of Aden, the coin collecting club).
// Java: dist/game/data/scripts/quests/Q00381_LetsBecomeARoyalMember/Q00381_LetsBecomeARoyalMember.java
//
// Pattern: a collection quest over two NPCs (`docs/design/scripts.md`, "Quest patterns"): the memo
// state marks the stage (1 = to be introduced to Sandra, 2 = collecting the four-leaf coin for
// the album). The Ancient Gargoyle drops Kail's coin (5%, one); Sorint trades the coin and the
// album for the Royal Membership and the quest ends (not repeatable). The album is not a
// registered quest item of this script, so it stays after the quest like in Java.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SANDRA: i32 = 30090;
const SORINT: i32 = 30232;

// Items
const COLLECTOR_MEMBERSHIP_1: i32 = 3813;
const KAILS_COIN: i32 = 5899;
const FOUR_LEAF_COIN: i32 = 7569;
const COIN_ALBUM: i32 = 5900;

// Monsters
const ANCIENT_GARGOYLE: i32 = 21018;
const FALLEN_CHIEF_VERGUS: i32 = 27316;

// Reward
const ROYAL_MEMBERSHIP: i32 = 5898;

// Misc
const MIN_LEVEL: i32 = 55;

/// Java `Q00381_LetsBecomeARoyalMember`.
pub struct Q00381LetsBecomeARoyalMember;

impl Script for Q00381LetsBecomeARoyalMember {
    fn name(&self) -> &'static str {
        "Q00381_LetsBecomeARoyalMember"
    }

    fn path(&self) -> &'static str {
        "quests/Q00381_LetsBecomeARoyalMember"
    }

    fn id(&self) -> i32 {
        381
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SORINT]);
        s.add_talk_id(&[SORINT, SANDRA]);
        s.add_kill_id(&[ANCIENT_GARGOYLE, FALLEN_CHIEF_VERGUS]);
        s.register_quest_items(&[KAILS_COIN, FOUR_LEAF_COIN]);
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
            "30232-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30090-02.html" => {
                if qs.is_memo_state(ctx, 1) && !ctx.has_quest_items(player, COIN_ALBUM) {
                    qs.set_memo_state(ctx, 2);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match npc.id {
            SORINT => {
                if qs.is_created(ctx) {
                    if ctx.player_level(talker) < MIN_LEVEL
                        || !ctx.has_quest_items(talker, COLLECTOR_MEMBERSHIP_1)
                    {
                        htmltext = "30232-02.html".to_owned();
                    } else if !ctx.has_quest_items(talker, ROYAL_MEMBERSHIP) {
                        htmltext = "30232-01.htm".to_owned();
                    }
                    // NOTE(java-quirk): with neither branch true the quest is not in the quest list
                    // (Java's TODO); the page stays the "no quest" one.
                } else if qs.is_started(ctx) {
                    let has_album = ctx.has_quest_items(talker, COIN_ALBUM);
                    let has_coin = ctx.has_quest_items(talker, KAILS_COIN);
                    if has_album && has_coin {
                        ctx.take_items_many(talker, 1, &[KAILS_COIN, COIN_ALBUM]);
                        ctx.give_items(talker, ROYAL_MEMBERSHIP, 1);
                        qs.exit_quest(ctx, false, true);
                        htmltext = "30232-06.html".to_owned();
                    } else {
                        htmltext = if has_album || has_coin {
                            "30232-05.html"
                        } else {
                            "30232-04.html"
                        }
                        .to_owned();
                    }
                } else {
                    htmltext = ctx.get_already_completed_msg(talker);
                }
            }
            SANDRA => match qs.get_memo_state(ctx) {
                1 => htmltext = "30090-01.html".to_owned(),
                2 => {
                    if ctx.has_quest_items(talker, COIN_ALBUM) {
                        htmltext = "30090-05.html".to_owned();
                    } else if ctx.has_quest_items(talker, FOUR_LEAF_COIN) {
                        ctx.take_items(talker, FOUR_LEAF_COIN, 1);
                        ctx.give_items(talker, COIN_ALBUM, 1);
                        ctx.play_sound(talker, QuestSound::ItemsoundQuestMiddle);
                        htmltext = "30090-04.html".to_owned();
                    } else {
                        htmltext = "30090-03.html".to_owned();
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            if npc.id == ANCIENT_GARGOYLE {
                ctx.give_item_randomly(killer, Some(npc), KAILS_COIN, 1, 1, 1, 0.05, true);
            } else if qs.is_memo_state(ctx, 2) && !ctx.has_quest_items(killer, FOUR_LEAF_COIN) {
                ctx.give_items(killer, FOUR_LEAF_COIN, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00381_LetsBecomeARoyalMember";

    const NPCS: &str = r#"
        <npc id="30232" type="Folk" level="70" name="Sorint"/>
        <npc id="30090" type="Folk" level="70" name="Sandra"/>
        <npc id="21018" type="Monster" level="60" name="Ancient Gargoyle"/>
        <npc id="27316" type="Monster" level="60" name="Fallen Chief Vergus"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="3813" type="EtcItem" name="Collector Membership I"/>
        <item id="5898" type="EtcItem" name="Royal Membership"/>
        <item id="5899" type="EtcItem" name="Kail's Coin"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5900" type="EtcItem" name="Coin Album"><set name="is_stackable" val="true"/></item>
        <item id="7569" type="EtcItem" name="Four-Leaf Coin"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00381LetsBecomeARoyalMember);
        let pages = Pages::of(&Q00381LetsBecomeARoyalMember)
            .page("30232-01.htm", &["30232-03.htm"])
            .plain(&[
                "30232-02.html",
                "30232-03.htm",
                "30232-04.html",
                "30232-05.html",
                "30232-06.html",
            ])
            .page("30090-01.html", &["30090-02.html"])
            .plain(&["30090-02.html", "30090-03.html", "30090-04.html", "30090-05.html"])
            .npcs(&[SORINT, SANDRA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let sorint = t.spawn(SORINT, 20);
        let sandra = t.spawn(SANDRA, 40);
        (t, sorint, sandra)
    }

    #[test]
    fn the_membership_and_the_level_are_needed_to_start() {
        let (mut t, sorint, _) = setup();
        t.enter(1, 54, vec![item(100, COLLECTOR_MEMBERSHIP_1, 1)]);
        assert!(t.quest_page(1, sorint).contains("30232-02.html"));
        t.enter(2, 55, vec![]);
        assert!(t.quest_page(2, sorint).contains("30232-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_album_and_the_coin_for_the_royal_membership() {
        let (mut t, sorint, sandra) = setup();
        t.enter(1, 55, vec![item(100, COLLECTOR_MEMBERSHIP_1, 1)]);
        assert!(t.quest_page(1, sorint).contains("30232-01.htm"));
        assert!(t.event(1, NAME, "30232-03.htm").contains("30232-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );

        // Sandra introduces herself: memo 2.
        assert!(t.quest_page(1, sandra).contains("30090-01.html"));
        assert!(t.event(1, NAME, "30090-02.html").contains("30090-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "2")]);

        // A Fallen Chief Vergus gives the four-leaf coin; Sandra makes the album of it.
        let vergus = t.spawn(FALLEN_CHIEF_VERGUS, 60);
        t.kill(1, vergus);
        assert_eq!(t.count(1, FOUR_LEAF_COIN), 1);
        assert!(t.quest_page(1, sandra).contains("30090-04.html"));
        assert_eq!((t.count(1, FOUR_LEAF_COIN), t.count(1, COIN_ALBUM)), (0, 1));
        assert!(t.take_writes().is_empty());

        // The album alone: Sorint wants the coin too.
        assert!(t.quest_page(1, sorint).contains("30232-05.html"));

        // Kail's coin from an Ancient Gargoyle (5%, one).
        for _ in 0..400 {
            if t.count(1, KAILS_COIN) >= 1 {
                break;
            }
            let gargoyle = t.spawn(ANCIENT_GARGOYLE, 60);
            t.kill(1, gargoyle);
        }
        assert_eq!(t.count(1, KAILS_COIN), 1);
        assert!(t.take_writes().is_empty());

        // Both: the membership, the album and the coin go, the quest ends (not repeatable).
        assert!(t.quest_page(1, sorint).contains("30232-06.html"));
        assert_eq!(t.count(1, ROYAL_MEMBERSHIP), 1);
        assert_eq!((t.count(1, KAILS_COIN), t.count(1, COIN_ALBUM)), (0, 0));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(t.quest_page(1, sorint).contains("already been completed"));
    }
}
