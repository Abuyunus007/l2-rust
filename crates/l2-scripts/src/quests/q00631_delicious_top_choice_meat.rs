//! Quest 631: Delicious Top Choice Meat (Tunatun in the Hot Springs area; the full grown
//! kookaburras, cougars, buffaloes and grendels drop the prime meat, 120 of them buy one of the
//! icarus recipes, pieces or spice crates).
// Java: dist/game/data/scripts/quests/Q00631_DeliciousTopChoiceMeat/Q00631_DeliciousTopChoiceMeat.java
//
// Pattern: a kill that sets cond 2 at a limit (120 prime meat, `giveItemRandomly` with its limit),
// a random reward from a switch over `getRandom(10)` (the RNG order matters: the switch first, then
// the entries and the counts), and the quit that ends the quest whatever the roll.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const TUNATUN: i32 = 31537;

// Items
const TOP_QUALITY_MEAT: i32 = 7546;
const PRIME_MEAT: i32 = 15534;

// Misc
const MIN_LEVEL: i32 = 82;
const PRIME_MEAT_COUNT: i64 = 120;

// Rewards
const RECIPE: [i32; 9] = [10373, 10374, 10375, 10376, 10377, 10378, 10379, 10380, 10381];
const PIECE: [i32; 9] = [10397, 10398, 10399, 10400, 10401, 10402, 10403, 10404, 10405];
const GOLDEN_SPICE_CRATE: i32 = 15482;
const CRYSTAL_SPICE_COMPRESSED_PACK: i32 = 15483;

/// Java `MOBS_MEAT`: the prime meat drop chance (a fraction) of each monster.
const MOBS_MEAT: [(i32, f64); 8] = [
    (18878, 0.172), // Full Grown Kookaburra
    (18879, 0.334), // Full Grown Kookaburra
    (18885, 0.172), // Full Grown Cougar
    (18886, 0.334), // Full Grown Cougar
    (18892, 0.182), // Full Grown Buffalo
    (18893, 0.349), // Full Grown Buffalo
    (18899, 0.182), // Full Grown Grendel
    (18900, 0.349), // Full Grown Grendel
];

/// Java `Q00631_DeliciousTopChoiceMeat`.
pub struct Q00631DeliciousTopChoiceMeat;

