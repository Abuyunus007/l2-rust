//! Quest 39: Red-Eyed Invaders (Guard Babenco and Captain Bathia at the Talking Island
//! Village; the lizardmen and the giant arane of the Gludio hunting grounds).
// Java: dist/game/data/scripts/quests/Q00039_RedEyedInvaders/Q00039_RedEyedInvaders.java
//
// Pattern: party kills with several drop chances (`docs/design/scripts.md`, "Quest patterns"):
// each monster gives one of the four quest items to the killer, and the member whose state is at
// the right cond moves on when the items it needs are in.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const CAPTAIN_BATHIA: i32 = 30332;
const GUARD_BABENCO: i32 = 30334;

// Monsters
const MALE_LIZARDMAN: i32 = 20919;
const MALE_LIZARDMAN_SCOUT: i32 = 20920;
const MALE_LIZARDMAN_GUARD: i32 = 20921;
const GIANT_ARANE: i32 = 20925;

// Items: `ItemHolder(id, count)` of Java as an id and a count
const LIZ_NECKLACE_A: i32 = 7178;
const LIZ_NECKLACE_A_COUNT: i64 = 100;
const LIZ_NECKLACE_B: i32 = 7179;
const LIZ_NECKLACE_B_COUNT: i64 = 100;
const LIZ_PERFUME: i32 = 7180;
const LIZ_PERFUME_COUNT: i64 = 30;
const LIZ_GEM: i32 = 7181;
const LIZ_GEM_COUNT: i64 = 30;

// Rewards
const GREEN_HIGH_LURE: i32 = 6521;
const GREEN_HIGH_LURE_COUNT: i64 = 60;
const BABYDUCK_ROD: i32 = 6529;
const FISHING_SHOT_NONE: i32 = 6535;
const FISHING_SHOT_NONE_COUNT: i64 = 500;

// Misc
const MIN_LEVEL: i32 = 20;

/// Java `Q00039_RedEyedInvaders`.
pub struct Q00039RedEyedInvaders;

