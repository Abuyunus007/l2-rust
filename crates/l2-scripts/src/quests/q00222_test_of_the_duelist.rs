//! Quest 222: Test of the Duelist (Duelist Kaien in Gludio: the Warrior, Elven Knight, Palus Knight
//! and Orc Monk second class change).
// Java: dist/game/data/scripts/quests/Q00222_TestOfTheDuelist/Q00222_TestOfTheDuelist.java
//
// Pattern: the test hunt of the "Test of ..." quests. The memo state picks the step, `memoStateEx(1)`
// counts the kills of the step, and each kill rolls one item (`giveItemRandomly` with a limit) until
// the whole step is in the inventory (`getQuestItemsCount(.., ..) == total`).

use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, player_class, race};
use l2_world::{Qs, QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const DUELIST_KAIEN: i32 = 30623;

// Items
const ORDER_GLUDIO: i32 = 2763;
const ORDER_DION: i32 = 2764;
const ORDER_GIRAN: i32 = 2765;
const ORDER_OREN: i32 = 2766;
const ORDER_ADEN: i32 = 2767;
const PUNCHERS_SHARD: i32 = 2768;
const NOBLE_ANTS_FEELER: i32 = 2769;
const DRONES_CHITIN: i32 = 2770;
const DEAD_SEEKER_FANG: i32 = 2771;
const OVERLORD_NECKLACE: i32 = 2772;
const FETTERED_SOULS_CHAIN: i32 = 2773;
const CHIEDS_AMULET: i32 = 2774;
const ENCHANTED_EYE_MEAT: i32 = 2775;
const TAMRIN_ORCS_RING: i32 = 2776;
const TAMRIN_ORCS_ARROW: i32 = 2777;
const FINAL_ORDER: i32 = 2778;
const EXCUROS_SKIN: i32 = 2779;
const KRATORS_SHARD: i32 = 2780;
const GRANDIS_SKIN: i32 = 2781;
const TIMAK_ORCS_BELT: i32 = 2782;
const LAKINS_MACE: i32 = 2783;

// Reward
const MARK_OF_DUELIST: i32 = 2762;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monsters
const PUNCHER: i32 = 20085;
const NOBLE_ANT_LEADER: i32 = 20090;
const DEAD_SEEKER: i32 = 20202;
const EXCURO: i32 = 20214;
const KRATOR: i32 = 20217;
const MARSH_STAKATO_DRONE: i32 = 20234;
const BREKA_ORC_OVERLORD: i32 = 20270;
const FETTERED_SOUL: i32 = 20552;
const GRANDIS: i32 = 20554;
const ENCHANTED_MONSTEREYE: i32 = 20564;
const LETO_LIZARDMAN_OVERLORD: i32 = 20582;
const TIMAK_ORC_OVERLORD: i32 = 20588;
const TAMLIN_ORC: i32 = 20601;
const TAMLIN_ORC_ARCHER: i32 = 20602;
const LAKIN: i32 = 20604;

// Misc
const MIN_LEVEL: i32 = 39;

/// A test hunt's counts: each item is rolled up to `limit`; the step is done when the other items
/// of the step add up to `total`. The kill counter (`memoStateEx(1)`) reaching `cond_at` moves the
/// condition to `cond`.
struct Step {
    limit: i64,
    total: i64,
    cond_at: i32,
    cond: i32,
}

/// The step-1 hunt (`memoState` 1): ten of each of the nine items, 90 in all.
const STEP1: Step = Step {
    limit: 10,
    total: 90,
    cond_at: 9,
    cond: 3,
};
/// The step-2 hunt (`memoState` 2): three of each of the four items, 12 in all.
const STEP2: Step = Step {
    limit: 3,
    total: 12,
    cond_at: 5,
    cond: 5,
};

/// Java `Q00222_TestOfTheDuelist`.
pub struct Q00222TestOfTheDuelist;

impl Script for Q00222TestOfTheDuelist {
    fn name(&self) -> &'static str {
        "Q00222_TestOfTheDuelist"
    }

    fn path(&self) -> &'static str {
        "quests/Q00222_TestOfTheDuelist"
    }

    fn id(&self) -> i32 {
        222
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DUELIST_KAIEN]);
        s.add_talk_id(&[DUELIST_KAIEN]);
        s.add_kill_id(&[
            PUNCHER,
            NOBLE_ANT_LEADER,
            DEAD_SEEKER,
            EXCURO,
            KRATOR,
            MARSH_STAKATO_DRONE,
            BREKA_ORC_OVERLORD,
            FETTERED_SOUL,
            GRANDIS,
            ENCHANTED_MONSTEREYE,
            LETO_LIZARDMAN_OVERLORD,
            TIMAK_ORC_OVERLORD,
            TAMLIN_ORC,
            TAMLIN_ORC_ARCHER,
            LAKIN,
        ]);
        s.register_quest_items(&[
            ORDER_GLUDIO,
            ORDER_DION,
            ORDER_GIRAN,
            ORDER_OREN,
            ORDER_ADEN,
            PUNCHERS_SHARD,
            NOBLE_ANTS_FEELER,
            DRONES_CHITIN,
            DEAD_SEEKER_FANG,
            OVERLORD_NECKLACE,
            FETTERED_SOULS_CHAIN,
            CHIEDS_AMULET,
            ENCHANTED_EYE_MEAT,
            TAMRIN_ORCS_RING,
            TAMRIN_ORCS_ARROW,
            FINAL_ORDER,
            EXCUROS_SKIN,
            KRATORS_SHARD,
            GRANDIS_SKIN,
            TIMAK_ORCS_BELT,
            LAKINS_MACE,
        ]);
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
            "ACCEPT" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    for order in [ORDER_GLUDIO, ORDER_DION, ORDER_GIRAN, ORDER_OREN, ORDER_ADEN] {
                        ctx.give_items(player, order, 1);
                    }
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    if ctx.player_variable_int(player, "2ND_CLASS_DIAMOND_REWARD", 0) == 0 {
                        if ctx.player_class_id(player) == player_class::PALUS_KNIGHT {
                            ctx.give_items(player, DIMENSIONAL_DIAMOND, 104);
                        } else {
                            ctx.give_items(player, DIMENSIONAL_DIAMOND, 72);
                        }
                        ctx.set_player_variable(player, "2ND_CLASS_DIAMOND_REWARD", "1");
                        htmltext = Some("30623-07a.htm".to_owned());
                    } else {
                        htmltext = Some("30623-07.htm".to_owned());
                    }
                }
            }
            "30623-04.htm" => {
                if ctx.player_race(player) != race::ORC {
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30623-05.htm".to_owned());
                }
            }
            "30623-06.htm" | "30623-07.html" | "30623-09.html" | "30623-10.html" | "30623-11.html"
            | "30623-12.html" | "30623-15.html" => {
                htmltext = Some(event.to_owned());
            }
            "30623-08.html" => {
                qs.set_cond(ctx, 2, true);
                htmltext = Some(event.to_owned());
            }
            "30623-16.html" => {
                for item in [
                    PUNCHERS_SHARD,
                    NOBLE_ANTS_FEELER,
                    DEAD_SEEKER_FANG,
                    DRONES_CHITIN,
                    OVERLORD_NECKLACE,
                    FETTERED_SOULS_CHAIN,
                    CHIEDS_AMULET,
                    ENCHANTED_EYE_MEAT,
                    TAMRIN_ORCS_RING,
                    TAMRIN_ORCS_ARROW,
                ] {
                    ctx.take_items(player, item, -1);
                }
                for order in [ORDER_GLUDIO, ORDER_DION, ORDER_GIRAN, ORDER_OREN, ORDER_ADEN] {
                    ctx.take_items(player, order, 1);
                }
                ctx.give_items(player, FINAL_ORDER, 1);
                qs.set_memo_state(ctx, 2);
                qs.set_cond(ctx, 4, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            match npc.id {
                PUNCHER => {
                    if qs.is_memo_state(ctx, 1) && ctx.has_quest_items(killer, ORDER_GLUDIO) {
                        let others = [
                            NOBLE_ANTS_FEELER,
                            DRONES_CHITIN,
                            DEAD_SEEKER_FANG,
                            OVERLORD_NECKLACE,
                            FETTERED_SOULS_CHAIN,
                            CHIEDS_AMULET,
                            ENCHANTED_EYE_MEAT,
                            TAMRIN_ORCS_RING,
                            TAMRIN_ORCS_ARROW,
                        ];
                        hunt_kill(ctx, qs, killer, npc, PUNCHERS_SHARD, &STEP1, &others);
                    }
                }
                NOBLE_ANT_LEADER => {
                    if qs.is_memo_state(ctx, 1) && ctx.has_quest_items(killer, ORDER_GLUDIO) {
                        let others = [
                            PUNCHERS_SHARD,
                            DRONES_CHITIN,
                            DEAD_SEEKER_FANG,
                            OVERLORD_NECKLACE,
                            FETTERED_SOULS_CHAIN,
                            CHIEDS_AMULET,
                            ENCHANTED_EYE_MEAT,
                            TAMRIN_ORCS_RING,
                            TAMRIN_ORCS_ARROW,
                        ];
                        hunt_kill(ctx, qs, killer, npc, NOBLE_ANTS_FEELER, &STEP1, &others);
                    }
                }
                DEAD_SEEKER => {
                    if qs.is_memo_state(ctx, 1) && ctx.has_quest_items(killer, ORDER_DION) {
                        let others = [
                            PUNCHERS_SHARD,
                            NOBLE_ANTS_FEELER,
                            DRONES_CHITIN,
                            OVERLORD_NECKLACE,
                            FETTERED_SOULS_CHAIN,
                            CHIEDS_AMULET,
                            ENCHANTED_EYE_MEAT,
                            TAMRIN_ORCS_RING,
                            TAMRIN_ORCS_ARROW,
                        ];
                        hunt_kill(ctx, qs, killer, npc, DEAD_SEEKER_FANG, &STEP1, &others);
                    }
                }
                EXCURO => {
                    if qs.is_memo_state(ctx, 2) && ctx.has_quest_items(killer, FINAL_ORDER) {
                        let others = [KRATORS_SHARD, LAKINS_MACE, GRANDIS_SKIN, TIMAK_ORCS_BELT];
                        hunt_kill(ctx, qs, killer, npc, EXCUROS_SKIN, &STEP2, &others);
                    }
                }
                KRATOR => {
                    if qs.is_memo_state(ctx, 2) && ctx.has_quest_items(killer, FINAL_ORDER) {
                        let others = [EXCUROS_SKIN, LAKINS_MACE, GRANDIS_SKIN, TIMAK_ORCS_BELT];
                        hunt_kill(ctx, qs, killer, npc, KRATORS_SHARD, &STEP2, &others);
                    }
                }
                MARSH_STAKATO_DRONE => {
                    if qs.is_memo_state(ctx, 1) && ctx.has_quest_items(killer, ORDER_DION) {
                        let others = [
                            PUNCHERS_SHARD,
                            NOBLE_ANTS_FEELER,
                            DEAD_SEEKER_FANG,
                            OVERLORD_NECKLACE,
                            FETTERED_SOULS_CHAIN,
                            CHIEDS_AMULET,
                            ENCHANTED_EYE_MEAT,
                            TAMRIN_ORCS_RING,
                            TAMRIN_ORCS_ARROW,
                        ];
                        hunt_kill(ctx, qs, killer, npc, DRONES_CHITIN, &STEP1, &others);
                    }
                }
                BREKA_ORC_OVERLORD => {
                    if qs.is_memo_state(ctx, 1) && ctx.has_quest_items(killer, ORDER_GIRAN) {
                        let others = [
                            PUNCHERS_SHARD,
                            NOBLE_ANTS_FEELER,
                            DRONES_CHITIN,
                            DEAD_SEEKER_FANG,
                            FETTERED_SOULS_CHAIN,
                            CHIEDS_AMULET,
                            ENCHANTED_EYE_MEAT,
                            TAMRIN_ORCS_RING,
                            TAMRIN_ORCS_ARROW,
                        ];
                        hunt_kill(ctx, qs, killer, npc, OVERLORD_NECKLACE, &STEP1, &others);
                    }
                }
                FETTERED_SOUL => {
                    if qs.is_memo_state(ctx, 1) && ctx.has_quest_items(killer, ORDER_GIRAN) {
                        let others = [
                            PUNCHERS_SHARD,
                            NOBLE_ANTS_FEELER,
                            DRONES_CHITIN,
                            DEAD_SEEKER_FANG,
                            OVERLORD_NECKLACE,
                            CHIEDS_AMULET,
                            ENCHANTED_EYE_MEAT,
                            TAMRIN_ORCS_RING,
                            TAMRIN_ORCS_ARROW,
                        ];
                        hunt_kill(ctx, qs, killer, npc, FETTERED_SOULS_CHAIN, &STEP1, &others);
                    }
                }
                GRANDIS => {
                    if qs.is_memo_state(ctx, 2) && ctx.has_quest_items(killer, FINAL_ORDER) {
                        let others = [EXCUROS_SKIN, KRATORS_SHARD, LAKINS_MACE, TIMAK_ORCS_BELT];
                        hunt_kill(ctx, qs, killer, npc, GRANDIS_SKIN, &STEP2, &others);
                    }
                }
                ENCHANTED_MONSTEREYE => {
                    if qs.is_memo_state(ctx, 1) && ctx.has_quest_items(killer, ORDER_OREN) {
                        let others = [
                            PUNCHERS_SHARD,
                            NOBLE_ANTS_FEELER,
                            DRONES_CHITIN,
                            DEAD_SEEKER_FANG,
                            OVERLORD_NECKLACE,
                            FETTERED_SOULS_CHAIN,
                            CHIEDS_AMULET,
                            TAMRIN_ORCS_RING,
                            TAMRIN_ORCS_ARROW,
                        ];
                        hunt_kill(ctx, qs, killer, npc, ENCHANTED_EYE_MEAT, &STEP1, &others);
                    }
                }
                LETO_LIZARDMAN_OVERLORD => {
                    if qs.is_memo_state(ctx, 1) && ctx.has_quest_items(killer, ORDER_OREN) {
                        let others = [
                            PUNCHERS_SHARD,
                            NOBLE_ANTS_FEELER,
                            DRONES_CHITIN,
                            DEAD_SEEKER_FANG,
                            OVERLORD_NECKLACE,
                            FETTERED_SOULS_CHAIN,
                            ENCHANTED_EYE_MEAT,
                            TAMRIN_ORCS_RING,
                            TAMRIN_ORCS_ARROW,
                        ];
                        hunt_kill(ctx, qs, killer, npc, CHIEDS_AMULET, &STEP1, &others);
                    }
                }
                TIMAK_ORC_OVERLORD => {
                    if qs.is_memo_state(ctx, 2) && ctx.has_quest_items(killer, FINAL_ORDER) {
                        let others = [EXCUROS_SKIN, KRATORS_SHARD, LAKINS_MACE, GRANDIS_SKIN];
                        hunt_kill(ctx, qs, killer, npc, TIMAK_ORCS_BELT, &STEP2, &others);
                    }
                }
                TAMLIN_ORC => {
                    if qs.is_memo_state(ctx, 1) && ctx.has_quest_items(killer, ORDER_ADEN) {
                        let others = [
                            PUNCHERS_SHARD,
                            NOBLE_ANTS_FEELER,
                            DRONES_CHITIN,
                            DEAD_SEEKER_FANG,
                            OVERLORD_NECKLACE,
                            FETTERED_SOULS_CHAIN,
                            CHIEDS_AMULET,
                            ENCHANTED_EYE_MEAT,
                            TAMRIN_ORCS_ARROW,
                        ];
                        hunt_kill(ctx, qs, killer, npc, TAMRIN_ORCS_RING, &STEP1, &others);
                    }
                }
                TAMLIN_ORC_ARCHER => {
                    if qs.is_memo_state(ctx, 1) && ctx.has_quest_items(killer, ORDER_ADEN) {
                        let others = [
                            PUNCHERS_SHARD,
                            NOBLE_ANTS_FEELER,
                            DRONES_CHITIN,
                            DEAD_SEEKER_FANG,
                            OVERLORD_NECKLACE,
                            FETTERED_SOULS_CHAIN,
                            CHIEDS_AMULET,
                            ENCHANTED_EYE_MEAT,
                            TAMRIN_ORCS_RING,
                        ];
                        hunt_kill(ctx, qs, killer, npc, TAMRIN_ORCS_ARROW, &STEP1, &others);
                    }
                }
                LAKIN => {
                    if qs.is_memo_state(ctx, 2) && ctx.has_quest_items(killer, FINAL_ORDER) {
                        let others = [EXCUROS_SKIN, KRATORS_SHARD, GRANDIS_SKIN, TIMAK_ORCS_BELT];
                        hunt_kill(ctx, qs, killer, npc, LAKINS_MACE, &STEP2, &others);
                    }
                }
                _ => {}
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            if matches!(
                ctx.player_class_id(player),
                player_class::WARRIOR
                    | player_class::ELVEN_KNIGHT
                    | player_class::PALUS_KNIGHT
                    | player_class::ORC_MONK
            ) {
                if ctx.player_level(player) >= MIN_LEVEL {
                    htmltext = "30623-03.htm".to_owned();
                } else {
                    htmltext = "30623-01.html".to_owned();
                }
            } else {
                htmltext = "30623-02.html".to_owned();
            }
        } else if state == STARTED {
            if ctx.has_all_quest_items(
                player,
                &[ORDER_GLUDIO, ORDER_DION, ORDER_GIRAN, ORDER_OREN, ORDER_ADEN],
            ) {
                if ctx.get_quest_items_count_of(
                    player,
                    &[
                        PUNCHERS_SHARD,
                        NOBLE_ANTS_FEELER,
                        DRONES_CHITIN,
                        DEAD_SEEKER_FANG,
                        OVERLORD_NECKLACE,
                        FETTERED_SOULS_CHAIN,
                        CHIEDS_AMULET,
                        ENCHANTED_EYE_MEAT,
                        TAMRIN_ORCS_RING,
                        TAMRIN_ORCS_ARROW,
                    ],
                ) == 100
                {
                    htmltext = "30623-13.html".to_owned();
                } else {
                    htmltext = "30623-14.html".to_owned();
                }
            } else if ctx.has_quest_items(player, FINAL_ORDER) {
                if ctx.get_quest_items_count_of(
                    player,
                    &[
                        EXCUROS_SKIN,
                        KRATORS_SHARD,
                        LAKINS_MACE,
                        GRANDIS_SKIN,
                        TIMAK_ORCS_BELT,
                    ],
                ) == 15
                {
                    ctx.give_adena(player, 161_806, true);
                    ctx.give_items(player, MARK_OF_DUELIST, 1);
                    ctx.add_exp_and_sp(player, 894_888, 61_408);
                    qs.exit_quest(ctx, false, true);
                    ctx.world().send(
                        player,
                        &SocialAction {
                            object_id: player.0,
                            action_id: 3,
                        },
                    );
                    htmltext = "30623-18.html".to_owned();
                } else {
                    htmltext = "30623-17.html".to_owned();
                }
            }
        } else if state == COMPLETED {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }
}

/// One kill of a test hunt (Java: `qs.setMemoStateEx(1, i0 + 1)`, the roll, the condition at the
/// kill counter, the counter reset when the step is complete).
fn hunt_kill(
    ctx: &mut ScriptCtx<'_>,
    qs: Qs,
    killer: ObjectId,
    npc: ScriptNpc,
    item: i32,
    step: &Step,
    others: &[i32],
) {
    let i0 = qs.get_memo_state_ex(ctx, 1);
    qs.set_memo_state_ex(ctx, 1, i0 + 1);
    if ctx.give_item_randomly(killer, Some(npc), item, 1, 1, step.limit, 1.0, true)
        && ctx.get_quest_items_count_of(killer, others) == step.total
    {
        if i0 >= step.cond_at {
            qs.set_cond(ctx, step.cond, false);
        }
        qs.set_memo_state_ex(ctx, 1, 0);
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00222_TestOfTheDuelist";

    const NPCS: &str = r#"
        <npc id="30623" type="Folk" level="70" name="Duelist Kaien"/>
        <npc id="20085" type="Monster" level="40" name="Puncher"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="2762" type="EtcItem" name="Mark of Duelist"><set name="is_questitem" val="true"/></item>
        <item id="2763" type="EtcItem" name="Order of Gludio"><set name="is_questitem" val="true"/></item>
        <item id="2764" type="EtcItem" name="Order of Dion"><set name="is_questitem" val="true"/></item>
        <item id="2765" type="EtcItem" name="Order of Giran"><set name="is_questitem" val="true"/></item>
        <item id="2766" type="EtcItem" name="Order of Oren"><set name="is_questitem" val="true"/></item>
        <item id="2767" type="EtcItem" name="Order of Aden"><set name="is_questitem" val="true"/></item>
        <item id="2768" type="EtcItem" name="Puncher Shard"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2769" type="EtcItem" name="Noble Ant Feeler"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2770" type="EtcItem" name="Drone Chitin"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2771" type="EtcItem" name="Dead Seeker Fang"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2772" type="EtcItem" name="Overlord Necklace"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2773" type="EtcItem" name="Fettered Souls Chain"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2774" type="EtcItem" name="Chieds Amulet"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2775" type="EtcItem" name="Enchanted Eye Meat"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2776" type="EtcItem" name="Tamrin Orcs Ring"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2777" type="EtcItem" name="Tamrin Orcs Arrow"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2778" type="EtcItem" name="Final Order"><set name="is_questitem" val="true"/></item>
        <item id="2779" type="EtcItem" name="Excuros Skin"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2780" type="EtcItem" name="Krators Shard"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2781" type="EtcItem" name="Grandis Skin"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2782" type="EtcItem" name="Timak Orcs Belt"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2783" type="EtcItem" name="Lakins Mace"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    /// The pages of the quest with the links of the real pages.
    fn pages() -> Pages {
        Pages::of(&Q00222TestOfTheDuelist)
            .plain(&[
                "30623-01.html",
                "30623-02.html",
                "30623-14.html",
                "30623-17.html",
                "30623-18.html",
            ])
            .page("30623-03.htm", &["30623-04.htm"])
            .page("30623-04.htm", &["30623-06.htm"])
            .page("30623-05.htm", &["30623-06.htm"])
            .page("30623-06.htm", &["ACCEPT"])
            .page("30623-07.htm", &["30623-08.html"])
            .page("30623-07a.htm", &["30623-08.html"])
            .page("30623-08.html", &["30623-09.html"])
            .page("30623-13.html", &["30623-15.html"])
            .page("30623-15.html", &["30623-16.html"])
            .plain(&["30623-16.html"])
            .npcs(&[DUELIST_KAIEN])
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00222TestOfTheDuelist);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let kaien = t.spawn(DUELIST_KAIEN, 20);
        (t, kaien)
    }

    /// Gives the player `count` of each of the items (the hunt's counts, without the kills).
    fn give_all(t: &mut ScriptTest, items: &[i32], count: i64) {
        t.world.run_as_script(NAME, |ctx| {
            for &item in items {
                ctx.give_items(Oid(1), item, count);
            }
        });
    }

    /// The nine step-1 items other than the puncher's shard.
    const PUNCHER_OTHERS: [i32; 9] = [
        NOBLE_ANTS_FEELER,
        DRONES_CHITIN,
        DEAD_SEEKER_FANG,
        OVERLORD_NECKLACE,
        FETTERED_SOULS_CHAIN,
        CHIEDS_AMULET,
        ENCHANTED_EYE_MEAT,
        TAMRIN_ORCS_RING,
        TAMRIN_ORCS_ARROW,
    ];

    /// Talks to Kaien, the rules page (`30623-04.htm`), the accept page and `ACCEPT`.
    fn accept_page(t: &mut ScriptTest, player: i32, kaien: Oid) -> String {
        t.quest_page(player, kaien);
        t.event(player, NAME, "30623-04.htm");
        t.event(player, NAME, "30623-06.htm");
        t.event(player, NAME, "ACCEPT")
    }

    fn accept(t: &mut ScriptTest, player: i32, kaien: Oid) {
        assert!(accept_page(t, player, kaien).contains("30623-07"));
    }

    #[test]
    fn only_the_second_class_warriors_of_level_39_are_taken() {
        let (mut t, kaien) = setup();
        t.enter_as(1, 38, race::HUMAN, player_class::WARRIOR, vec![]);
        assert!(t.quest_page(1, kaien).contains("30623-01.html"));
        t.enter_as(2, 40, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, kaien).contains("30623-02.html"));
        t.enter_as(3, 39, race::HUMAN, player_class::WARRIOR, vec![]);
        assert!(t.quest_page(3, kaien).contains("30623-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn an_orc_is_sent_back_and_a_human_is_told_the_quest() {
        let (mut t, kaien) = setup();
        t.enter_as(1, 39, race::ORC, player_class::ORC_MONK, vec![]);
        t.quest_page(1, kaien);
        assert!(t.event(1, NAME, "30623-04.htm").contains("30623-05.htm"));

        t.enter_as(2, 39, race::HUMAN, player_class::PALUS_KNIGHT, vec![]);
        t.quest_page(2, kaien);
        assert!(t.event(2, NAME, "30623-04.htm").contains("30623-04.htm"));
        assert!(t.event(2, NAME, "30623-06.htm").contains("30623-06.htm"));
    }

    #[test]
    fn accepting_gives_the_orders_and_the_diamonds_once() {
        let (mut t, kaien) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::PALUS_KNIGHT, vec![]);
        assert!(accept_page(&mut t, 1, kaien).contains("30623-07a.htm"));
        // Palus Knights get 104 diamonds, the other classes 72.
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 104);
        for order in [ORDER_GLUDIO, ORDER_DION, ORDER_GIRAN, ORDER_OREN, ORDER_ADEN] {
            assert_eq!(t.count(1, order), 1);
        }
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
    }

    #[test]
    fn the_puncher_hunt_moves_to_cond_3_after_ten_kills() {
        let (mut t, kaien) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WARRIOR, vec![]);
        t.quest_page(1, kaien);
        accept(&mut t, 1, kaien);
        t.take_writes();
        assert!(t.event(1, NAME, "30623-08.html").contains("30623-08.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The other nine items are full (10 each), so the tenth puncher completes the step.
        give_all(&mut t, &PUNCHER_OTHERS, 10);
        for _ in 0..10 {
            let mob = t.spawn(PUNCHER, 40);
            t.kill(1, mob);
            t.world.delete_npc(mob);
        }
        assert_eq!(t.count(1, PUNCHERS_SHARD), 10);
        let mut expected = vec![rows::insert(NAME, "memoStateEx1", "1")];
        expected.extend((2..=10).map(|n| rows::update(NAME, "memoStateEx1", &n.to_string())));
        expected.push(rows::update(NAME, "cond", "3"));
        expected.push(rows::update(NAME, "memoStateEx1", "0"));
        assert_eq!(t.take_writes(), expected);
    }

    #[test]
    fn the_whole_step_one_ends_with_the_hand_in_and_the_mark() {
        let (mut t, kaien) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::WARRIOR, vec![]);
        accept(&mut t, 1, kaien);
        t.event(1, NAME, "30623-08.html");
        t.take_writes();

        // All ten step-1 items (100): Kaien shows page 13, which leads to the hand-in.
        give_all(&mut t, &[PUNCHERS_SHARD], 10);
        give_all(&mut t, &PUNCHER_OTHERS, 10);
        assert!(t.quest_page(1, kaien).contains("30623-13.html"));
        assert!(t.event(1, NAME, "30623-15.html").contains("30623-15.html"));
        assert!(t.event(1, NAME, "30623-16.html").contains("30623-16.html"));
        // Java takes the ten items and the five orders and gives the final order.
        assert_eq!(t.count(1, PUNCHERS_SHARD), 0);
        assert_eq!(t.count(1, ORDER_ADEN), 0);
        assert_eq!(t.count(1, FINAL_ORDER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "4"),
                // The first `setCond` also writes the completed step flags (`COMPLETED_STATE_FLAGS`).
                rows::insert(NAME, "__compltdStateFlags", "-2147483637"),
            ]
        );

        // The final step's items (15): the reward, the mark, the adena and the experience.
        give_all(
            &mut t,
            &[
                EXCUROS_SKIN,
                KRATORS_SHARD,
                LAKINS_MACE,
                GRANDIS_SKIN,
                TIMAK_ORCS_BELT,
            ],
            3,
        );
        t.take();
        t.talk(1, kaien);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(
            crate::testing::pages_to(&out, 1)
                .last()
                .unwrap()
                .contains("30623-18.html")
        );
        assert!(crate::testing::names_to(&out, 1).contains(&"SOCIAL_ACTION".to_owned()));
        assert_eq!(t.count(1, MARK_OF_DUELIST), 1);
        assert_eq!(t.count(1, 57), 161_806);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(
            t.quest_page(1, kaien)
                .contains("This quest has already been completed.")
        );
    }
}
