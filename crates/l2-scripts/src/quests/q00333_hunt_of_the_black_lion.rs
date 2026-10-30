//! Quest 333: Hunt of the Black Lion (Sophya hands out four orders; the drops are paid in adena
//! and Lion's Claws, the cargo boxes go to Reedfoot and Morgon, the statue and tablet pieces to
//! Rupio, Undrias and Lockirin).
// Java: dist/game/data/scripts/quests/Q00333_HuntOfTheBlackLion/Q00333_HuntOfTheBlackLion.java
//
// Pattern: a repeatable quest with orders (items that switch the kill table), the memo state as a
// counter, and several random rolls per event (`docs/design/scripts.md`, "Quest patterns").

use l2_util::ObjectId;
use l2_world::script::{QuestSound, STARTED};
use l2_world::subclass::CategoryType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ABYSSAL_CELEBRANT_UNDRIAS: i32 = 30130;
const BLACKSMITH_RUPIO: i32 = 30471;
const IRON_GATES_LOCKIRIN: i32 = 30531;
const MERCENARY_CAPTAIN_SOPHYA: i32 = 30735;
const MERCENARY_REEDFOOT: i32 = 30736;
const GUILDSMAN_MORGON: i32 = 30737;

// Items
const BLACK_LION_MARK: i32 = 1369;
const CARGO_BOX_1ST: i32 = 3440;
const CARGO_BOX_2ND: i32 = 3441;
const CARGO_BOX_3RD: i32 = 3442;
const CARGO_BOX_4TH: i32 = 3443;
const STATUE_OF_SHILEN_HEAD: i32 = 3457;
const STATUE_OF_SHILEN_TORSO: i32 = 3458;
const STATUE_OF_SHILEN_ARM: i32 = 3459;
const STATUE_OF_SHILEN_LEG: i32 = 3460;
const COMPLETE_STATUE_OF_SHILEN: i32 = 3461;
const FRAGMENT_OF_ANCIENT_TABLET_1ST_PIECE: i32 = 3462;
const FRAGMENT_OF_ANCIENT_TABLET_2ND_PIECE: i32 = 3463;
const FRAGMENT_OF_ANCIENT_TABLET_3RD_PIECE: i32 = 3464;
const FRAGMENT_OF_ANCIENT_TABLET_4TH_PIECE: i32 = 3465;
const COMPLETE_ANCIENT_TABLET: i32 = 3466;
const SOPHYAS_1ST_ORDER: i32 = 3671;
const SOPHYAS_2ND_ORDER: i32 = 3672;
const SOPHYAS_3RD_ORDER: i32 = 3673;
const SOPHYAS_4TH_ORDER: i32 = 3674;
const LIONS_CLAW: i32 = 3675;
const LIONS_EYE: i32 = 3676;
const GUILD_COIN: i32 = 3677;
const UNDEAD_ASH: i32 = 3848;
const BLOODY_AXE_INSIGNIA: i32 = 3849;
const DELU_LIZARDMAN_FANG: i32 = 3850;
const STAKATO_TALON: i32 = 3851;

// Rewards
const ALACRITY_POTION: i32 = 735;
const SCROL_OF_ESCAPE: i32 = 736;
const HELING_POTION: i32 = 1061;
const SOULSHOT_D_GRADE: i32 = 1463;
const SPIRITSHOT_D_GRADE: i32 = 2510;
const GLUDIO_APPLES: i32 = 3444;
const DION_CORN_MEAL: i32 = 3445;
const DIRE_WOLF_PELTS: i32 = 3446;
const MOONSTONE: i32 = 3447;
const GLUDIO_WHEAT_FLOUR: i32 = 3448;
const SPIDERSILK_ROPE: i32 = 3449;
const ALEXANDRITE: i32 = 3450;
const SILVER_TEA_SERVICE: i32 = 3451;
const MECHANIC_GOLEM_SPACE_PARTS: i32 = 3452;
const FIRE_EMERALD: i32 = 3453;
const AVELLAN_SILK_FROCK: i32 = 3454;
const FERIOTIC_PORCELAIN_URM: i32 = 3455;
const IMPERIAL_DIAMOND: i32 = 3456;