impl Script for Q00039RedEyedInvaders {
    fn name(&self) -> &'static str {
        "Q00039_RedEyedInvaders"
    }

    fn path(&self) -> &'static str {
        "quests/Q00039_RedEyedInvaders"
    }

    fn id(&self) -> i32 {
        39
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GUARD_BABENCO]);
        s.add_talk_id(&[GUARD_BABENCO, CAPTAIN_BATHIA]);
        s.add_kill_id(&[
            MALE_LIZARDMAN_GUARD,
            MALE_LIZARDMAN_SCOUT,
            MALE_LIZARDMAN,
            GIANT_ARANE,
        ]);
        s.register_quest_items(&[LIZ_NECKLACE_A, LIZ_NECKLACE_B, LIZ_PERFUME, LIZ_GEM]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // Java sets `htmltext = null` before the state check: no state, no page.
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "30334-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "30332-02.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30332-05.html" => {
                if qs.is_cond(ctx, 3) {
                    if has_necklaces(ctx, player) {
                        qs.set_cond(ctx, 4, true);
                        ctx.take_items(player, LIZ_NECKLACE_A, LIZ_NECKLACE_A_COUNT);
                        ctx.take_items(player, LIZ_NECKLACE_B, LIZ_NECKLACE_B_COUNT);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("30332-06.html".to_owned());
                    }
                }
            }
            "30332-09.html" => {
                if qs.is_cond(ctx, 5) {
                    if has_perfume_and_gem(ctx, player) {
                        // The perfumes and the gems are not taken here: `exitQuest` removes the
                        // registered quest items.
                        ctx.reward_items(player, GREEN_HIGH_LURE, GREEN_HIGH_LURE_COUNT);
                        ctx.reward_items(player, BABYDUCK_ROD, 1);
                        ctx.reward_items(player, FISHING_SHOT_NONE, FISHING_SHOT_NONE_COUNT);
                        ctx.add_exp_and_sp(player, 62_366, 2783);
                        qs.exit_quest(ctx, false, true);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("30332-10.html".to_owned());
                    }
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
            CAPTAIN_BATHIA => match qs.get_cond(ctx) {
                1 => htmltext = "30332-01.html".to_owned(),
                2 => htmltext = "30332-03.html".to_owned(),
                3 => htmltext = "30332-04.html".to_owned(),
                4 => htmltext = "30332-07.html".to_owned(),
                5 => htmltext = "30332-08.html".to_owned(),
                _ => {}
            },
            GUARD_BABENCO => {
                if qs.is_created(ctx) {
                    htmltext = if ctx.player_level(talker) >= MIN_LEVEL {
                        "30334-01.htm"
                    } else {
                        "30334-02.htm"
                    }
                    .to_owned();
                } else if qs.is_started(ctx) && qs.is_cond(ctx, 1) {
                    htmltext = "30334-04.html".to_owned();
                } else if qs.is_completed(ctx) {
                    htmltext = ctx.get_already_completed_msg(talker);
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        match npc.id {
            MALE_LIZARDMAN => {
                if let Some(qs) = ctx.random_party_member_state(killer, 2, 3, Some(npc))
                    && ctx.give_item_randomly(
                        qs.player(),
                        Some(npc),
                        LIZ_NECKLACE_A,
                        1,
                        1,
                        LIZ_NECKLACE_A_COUNT,
                        0.5,
                        true,
                    )
                    && has_item_count(ctx, qs.player(), LIZ_NECKLACE_B, LIZ_NECKLACE_B_COUNT)
                {
                    qs.set_cond(ctx, 3, false);
                }
            }
            MALE_LIZARDMAN_SCOUT => {
                if ctx.get_random_boolean() {
                    if let Some(qs) = ctx.random_party_member_state(killer, 2, 3, Some(npc))
                        && ctx.give_item_randomly(
                            qs.player(),
                            Some(npc),
                            LIZ_NECKLACE_A,
                            1,
                            1,
                            LIZ_NECKLACE_A_COUNT,
                            0.5,
                            true,
                        )
                        && has_item_count(ctx, qs.player(), LIZ_NECKLACE_B, LIZ_NECKLACE_B_COUNT)
                    {
                        qs.set_cond(ctx, 3, false);
                    }
                } else if let Some(qs) = ctx.random_party_member_state(killer, 4, 3, Some(npc))
                    && ctx.give_item_randomly(
                        qs.player(),
                        Some(npc),
                        LIZ_PERFUME,
                        1,
                        1,
                        LIZ_PERFUME_COUNT,
                        0.25,
                        true,
                    )
                    && has_item_count(ctx, qs.player(), LIZ_GEM, LIZ_GEM_COUNT)
                {
                    qs.set_cond(ctx, 5, false);
                }
            }
            MALE_LIZARDMAN_GUARD => {
                if ctx.get_random_boolean() {
                    if let Some(qs) = ctx.random_party_member_state(killer, 2, 3, Some(npc))
                        && ctx.give_item_randomly(
                            qs.player(),
                            Some(npc),
                            LIZ_NECKLACE_B,
                            1,
                            1,
                            LIZ_NECKLACE_B_COUNT,
                            0.5,
                            true,
                        )
                        && has_item_count(ctx, qs.player(), LIZ_NECKLACE_A, LIZ_NECKLACE_A_COUNT)
                    {
                        qs.set_cond(ctx, 3, false);
                    }
                } else if let Some(qs) = ctx.random_party_member_state(killer, 4, 3, Some(npc))
                    && ctx.give_item_randomly(
                        qs.player(),
                        Some(npc),
                        LIZ_PERFUME,
                        1,
                        1,
                        LIZ_PERFUME_COUNT,
                        0.3,
                        true,
                    )
                    && has_item_count(ctx, qs.player(), LIZ_GEM, LIZ_GEM_COUNT)
                {
                    qs.set_cond(ctx, 5, false);
                }
            }
            GIANT_ARANE => {
                if let Some(qs) = ctx.random_party_member_state(killer, 4, 3, Some(npc))
                    && ctx.give_item_randomly(qs.player(), Some(npc), LIZ_GEM, 1, 1, LIZ_GEM_COUNT, 0.3, true)
                    && has_item_count(ctx, qs.player(), LIZ_PERFUME, LIZ_PERFUME_COUNT)
                {
                    qs.set_cond(ctx, 5, false);
                }
            }
            _ => {}
        }
    }
}

/// Java `hasItem(player, ItemHolder)`: the player has at least the holder's count.
fn has_item_count(ctx: &ScriptCtx<'_>, player: ObjectId, item_id: i32, count: i64) -> bool {
    ctx.get_quest_items_count(player, item_id) >= count
}

/// Java `hasAllItems(player, true, LIZ_NECKLACE_A, LIZ_NECKLACE_B)`.
fn has_necklaces(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    has_item_count(ctx, player, LIZ_NECKLACE_A, LIZ_NECKLACE_A_COUNT)
        && has_item_count(ctx, player, LIZ_NECKLACE_B, LIZ_NECKLACE_B_COUNT)
}

/// Java `hasAllItems(player, true, LIZ_PERFUME, LIZ_GEM)`.
fn has_perfume_and_gem(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    has_item_count(ctx, player, LIZ_PERFUME, LIZ_PERFUME_COUNT)
        && has_item_count(ctx, player, LIZ_GEM, LIZ_GEM_COUNT)
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00039_RedEyedInvaders";

    const NPCS: &str = r#"
        <npc id="30332" type="Folk" level="70" name="Captain Bathia"/>
        <npc id="30334" type="Folk" level="70" name="Guard Babenco"/>
        <npc id="20919" type="Monster" level="18" name="Male Lizardman"/>
        <npc id="20920" type="Monster" level="20" name="Male Lizardman Scout"/>
        <npc id="20921" type="Monster" level="22" name="Male Lizardman Guard"/>
        <npc id="20925" type="Monster" level="30" name="Giant Arane"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="6521" type="EtcItem" name="Green High Lure"><set name="is_stackable" val="true"/></item>
        <item id="6529" type="EtcItem" name="Babyduck Rod"/>
        <item id="6535" type="EtcItem" name="Fishing Shot: None"><set name="is_stackable" val="true"/></item>
        <item id="7178" type="EtcItem" name="Lizardman Necklace A"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7179" type="EtcItem" name="Lizardman Necklace B"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7180" type="EtcItem" name="Lizardman Perfume"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7181" type="EtcItem" name="Lizardman Gem"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        babenco: Oid,
        bathia: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00039RedEyedInvaders);
        let pages = Pages::of(&Q00039RedEyedInvaders)
            .page("30334-01.htm", &["30334-03.htm"])
            .page("30332-01.html", &["30332-02.html"])
            .page("30332-04.html", &["30332-05.html"])
            .page("30332-08.html", &["30332-09.html"])
            .plain(&[
                "30334-02.htm",
                "30334-03.htm",
                "30334-04.html",
                "30332-02.html",
                "30332-03.html",
                "30332-05.html",
                "30332-06.html",
                "30332-07.html",
                "30332-09.html",
                "30332-10.html",
            ])
            .npcs(&[GUARD_BABENCO, CAPTAIN_BATHIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            babenco: t.spawn(GUARD_BABENCO, 20),
            bathia: t.spawn(CAPTAIN_BATHIA, 40),
        };
        (t, npcs)
    }

    /// Sets the player's cond directly (for the branches reached through a long hunt).
    fn set_cond(t: &mut ScriptTest, cond: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_cond(ctx, cond, false);
            }
        });
        t.take_writes();
    }

    /// Calls a script event directly (for the refusals whose page is not linked yet).
    fn direct(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00039RedEyedInvaders.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    /// Kills `npc_id` until `done` holds; returns the number of kills (at most 500).
    fn hunt(t: &mut ScriptTest, npc_id: i32, done: impl Fn(&ScriptTest) -> bool) -> usize {
        for kills in 1..500 {
            let mob = t.spawn(npc_id, 80);
            t.kill(1, mob);
            if done(&*t) {
                return kills;
            }
        }
        panic!("the goal of {NAME} was not reached in 500 kills of {npc_id}");
    }

    #[test]
    fn a_level_19_character_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 19, vec![]);
        assert!(t.quest_page(1, npcs.babenco).contains("30334-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_necklaces_the_perfumes_and_the_gems_then_the_reward() {
        let (mut t, npcs) = setup();
        // The necklace B and most of the A are already in the bag, the perfumes too.
        t.enter(
            1,
            20,
            vec![
                item(1001, LIZ_NECKLACE_A, 99),
                item(1002, LIZ_NECKLACE_B, 100),
                item(1003, LIZ_PERFUME, 30),
                item(1004, LIZ_GEM, 29),
            ],
        );
        assert!(t.quest_page(1, npcs.babenco).contains("30334-01.htm"));
        assert!(t.event(1, NAME, "30334-03.htm").contains("30334-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.babenco).contains("30334-04.html"));

        // Bathia sends the player out: cond 2.
        assert!(t.quest_page(1, npcs.bathia).contains("30332-01.html"));
        assert!(t.event(1, NAME, "30332-02.html").contains("30332-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The male lizardmen give the necklace A; the hundredth sets cond 3 (B is in).
        hunt(&mut t, MALE_LIZARDMAN, |t| {
            t.count(1, LIZ_NECKLACE_A) >= LIZ_NECKLACE_A_COUNT
        });
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Bathia takes both necklaces: cond 4.
        assert!(t.quest_page(1, npcs.bathia).contains("30332-04.html"));
        assert!(t.event(1, NAME, "30332-05.html").contains("30332-05.html"));
        assert_eq!((t.count(1, LIZ_NECKLACE_A), t.count(1, LIZ_NECKLACE_B)), (0, 0));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        assert!(t.quest_page(1, npcs.bathia).contains("30332-07.html"));

        // The giant aranes give the gems: 30 gems and 30 perfumes set cond 5.
        hunt(&mut t, GIANT_ARANE, |t| t.count(1, LIZ_GEM) >= LIZ_GEM_COUNT);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // Bathia gives the reward; `exitQuest` takes the perfumes and the gems with it.
        assert!(t.quest_page(1, npcs.bathia).contains("30332-08.html"));
        assert!(t.event(1, NAME, "30332-09.html").contains("30332-09.html"));
        assert_eq!(t.count(1, GREEN_HIGH_LURE), GREEN_HIGH_LURE_COUNT);
        assert_eq!(t.count(1, BABYDUCK_ROD), 1);
        assert_eq!(t.count(1, FISHING_SHOT_NONE), FISHING_SHOT_NONE_COUNT);
        assert_eq!((t.count(1, LIZ_PERFUME), t.count(1, LIZ_GEM)), (0, 0));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.babenco)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn missing_necklaces_or_perfumes_are_refused() {
        let (mut t, npcs) = setup();
        t.enter(1, 20, vec![item(1001, LIZ_NECKLACE_A, 100)]);
        t.quest_page(1, npcs.babenco);
        t.event(1, NAME, "30334-03.htm");
        t.take_writes();

        // Bathia at cond 3 without the second necklace: the page for it, nothing taken.
        set_cond(&mut t, 3);
        assert_eq!(direct(&mut t, "30332-05.html").as_deref(), Some("30332-06.html"));
        assert_eq!(t.count(1, LIZ_NECKLACE_A), 100);
        assert!(t.take_writes().is_empty());

        // At cond 5 without the perfumes and gems: the page for it, no reward.
        set_cond(&mut t, 5);
        assert_eq!(direct(&mut t, "30332-09.html").as_deref(), Some("30332-10.html"));
        assert_eq!(t.count(1, GREEN_HIGH_LURE), 0);
        assert!(t.take_writes().is_empty());
    }
}
