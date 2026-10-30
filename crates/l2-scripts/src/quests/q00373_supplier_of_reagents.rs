//! Quest 373: Supplier of Reagents (Wesley, the alchemist's mixing urn).
// Java: dist/game/data/scripts/quests/Q00373_SupplierOfReagents/Q00373_SupplierOfReagents.java
//
// Pattern: a quest that keeps a mixing recipe in the memo state: an ingredient page adds 11..24,
// a catalyst page adds 1100..1600, and `mixitems` looks the sum up in a reward table.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const WESLEY: i32 = 30166;
const ALCHEMIST_MIXING_URN: i32 = 31149;

// Mobs
const HALLATE_MAID: i32 = 20822;
const HALLATE_GUARDIAN: i32 = 21061;
const HAMES_ORC_SHAMAN: i32 = 21115;
const LAVA_WYRM: i32 = 21111;
const CRENDION: i32 = 20813;
const PLATINUM_TRIBE_SHAMAN: i32 = 20828;
const PLATINUM_GUARDIAN_SHAMAN: i32 = 21066;

// Items
const REAGENT_POUNCH1: i32 = 6007;
const REAGENT_POUNCH2: i32 = 6008;
const REAGENT_POUNCH3: i32 = 6009;
const REAGENT_BOX: i32 = 6010;
const WYRM_BLOOD: i32 = 6011;
const LAVA_STONE: i32 = 6012;
const MOONSTONE_SHARD: i32 = 6013;
const ROTTEN_BONE: i32 = 6014;
const DEMONS_BLOOD: i32 = 6015;
const INFERNIUM_ORE: i32 = 6016;
const BLOOD_ROOT: i32 = 6017;
const VOLCANIC_ASH: i32 = 6018;
const QUICKSILVER: i32 = 6019;
const SULFUR: i32 = 6020;
const DRACOPLASM: i32 = 6021;
const MAGMA_DUST: i32 = 6022;
const MOON_DUST: i32 = 6023;
const NECROPLASM: i32 = 6024;
const DEMONPLASM: i32 = 6025;
const INFERNO_DUST: i32 = 6026;
const DRACONIC_ESSENCE: i32 = 6027;
const FIRE_ESSENCE: i32 = 6028;
const LUNARGENT: i32 = 6029;
const MIDNIGHT_OIL: i32 = 6030;
const DEMONIC_ESSENCE: i32 = 6031;
const ABYSS_OIL: i32 = 6032;
const HELLFIRE_OIL: i32 = 6033;
const NIGHTMARE_OIL: i32 = 6034;
const PURE_SILVER: i32 = 6320;
const MIXING_MANUAL: i32 = 6317;
const WESLEYS_MIXING_STONE: i32 = 5904;

// Misc
const MIN_LEVEL: i32 = 57;

/// Java `HTML_TO_MEMO_STATE`: the page of the ingredient or catalyst and what it adds to the memo.
fn html_to_memo_state(html: &str) -> Option<i32> {
    Some(match html {
        // List of ingredients to mix
        "31149-03.html" => 11,
        "31149-04.html" => 12,
        "31149-05.html" => 13,
        "31149-06.html" => 14,
        "31149-07.html" => 15,
        "31149-08.html" => 16,
        "31149-09.html" => 17,
        "31149-10.html" => 18,
        "31149-11.html" => 19,
        "31149-12.html" => 20,
        "31149-13.html" => 21,
        "31149-14.html" => 22,
        "31149-15.html" => 23,
        "31149-16.html" => 24,
        // List of catalysts to mix
        "31149-19.html" => 1100,
        "31149-20.html" => 1200,
        "31149-21.html" => 1300,
        "31149-22.html" => 1400,
        "31149-23.html" => 1500,
        "31149-24.html" => 1600,
        _ => return None,
    })
}