impl Script for Q00631DeliciousTopChoiceMeat {
    fn name(&self) -> &'static str {
        "Q00631_DeliciousTopChoiceMeat"
    }

    fn path(&self) -> &'static str {
        "quests/Q00631_DeliciousTopChoiceMeat"
    }

    fn id(&self) -> i32 {
        631
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TUNATUN]);
        s.add_talk_id(&[TUNATUN]);
        let mobs: Vec<i32> = MOBS_MEAT.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[TOP_QUALITY_MEAT, PRIME_MEAT]);
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
            "quest_accept" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                    htmltext = Some("31537-02.html".to_owned());
                } else {
                    htmltext = Some("31537-03.html".to_owned());
                }
            }
            "31537-06.html" => {
                if qs.is_cond(ctx, 2) && ctx.get_quest_items_count(player, PRIME_MEAT) >= PRIME_MEAT_COUNT {
                    match ctx.get_random(10) {
                        0 => {
                            let recipe = ctx.get_random_entry(&RECIPE).unwrap_or(RECIPE[0]);
                            ctx.reward_items(player, recipe, 1);
                        }
                        1 => {
                            let piece = ctx.get_random_entry(&PIECE).unwrap_or(PIECE[0]);
                            ctx.reward_items(player, piece, 1);
                        }
                        2 => {
                            let piece = ctx.get_random_entry(&PIECE).unwrap_or(PIECE[0]);
                            ctx.reward_items(player, piece, 2);
                        }
                        3 => {
                            let piece = ctx.get_random_entry(&PIECE).unwrap_or(PIECE[0]);
                            ctx.reward_items(player, piece, 3);
                        }
                        4 => {
                            let piece = ctx.get_random_entry(&PIECE).unwrap_or(PIECE[0]);
                            let count = ctx.get_random(5) + 2;
                            ctx.reward_items(player, piece, i64::from(count));
                        }
                        5 => {
                            let piece = ctx.get_random_entry(&PIECE).unwrap_or(PIECE[0]);
                            let count = ctx.get_random(7) + 2;
                            ctx.reward_items(player, piece, i64::from(count));
                        }
                        6 => ctx.reward_items(player, GOLDEN_SPICE_CRATE, 1),
                        7 => ctx.reward_items(player, GOLDEN_SPICE_CRATE, 2),
                        8 => ctx.reward_items(player, CRYSTAL_SPICE_COMPRESSED_PACK, 1),
                        9 => ctx.reward_items(player, CRYSTAL_SPICE_COMPRESSED_PACK, 2),
                        _ => {}
                    }
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMemberState(player, 1, 3, npc)`: a member at cond 1.
        let Some(qs) = ctx.random_party_member_state(killer, 1, 3, Some(npc)) else {
            return;
        };
        let Some(&(_, chance)) = MOBS_MEAT.iter().find(|(id, _)| *id == npc.id) else {
            return;
        };
        if ctx.give_item_randomly(
            qs.player(),
            Some(npc),
            PRIME_MEAT,
            1,
            1,
            PRIME_MEAT_COUNT,
            chance,
            true,
        ) {
            qs.set_cond(ctx, 2, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = "31537-01.htm".to_owned();
        } else if state == STARTED {
            if qs.is_cond(ctx, 1) {
                // NOTE(java-quirk): at cond 1 with the meat already at 120 Java shows the default
                // "no quest" page here.
                if ctx.get_quest_items_count(player, PRIME_MEAT) < PRIME_MEAT_COUNT {
                    htmltext = "31537-04.html".to_owned();
                }
            } else if qs.is_cond(ctx, 2) && ctx.get_quest_items_count(player, PRIME_MEAT) >= PRIME_MEAT_COUNT
            {
                htmltext = "31537-05.html".to_owned();
            }
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

    const NAME: &str = "Q00631_DeliciousTopChoiceMeat";

    const NPCS: &str = r#"
        <npc id="31537" type="Folk" level="70" name="Tunatun"/>
        <npc id="18878" type="Monster" level="70" name="Full Grown Kookaburra"/>
        <npc id="18879" type="Monster" level="70" name="Full Grown Kookaburra"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7546" type="EtcItem" name="Top Quality Meat"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="15534" type="EtcItem" name="Prime Meat"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="15482" type="EtcItem" name="Golden Spice Crate"><set name="is_stackable" val="true"/></item>
        <item id="15483" type="EtcItem" name="Crystal Spice Compressed Pack"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00631DeliciousTopChoiceMeat);
        let pages = Pages::of(&Q00631DeliciousTopChoiceMeat)
            .page("31537-01.htm", &["quest_accept"])
            .plain(&["31537-02.html", "31537-03.html", "31537-04.html"])
            .page("31537-05.html", &["31537-06.html"])
            .plain(&["31537-06.html"])
            .npcs(&[TUNATUN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let tunatun = t.spawn(TUNATUN, 20);
        (t, tunatun)
    }

    #[test]
    fn a_character_below_level_82_is_sent_away_at_the_accept() {
        let (mut t, tunatun) = setup();
        t.enter(1, 81, vec![]);
        assert!(t.quest_page(1, tunatun).contains("31537-01.htm"));
        assert!(t.event(1, NAME, "quest_accept").contains("31537-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn one_hundred_twenty_prime_meats_end_the_hunt_and_the_reward_ends_the_quest() {
        let (mut t, tunatun) = setup();
        t.enter(1, 82, vec![]);
        assert!(t.quest_page(1, tunatun).contains("31537-01.htm"));
        assert!(t.event(1, NAME, "quest_accept").contains("31537-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, tunatun).contains("31537-04.html"));

        // 119 by the script; a full grown kookaburra (33.4 %) makes the 120th and cond 2.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), PRIME_MEAT, 119);
        });
        t.take();
        let mut kills = 0;
        while t.count(1, PRIME_MEAT) < PRIME_MEAT_COUNT {
            let mob = t.spawn(18879, 40);
            t.kill(1, mob);
            kills += 1;
            assert!(kills < 100, "no prime meat dropped");
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The reward: one of the rewards, then the quest ends and the prime meat goes.
        assert!(t.quest_page(1, tunatun).contains("31537-05.html"));
        assert!(t.event(1, NAME, "31537-06.html").contains("31537-06.html"));
        assert_eq!(t.count(1, PRIME_MEAT), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
