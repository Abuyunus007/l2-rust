//! Quest 640: The Zero Hour (Kahman in the Gludio area; the spiked stakato monsters drop fangs,
//! 6 to 150 of them buy one of nine crafting materials).
// Java: dist/game/data/scripts/quests/Q00640_TheZeroHour/Q00640_TheZeroHour.java
//
// Pattern: a repeatable kill that drops one fang to a random started party member
// (`getRandomPartyMemberState(killer, State.STARTED)`, the state overload), a menu of rewards
// (events "1".."9") that each take a fixed number of fangs, and a start gate of Q00109.
// NOTE(java-quirk): the start and the quit events answer null (no page), as Java returns nothing.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KAHMAN: i32 = 31554;

/// Java `MONSTERS_TO_HUNT`.
const MONSTERS_TO_HUNT: [i32; 15] = [
    22617, // Spiked Stakato
    22618, // Spiked Stakato Worker
    22619, // Spiked Stakato Guard
    22620, // Female Spiked Stakato
    22621, // Male Spiked Stakato
    22622, // Male Spiked Stakato
    22623, // Spiked Stakato Sorcerer
    22625, // Cannibalistic Stakato Leader
    22626, // Cannibalistic Stakato Leader
    22627, // Spiked Stakato Soldier
    22628, // Spiked Stakato Drone
    22629, // Spiked Stakato Captain
    22630, // Spike Stakato Nurse
    22631, // Spike Stakato Nurse
    22633, // Spiked Stakato Shaman
];

// Item
const FANG_OF_STAKATO: i32 = 8085;

// Misc
const MIN_LEVEL: i32 = 66;

/// Java `Q00109_InSearchOfTheNest.class.getSimpleName()`.
const IN_SEARCH_OF_THE_NEST: &str = "Q00109_InSearchOfTheNest";

/// Java `Q00640_TheZeroHour`.
pub struct Q00640TheZeroHour;

/// Java `onEvent` "1".."9": the fangs taken, the item given and its count.
fn reward_of(event: &str) -> Option<(i64, i32, i64)> {
    Some(match event {
        "1" => (12, 4042, 1),   // Enria
        "2" => (6, 4043, 1),    // Asofe
        "3" => (6, 4044, 1),    // Thons
        "4" => (81, 1887, 10),  // Varnish of Purity
        "5" => (33, 1888, 5),   // Synthetic Cokes
        "6" => (30, 1889, 10),  // Compound Braid
        "7" => (150, 5550, 10), // Durable Metal Plate
        "8" => (131, 1890, 10), // Mithril Alloy
        "9" => (123, 1893, 5),  // Oriharukon
        _ => return None,
    })
}