// Monster
const MARSH_STAKATO: i32 = 20157;
const NEER_CRAWLER: i32 = 20160;
const SPECTER: i32 = 20171;
const SORROW_MAIDEN: i32 = 20197;
const NEER_CRAWLER_BERSERKER: i32 = 20198;
const STRAIN: i32 = 20200;
const GHOUL: i32 = 20201;
const OL_MAHUM_GUERILLA: i32 = 20207;
const OL_MAHUM_RAIDER: i32 = 20208;
const OL_MAHUM_MARKSMAN: i32 = 20209;
const OL_MAHUM_SERGEANT: i32 = 20210;
const OL_MAHUM_CAPTAIN: i32 = 20211;
const MARSH_STAKATO_WORKER: i32 = 20230;
const MARSH_STAKATO_SOLDIER: i32 = 20232;
const MARSH_STAKATO_DRONE: i32 = 20234;
const DELU_LIZARDMAN: i32 = 20251;
const DELU_LIZARDMAN_SCOUT: i32 = 20252;
const DELU_LIZARDMAN_WARRIOR: i32 = 20253;

// Quest Monster
const DELU_LIZARDMAN_HEADHUNTER: i32 = 27151;
const MARSH_STAKATO_MARQUESS: i32 = 27152;

// Misc
const MIN_LEVEL: i32 = 25;
const ADENA_ID: i32 = 57;

const CARGO_BOXES: [i32; 4] = [CARGO_BOX_1ST, CARGO_BOX_2ND, CARGO_BOX_3RD, CARGO_BOX_4TH];
const ORDERS: [i32; 4] = [
    SOPHYAS_1ST_ORDER,
    SOPHYAS_2ND_ORDER,
    SOPHYAS_3RD_ORDER,
    SOPHYAS_4TH_ORDER,
];
const TROPHIES: [i32; 4] = [
    UNDEAD_ASH,
    BLOODY_AXE_INSIGNIA,
    DELU_LIZARDMAN_FANG,
    STAKATO_TALON,
];
const STATUE_PARTS: [i32; 4] = [
    STATUE_OF_SHILEN_HEAD,
    STATUE_OF_SHILEN_TORSO,
    STATUE_OF_SHILEN_ARM,
    STATUE_OF_SHILEN_LEG,
];
const TABLET_PARTS: [i32; 4] = [
    FRAGMENT_OF_ANCIENT_TABLET_1ST_PIECE,
    FRAGMENT_OF_ANCIENT_TABLET_2ND_PIECE,
    FRAGMENT_OF_ANCIENT_TABLET_3RD_PIECE,
    FRAGMENT_OF_ANCIENT_TABLET_4TH_PIECE,
];

/// The sum of the counts of `items`.
fn sum(ctx: &ScriptCtx<'_>, player: ObjectId, items: &[i32]) -> i64 {
    items.iter().map(|&i| ctx.get_quest_items_count(player, i)).sum()
}

/// `if (hasQuestItems(CARGO_BOX_1ST)) take 1 else if (2ND) ...`: the first box the player has.
fn take_first_cargo_box(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    for box_item in CARGO_BOXES {
        if ctx.has_quest_items(player, box_item) {
            ctx.take_items(player, box_item, 1);
            return;
        }
    }
}

/// The first branch of the "lion's eye" reward roll (`chance` 0..99): the potion, the shots of
/// the player's group, the scroll or the potion of alacrity.
fn lions_eye_reward(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    chance: i32,
    (potion, soulshot, spiritshot, scroll, alacrity): (i64, i64, i64, i64, i64),
) {
    if chance < 25 {
        ctx.give_items(player, HELING_POTION, potion);
    } else if chance < 50 {
        if ctx.world().is_in_category(player, CategoryType::FighterGroup) {
            ctx.give_items(player, SOULSHOT_D_GRADE, soulshot);
        } else if ctx.world().is_in_category(player, CategoryType::MageGroup) {
            ctx.give_items(player, SPIRITSHOT_D_GRADE, spiritshot);
        }
    } else if chance < 75 {
        ctx.give_items(player, SCROL_OF_ESCAPE, scroll);
    } else {
        ctx.give_items(player, ALACRITY_POTION, alacrity);
    }
}

