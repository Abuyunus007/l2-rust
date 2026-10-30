//! Quest 420: Little Wing (Cooper, the fairy stone, the drake eggs and the hatchling's flute).
// Java: dist/game/data/scripts/quests/Q00420_LittleWing/Q00420_LittleWing.java
//
// Pattern: a long multi-NPC chain. The branch ("fairy_stone" 1 = plain, 2 = deluxe), the previous
// branch ("old_stone") and the chosen drake ("drake_hunt") live in the quest variables.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const MARIA: i32 = 30608;
const CRONOS: i32 = 30610;
const BYRON: i32 = 30711;
const MIMYU: i32 = 30747;
const EXARION: i32 = 30748;
const ZWOV: i32 = 30749;
const KALIBRAN: i32 = 30750;
const SUZET: i32 = 30751;
const SHAMHAI: i32 = 30752;
const COOPER: i32 = 30829;

// Items
const COAL: i32 = 1870;
const CHARCOAL: i32 = 1871;
const SILVER_NUGGET: i32 = 1873;
const STONE_OF_PURITY: i32 = 1875;
const GEMSTONE_D: i32 = 2130;
const GEMSTONE_C: i32 = 2131;
const FAIRY_DUST: i32 = 3499;
const FAIRY_STONE: i32 = 3816;
const DELUXE_FAIRY_STONE: i32 = 3817;
const FAIRY_STONE_LIST: i32 = 3818;
const DELUXE_STONE_LIST: i32 = 3819;
const TOAD_SKIN: i32 = 3820;
const MONKSHOOD_JUICE: i32 = 3821;
const EXARION_SCALE: i32 = 3822;
const EXARION_EGG: i32 = 3823;
const ZWOV_SCALE: i32 = 3824;
const ZWOV_EGG: i32 = 3825;
const KALIBRAN_SCALE: i32 = 3826;
const KALIBRAN_EGG: i32 = 3827;
const SUZET_SCALE: i32 = 3828;
const SUZET_EGG: i32 = 3829;
const SHAMHAI_SCALE: i32 = 3830;
const SHAMHAI_EGG: i32 = 3831;

// Monsters
const DEAD_SEEKER: i32 = 20202;
const TOAD_LORD: i32 = 20231;
const MARSH_SPIDER: i32 = 20233;
const BREKA_OVERLORD: i32 = 20270;
const ROAD_SCAVENGER: i32 = 20551;
const LETO_WARRIOR: i32 = 20580;
const DELUXE_STONE_BREAKERS: [i32; 16] = [
    20589, // Fline
    20590, // Liele
    20591, // Valley Treant
    20592, // Satyr
    20593, // Unicorn
    20594, // Forest Runner
    20595, // Fline Elder
    20596, // Liele Elder
    20597, // Valley Treant Elder
    20598, // Satyr Elder
    20599, // Unicorn Elder
    27185, // Fairy Tree of Wind (Quest Monster)
    27186, // Fairy Tree of Star (Quest Monster)
    27187, // Fairy Tree of Twilight (Quest Monster)
    27188, // Fairy Tree of Abyss (Quest Monster)
    27189, // Soul of Tree Guardian (Quest Monster)
];

// Rewards
const DRAGONFLUTE_OF_WIND: i32 = 3500;
const DRAGONFLUTE_OF_STAR: i32 = 3501;
const DRAGONFLUTE_OF_TWILIGHT: i32 = 3502;
const HATCHLING_ARMOR: i32 = 3912;
const HATCHLING_FOOD: i32 = 4038;
const EGGS: [i32; 5] = [EXARION_EGG, SUZET_EGG, KALIBRAN_EGG, SHAMHAI_EGG, ZWOV_EGG];

/// Java `EGG_DROPS`.
fn egg_drop(npc_id: i32) -> Option<i32> {
    match npc_id {
        DEAD_SEEKER => Some(SHAMHAI_EGG),
        MARSH_SPIDER => Some(ZWOV_EGG),
        BREKA_OVERLORD => Some(SUZET_EGG),
        ROAD_SCAVENGER => Some(KALIBRAN_EGG),
        LETO_WARRIOR => Some(EXARION_EGG),
        _ => None,
    }
}

// Misc
const MIN_LEVEL: i32 = 35;

/// Java `Q00420_LittleWing`.
pub struct Q00420LittleWing;