impl Script for Q00640TheZeroHour {
    fn name(&self) -> &'static str {
        "Q00640_TheZeroHour"
    }

    fn path(&self) -> &'static str {
        "quests/Q00640_TheZeroHour"
    }

    fn id(&self) -> i32 {
        640
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KAHMAN]);
        s.add_talk_id(&[KAHMAN]);
        s.add_kill_id(&MONSTERS_TO_HUNT);
        s.register_quest_items(&[FANG_OF_STAKATO]);
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
            "31554-02.htm" => qs.start_quest(ctx),
            "31554-05.htm" => htmltext = Some("31554-05.htm".to_owned()),
            "31554-08.htm" => qs.exit_quest(ctx, true, true),
            _ => {
                if let Some((fangs, item, count)) = reward_of(event) {
                    if ctx.get_quest_items_count(player, FANG_OF_STAKATO) >= fangs {
                        ctx.take_items(player, FANG_OF_STAKATO, fangs);
                        ctx.reward_items(player, item, count);
                        htmltext = Some("31554-09.htm".to_owned());
                    } else {
                        htmltext = Some("31554-07.htm".to_owned());
                    }
                }
            }
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMemberState(killer, State.STARTED)`.
        let Some(member) = ctx.get_random_party_member_state(killer, STARTED) else {
            return;
        };
        // NOTE(java-quirk): the member needs a quest state of its own (any state).
        if ctx.get_quest_state(member, false).is_none() {
            return;
        }
        ctx.give_items(member, FANG_OF_STAKATO, 1);
        ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                let nest_done = ctx
                    .player_quest_state(player, IN_SEARCH_OF_THE_NEST)
                    .is_some_and(|q| q.state() == COMPLETED);
                if nest_done {
                    "31554-01.htm".to_owned()
                } else {
                    "31554-10.htm".to_owned()
                }
            } else {
                "31554-00.htm".to_owned()
            };
        } else if state == STARTED && qs.is_cond(ctx, 1) {
            htmltext = if ctx.get_quest_items_count(player, FANG_OF_STAKATO) >= 1 {
                "31554-04.htm".to_owned()
            } else {
                "31554-03.htm".to_owned()
            };
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::COMPLETED;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00640_TheZeroHour";

    const NPCS: &str = r#"
        <npc id="31554" type="Folk" level="70" name="Kahman"/>
        <npc id="22617" type="Monster" level="60" name="Spiked Stakato"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8085" type="EtcItem" name="Fang of Stakato"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="4042" type="EtcItem" name="Enria"><set name="is_stackable" val="true"/></item>"#;

    /// Java `Q00109_InSearchOfTheNest`, as far as this quest reads it.
    struct InSearchOfTheNest;

    impl Script for InSearchOfTheNest {
        fn name(&self) -> &'static str {
            IN_SEARCH_OF_THE_NEST
        }
        fn path(&self) -> &'static str {
            "quests/Q00109_InSearchOfTheNest"
        }
        fn id(&self) -> i32 {
            109
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(InSearchOfTheNest);
        registry.add(Q00640TheZeroHour);
        let pages = Pages::of(&Q00640TheZeroHour)
            .page("31554-01.htm", &["31554-02.htm"])
            .plain(&[
                "31554-00.htm",
                "31554-02.htm",
                "31554-03.htm",
                "31554-07.htm",
                "31554-10.htm",
            ])
            .page("31554-04.htm", &["31554-05.htm"])
            .page("31554-05.htm", &["1", "2", "3", "4", "5", "6", "7", "8", "9"])
            .page("31554-09.htm", &["31554-05.htm", "31554-07.htm", "31554-08.htm"])
            .plain(&["31554-08.htm"])
            .npcs(&[KAHMAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kahman = t.spawn(KAHMAN, 20);
        (t, kahman)
    }

    #[test]
    fn the_start_needs_level_66_and_the_nest_quest() {
        let (mut t, kahman) = setup();
        t.enter(1, 65, vec![]);
        assert!(t.quest_page(1, kahman).contains("31554-00.htm"));
        t.enter(2, 66, vec![]);
        assert!(t.quest_page(2, kahman).contains("31554-10.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn twelve_fangs_buy_enria_and_the_quest_ends() {
        let (mut t, kahman) = setup();
        t.enter(1, 66, vec![]);
        t.world.run_as_script(IN_SEARCH_OF_THE_NEST, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
        assert!(t.quest_page(1, kahman).contains("31554-01.htm"));
        // The start returns no page; the quest is started.
        t.event(1, NAME, "31554-02.htm");
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, kahman).contains("31554-03.htm"));

        // Fangs from the kills, one each.
        let mut kills = 0;
        while t.count(1, FANG_OF_STAKATO) < 12 {
            let mob = t.spawn(22617, 40);
            t.kill(1, mob);
            kills += 1;
            assert!(kills < 100, "no fang dropped");
        }
        assert!(t.take_writes().is_empty());

        // Talk, the menu, the first reward.
        assert!(t.quest_page(1, kahman).contains("31554-04.htm"));
        assert!(t.event(1, NAME, "31554-05.htm").contains("31554-05.htm"));
        assert!(t.event(1, NAME, "1").contains("31554-09.htm"));
        assert_eq!(t.count(1, FANG_OF_STAKATO), 0);
        assert_eq!(t.count(1, 4042), 1);

        // Leave from the reward page: the quest ends.
        assert!(t.event(1, NAME, "31554-08.htm").is_empty());
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