/// Java `MEMO_STATE_TO_ITEM`: the item and count a memo part stands for.
fn memo_state_to_item(memo: i32) -> Option<(i32, i64)> {
    Some(match memo {
        11 => (WYRM_BLOOD, 10),
        12 => (LAVA_STONE, 10),
        13 => (MOONSTONE_SHARD, 10),
        14 => (ROTTEN_BONE, 10),
        15 => (DEMONS_BLOOD, 10),
        16 => (INFERNIUM_ORE, 10),
        17 => (DRACOPLASM, 10),
        18 => (MAGMA_DUST, 10),
        19 => (MOON_DUST, 10),
        20 => (NECROPLASM, 10),
        21 => (DEMONPLASM, 10),
        22 => (INFERNO_DUST, 10),
        23 => (FIRE_ESSENCE, 1),
        24 => (LUNARGENT, 1),
        1100 => (BLOOD_ROOT, 1),
        1200 => (VOLCANIC_ASH, 1),
        1300 => (QUICKSILVER, 1),
        1400 => (SULFUR, 1),
        1500 => (DEMONIC_ESSENCE, 1),
        1600 => (MIDNIGHT_OIL, 1),
        _ => return None,
    })
}

/// Java `MEMO_STATE_TO_REWARD`: the sum of ingredient and catalyst gives an item and a page.
fn memo_state_to_reward(memo: i32) -> Option<(i32, &'static str)> {
    Some(match memo {
        1111 => (DRACOPLASM, "31149-30.html"),
        1212 => (MAGMA_DUST, "31149-31.html"),
        1213 => (MOON_DUST, "31149-32.html"),
        1114 => (NECROPLASM, "31149-33.html"),
        1115 => (DEMONPLASM, "31149-34.html"),
        1216 => (INFERNO_DUST, "31149-35.html"),
        1317 => (DRACONIC_ESSENCE, "31149-36.html"),
        1418 => (FIRE_ESSENCE, "31149-37.html"),
        1319 => (LUNARGENT, "31149-38.html"),
        1320 => (MIDNIGHT_OIL, "31149-39.html"),
        1421 => (DEMONIC_ESSENCE, "31149-40.html"),
        1422 => (ABYSS_OIL, "31149-41.html"),
        1523 => (HELLFIRE_OIL, "31149-42.html"),
        1624 => (NIGHTMARE_OIL, "31149-43.html"),
        1324 => (PURE_SILVER, "31149-46.html"),
        _ => return None,
    })
}

/// Java `hasItem(player, holder)`: a missing holder is not owned.
fn has_item(ctx: &ScriptCtx<'_>, player: ObjectId, item: Option<(i32, i64)>) -> bool {
    item.is_some_and(|(id, count)| ctx.get_quest_items_count(player, id) >= count)
}

/// Java `takeItem(player, holder)`: a missing holder takes nothing.
fn take_item(ctx: &mut ScriptCtx<'_>, player: ObjectId, item: Option<(i32, i64)>) {
    if let Some((id, count)) = item {
        ctx.take_items(player, id, count);
    }
}

/// Java `Q00373_SupplierOfReagents`.
pub struct Q00373SupplierOfReagents;