/// One kill row: the trophy (`half`: `getRandom(2) == 0`, else `getRandom(100) < chance`) and
/// the cargo box (`getRandom(100) < box_chance`), when the player holds `order`.
fn trophy_and_box(
    ctx: &mut ScriptCtx<'_>,
    killer: ObjectId,
    order: i32,
    (trophy, trophy_chance): (i32, Option<i32>),
    (box_item, box_chance): (i32, i32),
) {
    if !ctx.has_quest_items(killer, order) {
        return;
    }
    let hit = match trophy_chance {
        None => ctx.get_random(2) == 0,
        Some(chance) => ctx.get_random(100) < chance,
    };
    if hit {
        ctx.give_items(killer, trophy, 1);
    }
    if ctx.get_random(100) < box_chance {
        ctx.give_items(killer, box_item, 1);
    }
}

/// A marsh stakato row: `trophy_and_box` and then the 2% marquess (`addSpawn(.., npc, true, 0,
/// false)`).
fn stakato(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, talon_chance: i32, box_chance: i32) {
    if ctx.has_quest_items(killer, SOPHYAS_4TH_ORDER) {
        trophy_and_box(
            ctx,
            killer,
            SOPHYAS_4TH_ORDER,
            (STAKATO_TALON, Some(talon_chance)),
            (CARGO_BOX_4TH, box_chance),
        );
        if ctx.get_random(100) < 2 && ctx.has_quest_items(killer, SOPHYAS_4TH_ORDER) {
            ctx.add_spawn_at(MARSH_STAKATO_MARQUESS, npc.object_id, true, 0);
        }
    }
}

/// A Delu lizardman row: the fang and box, then the 3% pair of headhunters.
fn delu(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, box_chance: i32) {
    trophy_and_box(
        ctx,
        killer,
        SOPHYAS_3RD_ORDER,
        (DELU_LIZARDMAN_FANG, None),
        (CARGO_BOX_3RD, box_chance),
    );
    if ctx.get_random(100) < 3 && ctx.has_quest_items(killer, SOPHYAS_3RD_ORDER) {
        ctx.add_spawn_at(DELU_LIZARDMAN_HEADHUNTER, npc.object_id, true, 0);
        ctx.add_spawn_at(DELU_LIZARDMAN_HEADHUNTER, npc.object_id, true, 0);
    }
}

/// The Lion's Claw for the number of trophies (`itemcount < 20` gives none).
fn lions_claw_for(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let itemcount = sum(ctx, player, &TROPHIES);
    if itemcount < 20 {
        // Do nothing.
    } else if itemcount < 50 {
        ctx.give_items(player, LIONS_CLAW, 1);
    } else if itemcount < 100 {
        ctx.give_items(player, LIONS_CLAW, 2);
    } else {
        ctx.give_items(player, LIONS_CLAW, 3);
    }
}

/// Java `Q00333_HuntOfTheBlackLion`.
pub struct Q00333HuntOfTheBlackLion;