impl Q00420LittleWing {
    /// Java `giveReward`: the flute for the first egg the player has; the egg is taken.
    fn give_reward(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        let random = ctx.get_random(100);
        for (index, &i) in EGGS.iter().enumerate() {
            if ctx.has_quest_items(player, i) {
                let mul = index as i32 * 5;
                if ctx.has_quest_items(player, FAIRY_DUST) {
                    if random < 45 + mul {
                        ctx.give_items(player, DRAGONFLUTE_OF_WIND, 1);
                    } else if random < 75 + mul {
                        ctx.give_items(player, DRAGONFLUTE_OF_STAR, 1);
                    } else {
                        ctx.give_items(player, DRAGONFLUTE_OF_TWILIGHT, 1);
                    }
                } else if random < 50 + mul {
                    ctx.give_items(player, DRAGONFLUTE_OF_WIND, 1);
                } else if random < 85 + mul {
                    ctx.give_items(player, DRAGONFLUTE_OF_STAR, 1);
                } else {
                    ctx.give_items(player, DRAGONFLUTE_OF_TWILIGHT, 1);
                }

                ctx.take_items(player, i, -1);
                break;
            }
        }
    }
}

impl Script for Q00420LittleWing {
    fn name(&self) -> &'static str {
        "Q00420_LittleWing"
    }

    fn path(&self) -> &'static str {
        "quests/Q00420_LittleWing"
    }

    fn id(&self) -> i32 {
        420
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[COOPER]);
        s.add_talk_id(&[
            MARIA, CRONOS, BYRON, MIMYU, EXARION, ZWOV, KALIBRAN, SUZET, SHAMHAI, COOPER,
        ]);
        s.add_attack_id(&DELUXE_STONE_BREAKERS);
        s.add_kill_id(&[
            TOAD_LORD,
            DEAD_SEEKER,
            MARSH_SPIDER,
            BREKA_OVERLORD,
            ROAD_SCAVENGER,
            LETO_WARRIOR,
        ]);
        s.register_quest_items(&[
            FAIRY_DUST,
            FAIRY_STONE,
            DELUXE_FAIRY_STONE,
            FAIRY_STONE_LIST,
            DELUXE_STONE_LIST,
            TOAD_SKIN,
            MONKSHOOD_JUICE,
            EXARION_SCALE,
            EXARION_EGG,
            ZWOV_SCALE,
            ZWOV_EGG,
            KALIBRAN_SCALE,
            KALIBRAN_EGG,
            SUZET_SCALE,
            SUZET_EGG,
            SHAMHAI_SCALE,
            SHAMHAI_EGG,
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
            "30610-02.html" | "30610-03.html" | "30610-04.html" | "30711-02.html" | "30747-05.html"
            | "30747-06.html" | "30751-02.html" => {
                htmltext = Some(event.to_owned());
            }
            "30829-02.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "30610-05.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    qs.set_int(ctx, "old_stone", 0);
                    qs.set_int(ctx, "fairy_stone", 1);
                    ctx.give_items(player, FAIRY_STONE_LIST, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30610-06.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    qs.set_int(ctx, "old_stone", 0);
                    qs.set_int(ctx, "fairy_stone", 2);
                    ctx.give_items(player, DELUXE_STONE_LIST, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30610-12.html" => {
                if qs.is_cond(ctx, 5) {
                    qs.set_cond(ctx, 2, true);
                    let old = qs.get_int(ctx, "fairy_stone");
                    qs.set_int(ctx, "old_stone", old);
                    qs.set_int(ctx, "fairy_stone", 1);
                    ctx.give_items(player, FAIRY_STONE_LIST, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30610-13.html" => {
                if qs.is_cond(ctx, 5) {
                    qs.set_cond(ctx, 2, true);
                    let old = qs.get_int(ctx, "fairy_stone");
                    qs.set_int(ctx, "old_stone", old);
                    qs.set_int(ctx, "fairy_stone", 2);
                    ctx.give_items(player, DELUXE_STONE_LIST, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30608-03.html" => {
                if qs.is_cond(ctx, 2) {
                    if qs.get_int(ctx, "fairy_stone") == 1
                        && ctx.get_quest_items_count(player, COAL) >= 10
                        && ctx.get_quest_items_count(player, CHARCOAL) >= 10
                        && ctx.get_quest_items_count(player, GEMSTONE_D) >= 1
                        && ctx.get_quest_items_count(player, SILVER_NUGGET) >= 3
                        && ctx.get_quest_items_count(player, TOAD_SKIN) >= 10
                    {
                        ctx.take_items(player, FAIRY_STONE_LIST, -1);
                        ctx.take_items(player, COAL, 10);
                        ctx.take_items(player, CHARCOAL, 10);
                        ctx.take_items(player, GEMSTONE_D, 1);
                        ctx.take_items(player, SILVER_NUGGET, 3);
                        ctx.take_items(player, TOAD_SKIN, -1);
                        ctx.give_items(player, FAIRY_STONE, 1);
                    }

                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30608-05.html" => {
                if qs.is_cond(ctx, 2) {
                    if qs.get_int(ctx, "fairy_stone") == 2
                        && ctx.get_quest_items_count(player, COAL) >= 10
                        && ctx.get_quest_items_count(player, CHARCOAL) >= 10
                        && ctx.get_quest_items_count(player, GEMSTONE_C) >= 1
                        && ctx.get_quest_items_count(player, STONE_OF_PURITY) >= 1
                        && ctx.get_quest_items_count(player, SILVER_NUGGET) >= 5
                        && ctx.get_quest_items_count(player, TOAD_SKIN) >= 20
                    {
                        ctx.take_items(player, DELUXE_STONE_LIST, -1);
                        ctx.take_items(player, COAL, 10);
                        ctx.take_items(player, CHARCOAL, 10);
                        ctx.take_items(player, GEMSTONE_C, 1);
                        ctx.take_items(player, STONE_OF_PURITY, 1);
                        ctx.take_items(player, SILVER_NUGGET, 5);
                        ctx.take_items(player, TOAD_SKIN, -1);
                        ctx.give_items(player, DELUXE_FAIRY_STONE, 1);
                    }

                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30711-03.html" => {
                if qs.is_cond(ctx, 3) {
                    qs.set_cond(ctx, 4, true);
                    if qs.get_int(ctx, "fairy_stone") == 2 {
                        htmltext = Some("30711-04.html".to_owned());
                    } else {
                        htmltext = Some(event.to_owned());
                    }
                }
            }
            "30747-02.html" | "30747-04.html" => {
                if qs.is_cond(ctx, 4)
                    && ctx.get_quest_items_count(player, FAIRY_STONE)
                        + ctx.get_quest_items_count(player, DELUXE_FAIRY_STONE)
                        > 0
                {
                    ctx.take_items_many(player, -1, &[FAIRY_STONE, DELUXE_FAIRY_STONE]);
                    if qs.get_int(ctx, "fairy_stone") == 2 {
                        ctx.give_items(player, FAIRY_DUST, 1);
                    }

                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30747-07.html" | "30747-08.html" => {
                if qs.is_cond(ctx, 5) && ctx.get_quest_items_count(player, MONKSHOOD_JUICE) == 0 {
                    ctx.give_items(player, MONKSHOOD_JUICE, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30747-12.html" => {
                if qs.is_cond(ctx, 7) {
                    if qs.get_int(ctx, "fairy_stone") == 1
                        || ctx.get_quest_items_count(player, FAIRY_DUST) == 0
                    {
                        Self::give_reward(ctx, player);
                        qs.exit_quest(ctx, true, true);
                        htmltext = Some("30747-16.html".to_owned());
                    } else {
                        qs.set_cond(ctx, 8, false);
                        htmltext = Some(event.to_owned());
                    }
                } else if qs.is_cond(ctx, 8) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30747-13.html" => {
                if qs.is_cond(ctx, 8) {
                    Self::give_reward(ctx, player);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30747-15.html" => {
                if qs.is_cond(ctx, 8) && ctx.get_quest_items_count(player, FAIRY_DUST) > 1 {
                    if ctx.get_random(100) < 5 {
                        ctx.give_items(player, HATCHLING_ARMOR, 1);
                        htmltext = Some("30747-14.html".to_owned());
                    } else {
                        ctx.give_items(player, HATCHLING_FOOD, 20);
                        htmltext = Some(event.to_owned());
                    }

                    Self::give_reward(ctx, player);
                    ctx.take_items(player, FAIRY_DUST, -1);
                    qs.exit_quest(ctx, true, true);
                }
            }
            "30748-02.html" => {
                if qs.is_cond(ctx, 5) {
                    ctx.take_items(player, MONKSHOOD_JUICE, -1);
                    ctx.give_items(player, EXARION_SCALE, 1);
                    qs.set_cond(ctx, 6, true);
                    qs.set_int(ctx, "drake_hunt", LETO_WARRIOR);
                    htmltext = Some(event.to_owned());
                }
            }
            "30749-02.html" => {
                if qs.is_cond(ctx, 5) {
                    ctx.take_items(player, MONKSHOOD_JUICE, -1);
                    ctx.give_items(player, ZWOV_SCALE, 1);
                    qs.set_cond(ctx, 6, true);
                    qs.set_int(ctx, "drake_hunt", MARSH_SPIDER);
                    htmltext = Some(event.to_owned());
                }
            }
            "30750-02.html" => {
                if qs.is_cond(ctx, 5) {
                    ctx.take_items(player, MONKSHOOD_JUICE, -1);
                    ctx.give_items(player, KALIBRAN_SCALE, 1);
                    qs.set_cond(ctx, 6, true);
                    qs.set_int(ctx, "drake_hunt", ROAD_SCAVENGER);
                    htmltext = Some(event.to_owned());
                }
            }
            "30750-05.html" => {
                if qs.is_cond(ctx, 6) && ctx.get_quest_items_count(player, KALIBRAN_EGG) >= 20 {
                    ctx.take_items_many(player, -1, &[KALIBRAN_SCALE, KALIBRAN_EGG]);
                    ctx.give_items(player, KALIBRAN_EGG, 1);
                    qs.set_cond(ctx, 7, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30751-03.html" => {
                if qs.is_cond(ctx, 5) {
                    ctx.take_items(player, MONKSHOOD_JUICE, -1);
                    ctx.give_items(player, SUZET_SCALE, 1);
                    qs.set_cond(ctx, 6, true);
                    qs.set_int(ctx, "drake_hunt", BREKA_OVERLORD);
                    htmltext = Some(event.to_owned());
                }
            }
            "30752-02.html" => {
                if qs.is_cond(ctx, 5) {
                    ctx.take_items(player, MONKSHOOD_JUICE, -1);
                    ctx.give_items(player, SHAMHAI_SCALE, 1);
                    qs.set_cond(ctx, 6, true);
                    qs.set_int(ctx, "drake_hunt", DEAD_SEEKER);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        let qs = ctx.get_quest_state(attacker, false);
        if qs.is_some()
            && ctx.get_quest_items_count(attacker, DELUXE_FAIRY_STONE) > 0
            && ctx.get_random(100) < 30
        {
            ctx.take_items(attacker, DELUXE_FAIRY_STONE, -1);
            ctx.play_sound(attacker, QuestSound::ItemsoundQuestMiddle);
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::TheStoneTheElvenStoneBroke,
                &[],
            );
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match qs.state(ctx) {
            CREATED => {
                if npc.id == COOPER {
                    htmltext = if ctx.player_level(talker) >= MIN_LEVEL {
                        "30829-01.htm"
                    } else {
                        "30829-03.html"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                COOPER => htmltext = "30829-04.html".to_owned(),
                CRONOS => match qs.get_cond(ctx) {
                    1 => htmltext = "30610-01.html".to_owned(),
                    2 => htmltext = "30610-07.html".to_owned(),
                    3 => {
                        if qs.get_int(ctx, "old_stone") > 0 {
                            htmltext = "30610-14.html".to_owned();
                        } else {
                            htmltext = "30610-08.html".to_owned();
                        }
                    }
                    4 => htmltext = "30610-09.html".to_owned(),
                    5 => {
                        if ctx.get_quest_items_count(talker, FAIRY_STONE) == 0
                            && ctx.get_quest_items_count(talker, DELUXE_FAIRY_STONE) == 0
                        {
                            htmltext = "30610-10.html".to_owned();
                        } else {
                            htmltext = "30610-11.html".to_owned();
                        }
                    }
                    _ => {}
                },
                MARIA => match qs.get_cond(ctx) {
                    2 => {
                        if qs.get_int(ctx, "fairy_stone") == 1
                            && ctx.get_quest_items_count(talker, COAL) >= 10
                            && ctx.get_quest_items_count(talker, CHARCOAL) >= 10
                            && ctx.get_quest_items_count(talker, GEMSTONE_D) >= 1
                            && ctx.get_quest_items_count(talker, SILVER_NUGGET) >= 3
                            && ctx.get_quest_items_count(talker, TOAD_SKIN) >= 10
                        {
                            htmltext = "30608-02.html".to_owned();
                        } else if qs.get_int(ctx, "fairy_stone") == 2
                            && ctx.get_quest_items_count(talker, COAL) >= 10
                            && ctx.get_quest_items_count(talker, CHARCOAL) >= 10
                            && ctx.get_quest_items_count(talker, GEMSTONE_C) >= 1
                            && ctx.get_quest_items_count(talker, STONE_OF_PURITY) >= 1
                            && ctx.get_quest_items_count(talker, SILVER_NUGGET) >= 5
                            && ctx.get_quest_items_count(talker, TOAD_SKIN) >= 20
                        {
                            htmltext = "30608-04.html".to_owned();
                        } else {
                            htmltext = "30608-01.html".to_owned();
                        }
                    }
                    3 => htmltext = "30608-06.html".to_owned(),
                    _ => {}
                },
                BYRON => match qs.get_cond(ctx) {
                    2 => htmltext = "30711-10.html".to_owned(),
                    3 => {
                        if qs.get_int(ctx, "old_stone") == 0 {
                            htmltext = "30711-01.html".to_owned();
                        } else if qs.get_int(ctx, "old_stone") == 1 {
                            qs.set_cond(ctx, 5, true);
                            htmltext = "30711-05.html".to_owned();
                        } else {
                            qs.set_cond(ctx, 4, true);
                            htmltext = "30711-06.html".to_owned();
                        }
                    }
                    4 => {
                        if ctx.get_quest_items_count(talker, FAIRY_STONE) == 0
                            && ctx.get_quest_items_count(talker, DELUXE_FAIRY_STONE) == 0
                        {
                            htmltext = "30711-09.html".to_owned();
                        } else if ctx.get_quest_items_count(talker, FAIRY_STONE) == 0 {
                            htmltext = "30711-08.html".to_owned();
                        } else {
                            htmltext = "30711-07.html".to_owned();
                        }
                    }
                    _ => {}
                },
                MIMYU => match qs.get_cond(ctx) {
                    4 => {
                        if ctx.get_quest_items_count(talker, FAIRY_STONE) > 0 {
                            htmltext = "30747-01.html".to_owned();
                        } else if ctx.get_quest_items_count(talker, DELUXE_FAIRY_STONE) > 0 {
                            htmltext = "30747-03.html".to_owned();
                        }
                    }
                    5 => {
                        if ctx.get_quest_items_count(talker, MONKSHOOD_JUICE) > 0 {
                            htmltext = "30747-09.html".to_owned();
                        } else if qs.get_int(ctx, "fairy_stone") == 1 {
                            htmltext = "30747-05.html".to_owned();
                        } else {
                            htmltext = "30747-06.html".to_owned();
                        }
                    }
                    6 => {
                        if ctx.get_quest_items_count(talker, EXARION_EGG) >= 20
                            || ctx.get_quest_items_count(talker, ZWOV_EGG) >= 20
                            || ctx.get_quest_items_count(talker, KALIBRAN_EGG) >= 20
                            || ctx.get_quest_items_count(talker, SUZET_EGG) >= 20
                            || ctx.get_quest_items_count(talker, SHAMHAI_EGG) >= 20
                        {
                            htmltext = "30747-10.html".to_owned();
                        } else {
                            htmltext = "30747-09.html".to_owned();
                        }
                    }
                    7 => htmltext = "30747-11.html".to_owned(),
                    8 => htmltext = "30747-12.html".to_owned(),
                    _ => {}
                },
                EXARION => match qs.get_cond(ctx) {
                    5 => {
                        if ctx.get_quest_items_count(talker, MONKSHOOD_JUICE) > 0 {
                            htmltext = "30748-01.html".to_owned();
                        }
                    }
                    6 => {
                        if ctx.get_quest_items_count(talker, EXARION_EGG) >= 20 {
                            ctx.take_items_many(talker, -1, &[EXARION_SCALE, EXARION_EGG]);
                            ctx.give_items(talker, EXARION_EGG, 1);
                            qs.set_cond(ctx, 7, true);
                            htmltext = "30748-04.html".to_owned();
                        } else {
                            htmltext = "30748-03.html".to_owned();
                        }
                    }
                    7 => htmltext = "30748-05.html".to_owned(),
                    _ => {}
                },
                ZWOV => match qs.get_cond(ctx) {
                    5 => {
                        if ctx.get_quest_items_count(talker, MONKSHOOD_JUICE) > 0 {
                            htmltext = "30749-01.html".to_owned();
                        }
                    }
                    6 => {
                        if ctx.get_quest_items_count(talker, ZWOV_EGG) >= 20 {
                            ctx.take_items_many(talker, -1, &[ZWOV_SCALE, ZWOV_EGG]);
                            ctx.give_items(talker, ZWOV_EGG, 1);
                            qs.set_cond(ctx, 7, true);
                            htmltext = "30749-04.html".to_owned();
                        } else {
                            htmltext = "30749-03.html".to_owned();
                        }
                    }
                    7 => htmltext = "30749-05.html".to_owned(),
                    _ => {}
                },
                KALIBRAN => match qs.get_cond(ctx) {
                    5 => {
                        if ctx.get_quest_items_count(talker, MONKSHOOD_JUICE) > 0 {
                            htmltext = "30750-01.html".to_owned();
                        }
                    }
                    6 => {
                        if ctx.get_quest_items_count(talker, KALIBRAN_EGG) >= 20 {
                            htmltext = "30750-04.html".to_owned();
                        } else {
                            htmltext = "30750-03.html".to_owned();
                        }
                    }
                    7 => htmltext = "30750-06.html".to_owned(),
                    _ => {}
                },
                SUZET => match qs.get_cond(ctx) {
                    5 => {
                        if ctx.get_quest_items_count(talker, MONKSHOOD_JUICE) > 0 {
                            htmltext = "30751-01.html".to_owned();
                        }
                    }
                    6 => {
                        if ctx.get_quest_items_count(talker, SUZET_EGG) >= 20 {
                            ctx.take_items_many(talker, -1, &[SUZET_SCALE, SUZET_EGG]);
                            ctx.give_items(talker, SUZET_EGG, 1);
                            qs.set_cond(ctx, 7, true);
                            htmltext = "30751-05.html".to_owned();
                        } else {
                            htmltext = "30751-04.html".to_owned();
                        }
                    }
                    7 => htmltext = "30751-06.html".to_owned(),
                    _ => {}
                },
                SHAMHAI => match qs.get_cond(ctx) {
                    5 => {
                        if ctx.get_quest_items_count(talker, MONKSHOOD_JUICE) > 0 {
                            htmltext = "30752-01.html".to_owned();
                        }
                    }
                    6 => {
                        if ctx.get_quest_items_count(talker, SHAMHAI_EGG) >= 20 {
                            ctx.take_items_many(talker, -1, &[SHAMHAI_SCALE, SHAMHAI_EGG]);
                            ctx.give_items(talker, SHAMHAI_EGG, 1);
                            qs.set_cond(ctx, 7, true);
                            htmltext = "30752-04.html".to_owned();
                        } else {
                            htmltext = "30752-03.html".to_owned();
                        }
                    }
                    7 => htmltext = "30752-05.html".to_owned(),
                    _ => {}
                },
                _ => {}
            },
            COMPLETED => htmltext = ctx.get_already_completed_msg(talker),
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.random_party_member_state(killer, -1, 3, Some(npc));
        if let Some(qs) = qs {
            if qs.is_cond(ctx, 2) && npc.id == TOAD_LORD {
                if qs.get_int(ctx, "fairy_stone") == 1 {
                    ctx.give_item_randomly(qs.player(), Some(npc), TOAD_SKIN, 1, 1, 10, 0.3, true);
                } else {
                    ctx.give_item_randomly(qs.player(), Some(npc), TOAD_SKIN, 1, 1, 20, 0.3, true);
                }
            } else if qs.is_cond(ctx, 6) && npc.id == qs.get_int(ctx, "drake_hunt") {
                if let Some(egg) = egg_drop(npc.id) {
                    ctx.give_item_randomly(qs.player(), Some(npc), egg, 1, 1, 20, 0.5, true);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00420_LittleWing";

    const NPCS: &str = r#"
        <npc id="30829" type="Folk" level="30" name="Cooper"/>
        <npc id="30610" type="Folk" level="30" name="Cronos"/>
        <npc id="30608" type="Folk" level="30" name="Maria"/>
        <npc id="30711" type="Folk" level="30" name="Byron"/>
        <npc id="30747" type="Folk" level="30" name="Mimyu"/>
        <npc id="30748" type="Folk" level="30" name="Exarion"/>
        <npc id="20231" type="Monster" level="30" name="Toad Lord"/>
        <npc id="20580" type="Monster" level="30" name="Leto Warrior"/>
        <npc id="20589" type="Monster" level="30" name="Fline"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1870" type="EtcItem" name="Coal"><set name="is_stackable" val="true"/></item>
        <item id="1871" type="EtcItem" name="Charcoal"><set name="is_stackable" val="true"/></item>
        <item id="1873" type="EtcItem" name="Silver Nugget"><set name="is_stackable" val="true"/></item>
        <item id="2130" type="EtcItem" name="Gemstone D"><set name="is_stackable" val="true"/></item>
        <item id="3499" type="EtcItem" name="Fairy Dust"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3500" type="EtcItem" name="Dragonflute of Wind"/>
        <item id="3501" type="EtcItem" name="Dragonflute of Star"/>
        <item id="3502" type="EtcItem" name="Dragonflute of Twilight"/>
        <item id="3816" type="EtcItem" name="Fairy Stone"><set name="is_questitem" val="true"/></item>
        <item id="3817" type="EtcItem" name="Deluxe Fairy Stone"><set name="is_questitem" val="true"/></item>
        <item id="3818" type="EtcItem" name="Fairy Stone List"><set name="is_questitem" val="true"/></item>
        <item id="3819" type="EtcItem" name="Deluxe Stone List"><set name="is_questitem" val="true"/></item>
        <item id="3820" type="EtcItem" name="Toad Skin"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3821" type="EtcItem" name="Monkshood Juice"><set name="is_questitem" val="true"/></item>
        <item id="3822" type="EtcItem" name="Exarion's Scale"><set name="is_questitem" val="true"/></item>
        <item id="3823" type="EtcItem" name="Exarion's Egg"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Vec<Oid>) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00420LittleWing);
        let pages = Pages::of(&Q00420LittleWing)
            .page("30829-01.htm", &["30829-02.htm"])
            .page("30610-01.html", &["30610-02.html"])
            .page("30610-02.html", &["30610-03.html"])
            .page("30610-03.html", &["30610-04.html"])
            .page("30610-04.html", &["30610-05.html", "30610-06.html"])
            .page("30608-02.html", &["30608-03.html"])
            .page("30711-01.html", &["30711-02.html"])
            .page("30711-02.html", &["30711-03.html"])
            .page("30747-01.html", &["30747-02.html"])
            .page("30747-02.html", &["30747-05.html"])
            .page("30747-05.html", &["30747-07.html"])
            .page("30748-01.html", &["30748-02.html"])
            .page("30747-11.html", &["30747-12.html"])
            .plain(&[
                "30829-02.htm",
                "30829-03.html",
                "30610-05.html",
                "30608-03.html",
                "30711-03.html",
                "30747-07.html",
                "30747-09.html",
                "30747-10.html",
                "30748-02.html",
                "30748-04.html",
                "30747-16.html",
            ])
            .npcs(&[30829, 30610, 30608, 30711, 30747, 30748]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = [30829, 30610, 30608, 30711, 30747, 30748]
            .iter()
            .enumerate()
            .map(|(i, id)| t.spawn(*id, 20 + 5 * i as i32))
            .collect();
        (t, npcs)
    }

    fn items(t: &mut ScriptTest, list: &[(i32, i64)]) {
        t.world.run_as_script(NAME, |ctx| {
            for (id, count) in list {
                ctx.give_items(Oid(1), *id, *count);
            }
        });
    }

    #[test]
    fn a_level_34_character_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 34, vec![]);
        assert!(t.quest_page(1, npcs[0]).contains("30829-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_plain_stone_to_the_flute() {
        let (mut t, npcs) = setup();
        let (cooper, cronos, maria, byron, mimyu, exarion) =
            (npcs[0], npcs[1], npcs[2], npcs[3], npcs[4], npcs[5]);
        t.enter(1, 40, vec![]);
        assert!(t.quest_page(1, cooper).contains("30829-01.htm"));
        t.event(1, NAME, "30829-02.htm");
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Cronos: the plain list (fairy_stone 1).
        assert!(t.quest_page(1, cronos).contains("30610-01.html"));
        t.event(1, NAME, "30610-02.html");
        t.event(1, NAME, "30610-03.html");
        t.event(1, NAME, "30610-04.html");
        t.event(1, NAME, "30610-05.html");
        assert_eq!(t.count(1, FAIRY_STONE_LIST), 1);
        t.take_writes();

        // Ten toad skins (limit 10, 30%).
        for _ in 0..300 {
            let toad = t.spawn(TOAD_LORD, 60);
            t.kill(1, toad);
            if t.count(1, TOAD_SKIN) >= 10 {
                break;
            }
        }
        assert_eq!(t.count(1, TOAD_SKIN), 10);
        items(
            &mut t,
            &[(COAL, 10), (CHARCOAL, 10), (GEMSTONE_D, 1), (SILVER_NUGGET, 3)],
        );

        // Maria makes the stone, Byron sends on, Mimyu takes it.
        assert!(t.quest_page(1, maria).contains("30608-02.html"));
        t.event(1, NAME, "30608-03.html");
        assert_eq!((t.count(1, FAIRY_STONE), t.count(1, FAIRY_STONE_LIST)), (1, 0));
        assert_eq!(t.count(1, TOAD_SKIN), 0);
        assert!(t.quest_page(1, byron).contains("30711-01.html"));
        t.event(1, NAME, "30711-02.html");
        t.event(1, NAME, "30711-03.html");
        assert!(t.quest_page(1, mimyu).contains("30747-01.html"));
        t.event(1, NAME, "30747-02.html");
        assert_eq!(t.count(1, FAIRY_STONE), 0);
        t.event(1, NAME, "30747-05.html");
        t.event(1, NAME, "30747-07.html");
        assert_eq!(t.count(1, MONKSHOOD_JUICE), 1);

        // Exarion: the scale; the Leto Warriors drop 20 eggs (50%).
        assert!(t.quest_page(1, exarion).contains("30748-01.html"));
        t.event(1, NAME, "30748-02.html");
        assert_eq!((t.count(1, EXARION_SCALE), t.count(1, MONKSHOOD_JUICE)), (1, 0));
        for _ in 0..300 {
            let warrior = t.spawn(LETO_WARRIOR, 60);
            t.kill(1, warrior);
            if t.count(1, EXARION_EGG) >= 20 {
                break;
            }
        }
        assert_eq!(t.count(1, EXARION_EGG), 20);
        assert!(t.quest_page(1, mimyu).contains("30747-10.html"));
        assert!(t.quest_page(1, exarion).contains("30748-04.html"));
        assert_eq!((t.count(1, EXARION_EGG), t.count(1, EXARION_SCALE)), (1, 0));

        // Mimyu: the flute (no fairy dust: the plain branch), the quest ends repeatable.
        t.take_writes();
        assert!(t.quest_page(1, mimyu).contains("30747-11.html"));
        t.event(1, NAME, "30747-12.html");
        let flutes: i64 = [3500, 3501, 3502].iter().map(|id| t.count(1, *id)).sum();
        assert_eq!(flutes, 1);
        assert_eq!(t.count(1, EXARION_EGG), 0);
        let writes = t.take_writes();
        assert_eq!(writes.last(), Some(&rows::delete(NAME, true)));
    }

    #[test]
    fn the_deluxe_stone_breaks_on_a_hit_sometimes() {
        let (mut t, npcs) = setup();
        t.enter(1, 40, vec![]);
        t.quest_page(1, npcs[0]);
        t.event(1, NAME, "30829-02.htm");
        items(&mut t, &[(DELUXE_FAIRY_STONE, 1)]);
        assert_eq!(t.count(1, DELUXE_FAIRY_STONE), 1);
        let fline = t.spawn(20589, 60);
        let npc = ScriptNpc {
            object_id: fline,
            id: 20589,
        };
        // 30% per hit: a few hits break it.
        for _ in 0..200 {
            t.world.run_as_script(NAME, |ctx| {
                Q00420LittleWing.on_attack(ctx, npc, Oid(1), 10, false, None);
            });
            if t.count(1, DELUXE_FAIRY_STONE) == 0 {
                return;
            }
        }
        panic!("the stone never broke");
    }
}