impl Script for Q00373SupplierOfReagents {
    fn name(&self) -> &'static str {
        "Q00373_SupplierOfReagents"
    }

    fn path(&self) -> &'static str {
        "quests/Q00373_SupplierOfReagents"
    }

    fn id(&self) -> i32 {
        373
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WESLEY]);
        s.add_kill_id(&[
            HALLATE_GUARDIAN,
            HALLATE_MAID,
            HAMES_ORC_SHAMAN,
            LAVA_WYRM,
            CRENDION,
            PLATINUM_GUARDIAN_SHAMAN,
            PLATINUM_TRIBE_SHAMAN,
        ]);
        s.add_talk_id(&[WESLEY, ALCHEMIST_MIXING_URN]);
        s.register_quest_items(&[WESLEYS_MIXING_STONE, MIXING_MANUAL]);
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

        match event {
            "30166-03.htm" | "30166-06.html" | "30166-04a.html" | "30166-04b.html" | "30166-04c.html"
            | "30166-04d.html" | "31149-18.html" => Some(event.to_owned()),
            "30166-04.html" => {
                if ctx.player_level(player) >= MIN_LEVEL && qs.is_created(ctx) {
                    ctx.give_items(player, WESLEYS_MIXING_STONE, 1);
                    ctx.give_items(player, MIXING_MANUAL, 1);
                    qs.start_quest(ctx);
                    return Some(event.to_owned());
                }
                None
            }
            "30166-07.html" => {
                qs.exit_quest(ctx, true, true);
                Some(event.to_owned())
            }
            "31149-02.html" => {
                qs.set_memo_state(ctx, 0);
                qs.set_memo_state_ex(ctx, 1, 0);
                Some(event.to_owned())
            }
            "31149-03.html" | "31149-04.html" | "31149-05.html" | "31149-06.html" | "31149-07.html"
            | "31149-08.html" | "31149-09.html" | "31149-10.html" | "31149-11.html" | "31149-12.html"
            | "31149-13.html" | "31149-14.html" | "31149-15.html" | "31149-16.html" | "31149-19.html"
            | "31149-20.html" | "31149-21.html" | "31149-22.html" | "31149-23.html" | "31149-24.html" => {
                let memo_state = html_to_memo_state(event)?;
                if has_item(ctx, player, memo_state_to_item(memo_state)) {
                    // The chosen item (ingredient or catalyst) is saved for the catalyst or the reward.
                    qs.set_memo_state(ctx, qs.get_memo_state(ctx) + memo_state);
                    ctx.play_sound(player, QuestSound::SkillsoundLiquidMix);
                    Some(event.to_owned())
                } else {
                    // Without the chosen catalyst the ingredient saved before is taken.
                    take_item(ctx, player, memo_state_to_item(qs.get_memo_state(ctx)));
                    if event == "31149-19.html" {
                        Some("31149-25.html".to_owned())
                    } else {
                        Some("31149-17.html".to_owned())
                    }
                }
            }
            "31149-26.html" => {
                if qs.is_memo_state(ctx, 1324) {
                    Some("31149-26a.html".to_owned())
                } else {
                    Some(event.to_owned())
                }
            }
            "31149-27.html" => {
                qs.set_memo_state_ex(ctx, 1, 1); // Temperature Salamander
                Some(event.to_owned())
            }
            "31149-28a.html" => {
                if ctx.get_random(100) < 33 {
                    qs.set_memo_state_ex(ctx, 1, 3); // Temperature Ifrit
                } else {
                    qs.set_memo_state_ex(ctx, 1, 0);
                }
                Some(event.to_owned())
            }
            "31149-29a.html" => {
                if ctx.get_random(100) < 20 {
                    qs.set_memo_state_ex(ctx, 1, 5); // Temperature Phoenix
                } else {
                    qs.set_memo_state_ex(ctx, 1, 0);
                }
                Some(event.to_owned())
            }
            "mixitems" => {
                let memo_state = qs.get_memo_state(ctx);
                let item1 = memo_state_to_item(memo_state % 100);
                let item2 = memo_state_to_item((memo_state / 100) * 100);
                let reward = memo_state_to_reward(memo_state);
                let mimirs_started = ctx
                    .player_quest_state(player, "Q00235_MimirsElixir")
                    .is_some_and(|q| q.is_started());
                if reward.is_none() || qs.is_memo_state_ex(ctx, 1, 0) {
                    take_item(ctx, player, item1);
                    take_item(ctx, player, item2);
                    ctx.play_sound(player, QuestSound::SkillsoundLiquidFail);
                    Some(
                        if reward.is_none() {
                            "31149-44.html"
                        } else {
                            "31149-45.html"
                        }
                        .to_owned(),
                    )
                } else if let Some((reward_item, reward_html)) = reward
                    && (memo_state != 1324 || (mimirs_started && !ctx.has_quest_items(player, reward_item)))
                {
                    if has_item(ctx, player, item1) && has_item(ctx, player, item2) {
                        take_item(ctx, player, item1);
                        take_item(ctx, player, item2);
                        let amount = if memo_state == 1324 {
                            1
                        } else {
                            i64::from(qs.get_memo_state_ex(ctx, 1))
                        };
                        ctx.give_items(player, reward_item, amount);
                        qs.set_memo_state(ctx, 0);
                        qs.set_memo_state_ex(ctx, 1, 0);
                        ctx.play_sound(player, QuestSound::SkillsoundLiquidSuccess);
                        Some(reward_html.to_owned())
                    } else {
                        ctx.play_sound(player, QuestSound::SkillsoundLiquidFail);
                        Some("31149-44.html".to_owned())
                    }
                } else {
                    Some("31149-44.html".to_owned())
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) else {
            return;
        };
        let member = qs.player();
        // `giveItemRandomly(member, npc, item, amount, 0, 1, true)`: no limit, always.
        match npc.id {
            HALLATE_GUARDIAN => {
                let chance = ctx.get_random(1000);
                if chance < 766 {
                    ctx.give_item_randomly(member, Some(npc), DEMONS_BLOOD, 3, 3, 0, 1.0, true);
                } else if chance < 876 {
                    ctx.give_item_randomly(member, Some(npc), MOONSTONE_SHARD, 1, 1, 0, 1.0, true);
                }
            }
            HALLATE_MAID => {
                let chance = ctx.get_random(100);
                if chance < 45 {
                    ctx.give_item_randomly(member, Some(npc), REAGENT_POUNCH1, 1, 1, 0, 1.0, true);
                } else if chance < 65 {
                    ctx.give_item_randomly(member, Some(npc), VOLCANIC_ASH, 1, 1, 0, 1.0, true);
                }
            }
            HAMES_ORC_SHAMAN => {
                if ctx.get_random(1000) < 616 {
                    ctx.give_item_randomly(member, Some(npc), REAGENT_POUNCH3, 1, 1, 0, 1.0, true);
                }
            }
            LAVA_WYRM => {
                let chance = ctx.get_random(1000);
                if chance < 666 {
                    ctx.give_item_randomly(member, Some(npc), WYRM_BLOOD, 1, 1, 0, 1.0, true);
                } else if chance < 989 {
                    ctx.give_item_randomly(member, Some(npc), LAVA_STONE, 1, 1, 0, 1.0, true);
                }
            }
            CRENDION => {
                if ctx.get_random(1000) < 618 {
                    ctx.give_item_randomly(member, Some(npc), ROTTEN_BONE, 1, 1, 0, 1.0, true);
                } else {
                    ctx.give_item_randomly(member, Some(npc), QUICKSILVER, 1, 1, 0, 1.0, true);
                }
            }
            PLATINUM_GUARDIAN_SHAMAN => {
                if ctx.get_random(1000) < 444 {
                    ctx.give_item_randomly(member, Some(npc), REAGENT_BOX, 1, 1, 0, 1.0, true);
                }
            }
            PLATINUM_TRIBE_SHAMAN => {
                if ctx.get_random(1000) < 658 {
                    ctx.give_item_randomly(member, Some(npc), REAGENT_POUNCH2, 1, 1, 0, 1.0, true);
                } else {
                    ctx.give_item_randomly(member, Some(npc), QUICKSILVER, 2, 2, 0, 1.0, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(talker) < MIN_LEVEL {
                    "30166-01.html"
                } else {
                    "30166-02.htm"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if npc.id == WESLEY {
                    "30166-05.html"
                } else {
                    "31149-01.html"
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
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00373_SupplierOfReagents";

    const NPCS: &str = r#"
        <npc id="30166" type="Folk" level="70" name="Wesley"/>
        <npc id="31149" type="Folk" level="70" name="Alchemist's Mixing Urn"/>
        <npc id="21111" type="Monster" level="60" name="Lava Wyrm"/>"#;

    fn items() -> String {
        let mut out = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#,
        );
        for id in (6007..=6034).chain([5904, 6317, 6320]) {
            out.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Item {id}"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#
            ));
        }
        out
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00373SupplierOfReagents);
        let pages = Pages::of(&Q00373SupplierOfReagents)
            .page("30166-02.htm", &["30166-03.htm"])
            .page("30166-03.htm", &["30166-04.html"])
            .page("30166-05.html", &["30166-06.html", "30166-07.html"])
            .page("31149-01.html", &["31149-02.html"])
            .page(
                "31149-02.html",
                &["31149-03.html", "31149-04.html", "31149-11.html", "31149-16.html"],
            )
            .page("31149-03.html", &["31149-18.html"])
            .page("31149-18.html", &["31149-19.html", "31149-20.html"])
            .page("31149-19.html", &["31149-26.html"])
            .page(
                "31149-26.html",
                &["31149-27.html", "31149-28a.html", "31149-29a.html"],
            )
            .page("31149-27.html", &["mixitems"])
            .plain(&[
                "30166-01.html",
                "30166-04.html",
                "30166-06.html",
                "30166-07.html",
                "31149-17.html",
                "31149-25.html",
                "31149-30.html",
                "31149-44.html",
                "31149-45.html",
            ])
            .npcs(&[WESLEY, ALCHEMIST_MIXING_URN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, &items(), pages);
        let wesley = t.spawn(WESLEY, 20);
        let urn = t.spawn(ALCHEMIST_MIXING_URN, 30);
        (t, wesley, urn)
    }

    fn start(t: &mut ScriptTest, wesley: Oid) {
        assert!(t.quest_page(1, wesley).contains("30166-02.htm"));
        assert!(t.event(1, NAME, "30166-03.htm").contains("30166-03.htm"));
        assert!(t.event(1, NAME, "30166-04.html").contains("30166-04.html"));
    }

    #[test]
    fn a_low_level_character_is_sent_away() {
        let (mut t, wesley, _urn) = setup();
        t.enter(1, 56, vec![]);
        assert!(t.quest_page(1, wesley).contains("30166-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn wesley_hands_out_the_stone_and_the_manual() {
        let (mut t, wesley, _urn) = setup();
        t.enter(1, 57, vec![]);
        start(&mut t, wesley);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(
            (t.count(1, WESLEYS_MIXING_STONE), t.count(1, MIXING_MANUAL)),
            (1, 1)
        );
        assert!(t.quest_page(1, wesley).contains("30166-05.html"));
        assert!(t.event(1, NAME, "30166-07.html").contains("30166-07.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn wyrm_blood_and_blood_root_are_mixed_into_dracoplasm() {
        let (mut t, wesley, urn) = setup();
        t.enter(1, 57, vec![item(100, WYRM_BLOOD, 10), item(101, BLOOD_ROOT, 1)]);
        start(&mut t, wesley);
        t.take_writes();
        assert!(t.quest_page(1, urn).contains("31149-01.html"));
        assert!(t.event(1, NAME, "31149-02.html").contains("31149-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "memoState", "0"),
                rows::insert(NAME, "memoStateEx1", "0")
            ]
        );
        assert!(t.event(1, NAME, "31149-03.html").contains("31149-03.html"));
        assert!(t.event(1, NAME, "31149-18.html").contains("31149-18.html"));
        assert!(t.event(1, NAME, "31149-19.html").contains("31149-19.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "11"),
                rows::update(NAME, "memoState", "1111")
            ]
        );
        assert!(t.event(1, NAME, "31149-26.html").contains("31149-26.html"));
        assert!(t.event(1, NAME, "31149-27.html").contains("31149-27.html"));
        t.take_writes();
        assert!(t.event(1, NAME, "mixitems").contains("31149-30.html"));
        assert_eq!(
            (
                t.count(1, WYRM_BLOOD),
                t.count(1, BLOOD_ROOT),
                t.count(1, DRACOPLASM)
            ),
            (0, 0, 1)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "0"),
                rows::update(NAME, "memoStateEx1", "0")
            ]
        );
    }

    #[test]
    fn a_missing_catalyst_takes_the_ingredient() {
        let (mut t, wesley, urn) = setup();
        t.enter(1, 57, vec![item(100, WYRM_BLOOD, 10)]);
        start(&mut t, wesley);
        assert!(t.quest_page(1, urn).contains("31149-01.html"));
        t.event(1, NAME, "31149-02.html");
        t.event(1, NAME, "31149-03.html");
        t.event(1, NAME, "31149-18.html");
        assert!(t.event(1, NAME, "31149-19.html").contains("31149-25.html"));
        assert_eq!(t.count(1, WYRM_BLOOD), 0);
    }

    #[test]
    fn a_lava_wyrm_drops_blood_or_stone() {
        let (mut t, wesley, _urn) = setup();
        t.enter(1, 57, vec![]);
        start(&mut t, wesley);
        for _ in 0..50 {
            let wyrm = t.spawn(LAVA_WYRM, 60);
            t.kill(1, wyrm);
        }
        assert!(t.count(1, WYRM_BLOOD) + t.count(1, LAVA_STONE) > 0);
    }
}