impl Script for Q00333HuntOfTheBlackLion {
    fn name(&self) -> &'static str {
        "Q00333_HuntOfTheBlackLion"
    }

    fn path(&self) -> &'static str {
        "quests/Q00333_HuntOfTheBlackLion"
    }

    fn id(&self) -> i32 {
        333
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MERCENARY_CAPTAIN_SOPHYA]);
        s.add_talk_id(&[
            MERCENARY_CAPTAIN_SOPHYA,
            ABYSSAL_CELEBRANT_UNDRIAS,
            BLACKSMITH_RUPIO,
            IRON_GATES_LOCKIRIN,
            MERCENARY_REEDFOOT,
            GUILDSMAN_MORGON,
        ]);
        s.add_kill_id(&[
            MARSH_STAKATO,
            NEER_CRAWLER,
            SPECTER,
            SORROW_MAIDEN,
            NEER_CRAWLER_BERSERKER,
            STRAIN,
            GHOUL,
            OL_MAHUM_GUERILLA,
            OL_MAHUM_RAIDER,
            OL_MAHUM_MARKSMAN,
            OL_MAHUM_SERGEANT,
            OL_MAHUM_CAPTAIN,
            MARSH_STAKATO_WORKER,
            MARSH_STAKATO_SOLDIER,
            MARSH_STAKATO_DRONE,
            DELU_LIZARDMAN,
            DELU_LIZARDMAN_SCOUT,
            DELU_LIZARDMAN_WARRIOR,
            DELU_LIZARDMAN_HEADHUNTER,
            MARSH_STAKATO_MARQUESS,
        ]);
        s.register_quest_items(&[
            BLACK_LION_MARK,
            CARGO_BOX_1ST,
            CARGO_BOX_2ND,
            CARGO_BOX_3RD,
            CARGO_BOX_4TH,
            STATUE_OF_SHILEN_HEAD,
            STATUE_OF_SHILEN_TORSO,
            STATUE_OF_SHILEN_ARM,
            STATUE_OF_SHILEN_LEG,
            COMPLETE_STATUE_OF_SHILEN,
            FRAGMENT_OF_ANCIENT_TABLET_1ST_PIECE,
            FRAGMENT_OF_ANCIENT_TABLET_2ND_PIECE,
            FRAGMENT_OF_ANCIENT_TABLET_3RD_PIECE,
            FRAGMENT_OF_ANCIENT_TABLET_4TH_PIECE,
            COMPLETE_ANCIENT_TABLET,
            SOPHYAS_1ST_ORDER,
            SOPHYAS_2ND_ORDER,
            SOPHYAS_3RD_ORDER,
            SOPHYAS_4TH_ORDER,
            LIONS_CLAW,
            LIONS_EYE,
            GUILD_COIN,
            UNDEAD_ASH,
            BLOODY_AXE_INSIGNIA,
            DELU_LIZARDMAN_FANG,
            STAKATO_TALON,
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

        // NOTE(java-quirk): both rolls are made for every event, before the switch.
        let chance = ctx.get_random(100);
        let chance1 = ctx.get_random(100);

        let mut htmltext = None;
        let event_page = Some(event.to_owned());
        match event {
            "30735-04.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = event_page;
                }
            }
            "30735-05.html" | "30735-06.html" | "30735-07.html" | "30735-08.html" | "30735-09.html"
            | "30130-05.html" | "30531-05.html" | "30735-21.html" | "30735-24a.html" | "30735-25b.html"
            | "30736-06.html" | "30736-09.html" | "30737-07.html" => {
                htmltext = event_page;
            }
            "30735-10.html" => {
                if !ctx.has_quest_items(player, SOPHYAS_1ST_ORDER) {
                    ctx.give_items(player, SOPHYAS_1ST_ORDER, 1);
                    htmltext = event_page;
                }
            }
            "30735-11.html" => {
                if !ctx.has_quest_items(player, SOPHYAS_2ND_ORDER) {
                    ctx.give_items(player, SOPHYAS_2ND_ORDER, 1);
                    htmltext = event_page;
                }
            }
            "30735-12.html" => {
                if !ctx.has_quest_items(player, SOPHYAS_3RD_ORDER) {
                    ctx.give_items(player, SOPHYAS_3RD_ORDER, 1);
                    htmltext = event_page;
                }
            }
            "30735-13.html" => {
                if !ctx.has_quest_items(player, SOPHYAS_4TH_ORDER) {
                    ctx.give_items(player, SOPHYAS_4TH_ORDER, 1);
                    htmltext = event_page;
                }
            }
            "30735-16.html" => {
                let claws = ctx.get_quest_items_count(player, LIONS_CLAW);
                let eyes = ctx.get_quest_items_count(player, LIONS_EYE);
                if claws < 10 {
                    htmltext = event_page;
                } else if claws >= 10 && eyes < 4 {
                    ctx.give_items(player, LIONS_EYE, 1);
                    lions_eye_reward(ctx, player, chance, (20, 100, 50, 20, 3));
                    ctx.take_items(player, LIONS_CLAW, 10);
                    htmltext = Some("30735-17a.html".to_owned());
                } else if claws >= 10 && (4..=7).contains(&eyes) {
                    ctx.give_items(player, LIONS_EYE, 1);
                    lions_eye_reward(ctx, player, chance, (25, 200, 100, 20, 3));
                    ctx.take_items(player, LIONS_CLAW, 10);
                    htmltext = Some("30735-18b.html".to_owned());
                } else if claws >= 10 && eyes >= 8 {
                    ctx.take_items(player, LIONS_EYE, 8);
                    lions_eye_reward(ctx, player, chance, (50, 400, 200, 30, 4));
                    ctx.take_items(player, LIONS_CLAW, 10);
                    htmltext = Some("30735-19b.html".to_owned());
                }
            }
            "30735-20.html" => {
                for order in ORDERS {
                    ctx.take_items(player, order, -1);
                }
                htmltext = event_page;
            }
            "30735-26.html" => {
                if ctx.has_quest_items(player, BLACK_LION_MARK) {
                    ctx.give_adena(player, 12400, true);
                    qs.exit_quest(ctx, true, true);
                    htmltext = event_page;
                }
            }
            "30130-04.html" => {
                if ctx.has_quest_items(player, COMPLETE_STATUE_OF_SHILEN) {
                    ctx.give_adena(player, 30000, true);
                    ctx.take_items(player, COMPLETE_STATUE_OF_SHILEN, 1);
                    htmltext = event_page;
                }
            }
            "30471-03.html" => {
                if !ctx.has_all_quest_items(player, &STATUE_PARTS) {
                    htmltext = event_page;
                } else if ctx.get_random(100) < 50 {
                    ctx.give_items(player, COMPLETE_STATUE_OF_SHILEN, 1);
                    for part in STATUE_PARTS {
                        ctx.take_items(player, part, 1);
                    }
                    htmltext = Some("30471-04.html".to_owned());
                } else {
                    for part in STATUE_PARTS {
                        ctx.take_items(player, part, 1);
                    }
                    htmltext = Some("30471-05.html".to_owned());
                }
            }
            "30471-06.html" => {
                if !ctx.has_all_quest_items(player, &TABLET_PARTS) {
                    htmltext = event_page;
                } else if ctx.get_random(100) < 50 {
                    ctx.give_items(player, COMPLETE_ANCIENT_TABLET, 1);
                    for part in TABLET_PARTS {
                        ctx.take_items(player, part, 1);
                    }
                    htmltext = Some("30471-07.html".to_owned());
                } else {
                    for part in TABLET_PARTS {
                        ctx.take_items(player, part, 1);
                    }
                    htmltext = Some("30471-08.html".to_owned());
                }
            }
            "30531-04.html" => {
                if ctx.has_quest_items(player, COMPLETE_ANCIENT_TABLET) {
                    ctx.give_adena(player, 30000, true);
                    ctx.take_items(player, COMPLETE_ANCIENT_TABLET, 1);
                    htmltext = event_page;
                }
            }
            "30736-03.html" => {
                let adena = ctx.get_quest_items_count(player, ADENA_ID);
                let boxes = sum(ctx, player, &CARGO_BOXES);
                if adena < 650 && boxes >= 1 {
                    htmltext = event_page;
                } else if adena >= 650 && boxes >= 1 {
                    ctx.take_items(player, ADENA_ID, 650);
                    take_first_cargo_box(ctx, player);

                    if chance < 40 {
                        let (item, page) = if chance1 < 33 {
                            (GLUDIO_APPLES, "30736-04a.html")
                        } else if chance1 < 66 {
                            (DION_CORN_MEAL, "30736-04b.html")
                        } else {
                            (DIRE_WOLF_PELTS, "30736-04c.html")
                        };
                        ctx.give_items(player, item, 1);
                        htmltext = Some(page.to_owned());
                    } else if chance < 60 {
                        let (item, page) = if chance1 < 33 {
                            (MOONSTONE, "30736-04d.html")
                        } else if chance1 < 66 {
                            (GLUDIO_WHEAT_FLOUR, "30736-04e.html")
                        } else {
                            (SPIDERSILK_ROPE, "30736-04f.html")
                        };
                        ctx.give_items(player, item, 1);
                        htmltext = Some(page.to_owned());
                    } else if chance < 70 {
                        let (item, page) = if chance1 < 33 {
                            (ALEXANDRITE, "30736-04g.html")
                        } else if chance1 < 66 {
                            (SILVER_TEA_SERVICE, "30736-04h.html")
                        } else {
                            (MECHANIC_GOLEM_SPACE_PARTS, "30736-04i.html")
                        };
                        ctx.give_items(player, item, 1);
                        htmltext = Some(page.to_owned());
                    } else if chance < 75 {
                        let (item, page) = if chance1 < 33 {
                            (FIRE_EMERALD, "30736-04j.html")
                        } else if chance1 < 66 {
                            (AVELLAN_SILK_FROCK, "30736-04k.html")
                        } else {
                            (FERIOTIC_PORCELAIN_URM, "30736-04l.html")
                        };
                        ctx.give_items(player, item, 1);
                        htmltext = Some(page.to_owned());
                    } else if chance < 76 {
                        ctx.give_items(player, IMPERIAL_DIAMOND, 1);
                        htmltext = Some("30736-04m.html".to_owned());
                    } else if ctx.get_random(100) < 50 {
                        let part = if chance1 < 25 {
                            STATUE_OF_SHILEN_HEAD
                        } else if chance1 < 50 {
                            STATUE_OF_SHILEN_TORSO
                        } else if chance1 < 75 {
                            STATUE_OF_SHILEN_ARM
                        } else {
                            STATUE_OF_SHILEN_LEG
                        };
                        ctx.give_items(player, part, 1);
                        htmltext = Some("30736-04n.html".to_owned());
                    } else {
                        let part = if chance1 < 25 {
                            FRAGMENT_OF_ANCIENT_TABLET_1ST_PIECE
                        } else if chance1 < 50 {
                            FRAGMENT_OF_ANCIENT_TABLET_2ND_PIECE
                        } else if chance1 < 75 {
                            FRAGMENT_OF_ANCIENT_TABLET_3RD_PIECE
                        } else {
                            FRAGMENT_OF_ANCIENT_TABLET_4TH_PIECE
                        };
                        ctx.give_items(player, part, 1);
                        htmltext = Some("30736-04o.html".to_owned());
                    }
                } else if boxes < 1 {
                    htmltext = Some("30736-05.html".to_owned());
                }
            }
            "30736-07.html" => {
                let memo = i64::from(qs.get_memo_state(ctx));
                if ctx.adena_of(player) < 200 + memo * 200 {
                    htmltext = event_page;
                } else if memo * 100 > 200 {
                    htmltext = Some("30736-08.html".to_owned());
                } else {
                    // Twenty pages "30736-08a".."30736-08t", five points of the roll each.
                    let letter = char::from(b'a' + u8::try_from((chance / 5).min(19)).unwrap_or(19));
                    htmltext = Some(format!("30736-08{letter}.html"));

                    ctx.take_items(player, ADENA_ID, 200 + memo * 200);
                    qs.set_memo_state(ctx, qs.get_memo_state(ctx) + 1);
                }
            }
            "30737-06.html" => {
                if sum(ctx, player, &CARGO_BOXES) < 1 {
                    htmltext = event_page;
                } else {
                    take_first_cargo_box(ctx, player);

                    if ctx.get_quest_items_count(player, GUILD_COIN) < 80 {
                        ctx.give_items(player, GUILD_COIN, 1);
                    } else {
                        ctx.take_items(player, GUILD_COIN, 80);
                    }

                    let coins = ctx.get_quest_items_count(player, GUILD_COIN);
                    if coins < 40 {
                        ctx.give_adena(player, 100, true);
                        htmltext = Some("30737-03.html".to_owned());
                    } else if (40..80).contains(&coins) {
                        ctx.give_adena(player, 200, true);
                        htmltext = Some("30737-04.html".to_owned());
                    } else {
                        ctx.give_adena(player, 300, true);
                        htmltext = Some("30737-05.html".to_owned());
                    }
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !(qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true))
        {
            return;
        }
        let first = |ctx: &mut ScriptCtx<'_>, half: Option<i32>, box_chance: i32| {
            trophy_and_box(
                ctx,
                killer,
                SOPHYAS_1ST_ORDER,
                (UNDEAD_ASH, half),
                (CARGO_BOX_1ST, box_chance),
            );
        };
        let second = |ctx: &mut ScriptCtx<'_>, box_chance: i32| {
            trophy_and_box(
                ctx,
                killer,
                SOPHYAS_2ND_ORDER,
                (BLOODY_AXE_INSIGNIA, None),
                (CARGO_BOX_2ND, box_chance),
            );
        };
        match npc.id {
            MARSH_STAKATO => stakato(ctx, npc, killer, 55, 12),
            NEER_CRAWLER => first(ctx, None, 11),
            SPECTER => first(ctx, Some(60), 8),
            SORROW_MAIDEN => first(ctx, Some(60), 9),
            NEER_CRAWLER_BERSERKER => first(ctx, None, 12),
            STRAIN => first(ctx, None, 13),
            GHOUL => first(ctx, None, 15),
            OL_MAHUM_GUERILLA => second(ctx, 9),
            OL_MAHUM_RAIDER => second(ctx, 10),
            OL_MAHUM_MARKSMAN => second(ctx, 11),
            OL_MAHUM_SERGEANT => second(ctx, 12),
            OL_MAHUM_CAPTAIN => second(ctx, 13),
            MARSH_STAKATO_WORKER => stakato(ctx, npc, killer, 60, 13),
            MARSH_STAKATO_SOLDIER => stakato(ctx, npc, killer, 56, 14),
            MARSH_STAKATO_DRONE => stakato(ctx, npc, killer, 60, 15),
            DELU_LIZARDMAN | DELU_LIZARDMAN_SCOUT => delu(ctx, npc, killer, 14),
            DELU_LIZARDMAN_WARRIOR => delu(ctx, npc, killer, 15),
            DELU_LIZARDMAN_HEADHUNTER => {
                if ctx.has_quest_items(killer, SOPHYAS_3RD_ORDER) {
                    ctx.give_items(killer, DELU_LIZARDMAN_FANG, 4);
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                }
            }
            MARSH_STAKATO_MARQUESS => {
                if ctx.has_quest_items(killer, SOPHYAS_4TH_ORDER) {
                    ctx.give_items(killer, STAKATO_TALON, 8);
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            if npc.id == MERCENARY_CAPTAIN_SOPHYA {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "30735-01.htm"
                } else if !ctx.has_quest_items(player, BLACK_LION_MARK) {
                    "30735-02.htm"
                } else {
                    "30735-03.htm"
                }
                .to_owned();
            }
        } else if qs.state(ctx) == STARTED {
            match npc.id {
                MERCENARY_CAPTAIN_SOPHYA => {
                    let orders = sum(ctx, player, &ORDERS);
                    let trophies = sum(ctx, player, &TROPHIES);
                    let boxes = sum(ctx, player, &CARGO_BOXES);
                    if orders == 0 {
                        htmltext = "30735-14.html".to_owned();
                    } else if orders == 1 && trophies < 1 && boxes < 1 {
                        htmltext = "30735-15.html".to_owned();
                    } else if orders == 1 && trophies < 1 && boxes >= 1 {
                        htmltext = "30735-15a.html".to_owned();
                    } else if orders == 1 && trophies >= 1 && boxes == 0 {
                        lions_claw_for(ctx, player);

                        let ash = ctx.get_quest_items_count(player, UNDEAD_ASH);
                        let insignia = ctx.get_quest_items_count(player, BLOODY_AXE_INSIGNIA);
                        let fang = ctx.get_quest_items_count(player, DELU_LIZARDMAN_FANG);
                        let talon = ctx.get_quest_items_count(player, STAKATO_TALON);
                        // NOTE(java-quirk): `(fang + 35)` instead of `(fang * 35)`.
                        ctx.give_adena(player, ash * 35 + insignia * 35 + (fang + 35 + talon * 35), true);
                        for trophy in TROPHIES {
                            ctx.take_items(player, trophy, -1);
                        }
                        qs.set_memo_state(ctx, 0);
                        htmltext = "30735-22.html".to_owned();
                    } else if orders == 1 && trophies >= 1 && boxes >= 1 {
                        lions_claw_for(ctx, player);

                        for trophy in TROPHIES {
                            let count = ctx.get_quest_items_count(player, trophy);
                            ctx.give_adena(player, count * 35, true);
                        }
                        for trophy in TROPHIES {
                            ctx.take_items(player, trophy, -1);
                        }
                        qs.set_memo_state(ctx, 0);
                        htmltext = "30735-23.html".to_owned();
                    }
                }
                ABYSSAL_CELEBRANT_UNDRIAS => {
                    htmltext = if !ctx.has_quest_items(player, COMPLETE_STATUE_OF_SHILEN) {
                        if sum(ctx, player, &STATUE_PARTS) >= 1 {
                            "30130-02.html"
                        } else {
                            "30130-01.html"
                        }
                    } else {
                        "30130-03.html"
                    }
                    .to_owned();
                }
                BLACKSMITH_RUPIO => {
                    htmltext =
                        if sum(ctx, player, &STATUE_PARTS) >= 1 || sum(ctx, player, &TABLET_PARTS) >= 1 {
                            "30471-02.html"
                        } else {
                            "30471-01.html"
                        }
                        .to_owned();
                }
                IRON_GATES_LOCKIRIN => {
                    htmltext = if !ctx.has_quest_items(player, COMPLETE_ANCIENT_TABLET) {
                        if sum(ctx, player, &TABLET_PARTS) >= 1 {
                            "30531-02.html"
                        } else {
                            "30531-01.html"
                        }
                    } else {
                        "30531-03.html"
                    }
                    .to_owned();
                }
                MERCENARY_REEDFOOT => {
                    htmltext = if sum(ctx, player, &CARGO_BOXES) >= 1 {
                        "30736-02.html"
                    } else {
                        "30736-01.html"
                    }
                    .to_owned();
                }
                GUILDSMAN_MORGON => {
                    htmltext = if sum(ctx, player, &CARGO_BOXES) >= 1 {
                        "30737-02.html"
                    } else {
                        "30737-01.html"
                    }
                    .to_owned();
                }
                _ => {}
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
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00333_HuntOfTheBlackLion";

    const NPCS: &str = r#"
        <npc id="30735" type="Folk" level="20" name="Sophya"/>
        <npc id="30737" type="Folk" level="20" name="Morgon"/>
        <npc id="20160" type="Monster" level="10" name="Neer Crawler"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1369" type="EtcItem" name="Black Lion Mark"><set name="is_questitem" val="true"/></item>
        <item id="3440" type="EtcItem" name="Cargo Box 1"><set name="is_questitem" val="true"/></item>
        <item id="3671" type="EtcItem" name="Sophya's 1st Order"><set name="is_questitem" val="true"/></item>
        <item id="3675" type="EtcItem" name="Lion's Claw"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3677" type="EtcItem" name="Guild Coin"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3848" type="EtcItem" name="Undead Ash"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00333HuntOfTheBlackLion);
        let pages = Pages::of(&Q00333HuntOfTheBlackLion)
            .page("30735-02.htm", &["30735-04.htm"])
            .plain(&[
                "30735-01.htm",
                "30735-03.htm",
                "30735-04.htm",
                "30735-10.html",
                "30735-22.html",
                "30735-26.html",
            ])
            .page("30735-14.html", &["30735-10.html", "30735-26.html"])
            .page("30737-02.html", &["30737-06.html"])
            .plain(&["30737-03.html"])
            .npcs(&[30735, 30737]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let sophya = t.spawn(30735, 20);
        let morgon = t.spawn(30737, 40);
        (t, sophya, morgon)
    }

    #[test]
    fn a_low_level_character_is_refused() {
        let (mut t, sophya, _) = setup();
        t.enter(1, 24, vec![]);
        assert!(t.quest_page(1, sophya).contains("30735-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_quest_starts_and_an_order_makes_the_monsters_drop() {
        let (mut t, sophya, _) = setup();
        t.enter(1, 25, vec![]);
        assert!(t.quest_page(1, sophya).contains("30735-02.htm"));
        assert!(t.event(1, NAME, "30735-04.htm").contains("30735-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, sophya).contains("30735-14.html"));
        assert!(t.event(1, NAME, "30735-10.html").contains("30735-10.html"));
        assert_eq!(t.count(1, SOPHYAS_1ST_ORDER), 1);
        for _ in 0..40 {
            let m = t.spawn(NEER_CRAWLER, 20);
            t.kill(1, m);
        }
        assert!(t.count(1, UNDEAD_ASH) > 0);
    }

    #[test]
    fn the_mark_ends_the_quest_for_adena() {
        let (mut t, sophya, _) = setup();
        t.enter(1, 25, vec![item(10, BLACK_LION_MARK, 1)]);
        assert!(t.quest_page(1, sophya).contains("30735-03.htm"));
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
        });
        t.take_writes();
        assert!(t.quest_page(1, sophya).contains("30735-14.html"));
        assert!(t.event(1, NAME, "30735-26.html").contains("30735-26.html"));
        assert_eq!(t.count(1, 57), 12400);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn morgon_pays_for_a_cargo_box() {
        let (mut t, sophya, morgon) = setup();
        t.enter(1, 25, vec![item(10, CARGO_BOX_1ST, 1)]);
        t.quest_page(1, sophya);
        t.event(1, NAME, "30735-04.htm");
        t.take_writes();
        assert!(t.quest_page(1, morgon).contains("30737-02.html"));
        assert!(t.event(1, NAME, "30737-06.html").contains("30737-03.html"));
        assert_eq!(t.count(1, CARGO_BOX_1ST), 0);
        assert_eq!(t.count(1, GUILD_COIN), 1);
        assert_eq!(t.count(1, 57), 100);
    }
}
