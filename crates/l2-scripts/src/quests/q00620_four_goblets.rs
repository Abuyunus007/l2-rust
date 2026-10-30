//! Quest 620: Four Goblets (the Four Sepulchers: goblets from the four bosses, the Antique Brooch,
//! and the sealed boxes the monsters drop).
// Java: dist/game/data/scripts/quests/Q00620_FourGoblets/Q00620_FourGoblets.java
//
// The rooms, doors and bosses are `FourSepulchers` (`ai/areas/imperial_tomb/four_sepulchers.rs`);
// this quest only hands out the items and the rewards.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const GHOST_OF_WIGOTH_1: i32 = 31452;
const NAMELESS_SPIRIT: i32 = 31453;
const GHOST_OF_WIGOTH_2: i32 = 31454;
const GHOST_CHAMBERLAIN_OF_ELMOREDEN_1: i32 = 31919;
const CONQUERORS_SEPULCHER_MANAGER: i32 = 31921;
const EMPERORS_SEPULCHER_MANAGER: i32 = 31922;
const GREAT_SAGES_SEPULCHER_MANAGER: i32 = 31923;
const JUDGES_SEPULCHER_MANAGER: i32 = 31924;

// Items
const BROKEN_RELIC_PART: i32 = 7254;
const SEALED_BOX: i32 = 7255;
const GOBLET_OF_ALECTIA: i32 = 7256;
const GOBLET_OF_TISHAS: i32 = 7257;
const GOBLET_OF_MEKARA: i32 = 7258;
const GOBLET_OF_MORIGUL: i32 = 7259;
const CHAPEL_KEY: i32 = 7260;
const USED_GRAVE_PASS: i32 = 7261;
const ANTIQUE_BROOCH: i32 = 7262;
const GOBLETS: [i32; 4] = [GOBLET_OF_ALECTIA, GOBLET_OF_TISHAS, GOBLET_OF_MEKARA, GOBLET_OF_MORIGUL];

// Misc
const MIN_LEVEL: i32 = 74;

// Locations
const ENTER_LOC: (i32, i32, i32) = (170000, -88250, -2912);
const EXIT_LOC: (i32, i32, i32) = (169584, -91008, -2912);

// Mobs
const HALISHA_ALECTIA: i32 = 25339;
const HALISHA_TISHAS: i32 = 25342;
const HALISHA_MEKARA: i32 = 25346;
const HALISHA_MORIGUL: i32 = 25349;

/// Java `MOB1`: `(npc id, chance)`.
const MOB1: [(i32, f64); 10] = [
    (18141, 0.9),
    (18142, 0.9),
    (18143, 0.9),
    (18144, 0.9),
    (18145, 0.76),
    (18146, 0.78),
    (18147, 0.73),
    (18148, 0.85),
    (18149, 0.75),
    (18230, 0.58),
];

/// Java `MOB2`: `(npc id, percent for 2 boxes)`.
const MOB2: [(i32, i32); 60] = [
    (18120, 51),
    (18121, 44),
    (18122, 10),
    (18123, 51),
    (18124, 44),
    (18125, 10),
    (18126, 51),
    (18127, 44),
    (18128, 10),
    (18129, 51),
    (18130, 44),
    (18131, 10),
    (18132, 54),
    (18133, 42),
    (18134, 7),
    (18135, 42),
    (18136, 42),
    (18137, 6),
    (18138, 41),
    (18139, 39),
    (18140, 41),
    (18166, 8),
    (18167, 7),
    (18168, 10),
    (18169, 6),
    (18170, 7),
    (18171, 11),
    (18172, 6),
    (18173, 17),
    (18174, 45),
    (18175, 10),
    (18176, 17),
    (18177, 45),
    (18178, 10),
    (18179, 17),
    (18180, 45),
    (18181, 10),
    (18182, 17),
    (18183, 45),
    (18184, 10),
    (18185, 46),
    (18186, 47),
    (18187, 42),
    (18188, 7),
    (18189, 42),
    (18190, 42),
    (18191, 6),
    (18192, 41),
    (18193, 39),
    (18194, 41),
    (18195, 8),
    (18220, 47),
    (18221, 51),
    (18222, 43),
    (18223, 7),
    (18224, 44),
    (18225, 43),
    (18226, 6),
    (18227, 82),
    (18229, 41),
];

/// Java `MOB3`: `(npc id, percent for 5 boxes)`.
const MOB3: [(i32, i32); 8] = [
    (18212, 50),
    (18213, 50),
    (18214, 50),
    (18215, 50),
    (18216, 50),
    (18217, 50),
    (18218, 50),
    (18219, 50),
];

/// Reward tables of `getReward`: `(upper bound of the 0..1000 roll, item, count)`; the last entry
/// is Java's `else`.
type Table = [(i32, i32, i64)];

const CASE1_A: &Table = &[
    (43, 1884, 42),
    (66, 1895, 36),
    (184, 1876, 4),
    (250, 1881, 6),
    (287, 5549, 8),
    (484, 1874, 1),
    (681, 1889, 1),
    (799, 1877, 1),
    (902, 1894, 1),
    (1000, 4043, 1),
];
const CASE1_B: &Table = &[
    (335, 1888, 1),
    (556, 4040, 1),
    (725, 1890, 1),
    (872, 5550, 1),
    (962, 1893, 1),
    (986, 4046, 1),
    (1000, 4048, 1),
];
const CASE2_A: &Table = &[
    (148, 1878, 8),
    (175, 1882, 24),
    (273, 1879, 4),
    (322, 1880, 6),
    (357, 1885, 6),
    (554, 1875, 1),
    (685, 1883, 1),
    (803, 5220, 1),
    (901, 4039, 1),
    (1000, 4044, 1),
];
const CASE2_B: &Table = &[
    (350, 1887, 1),
    (587, 4042, 1),
    (798, 1886, 1),
    (922, 4041, 1),
    (966, 1892, 1),
    (996, 1891, 1),
    (1000, 4047, 1),
];
const CASE3_A: &Table = &[(223, 730, 1), (893, 948, 1), (1000, 960, 1)];
const CASE3_B: &Table = &[(202, 729, 1), (928, 947, 1), (1000, 959, 1)];
const CASE4_A: &Table = &[
    (88, 6698, 1),
    (185, 6699, 1),
    (238, 6700, 1),
    (262, 6701, 1),
    (292, 6702, 1),
    (356, 6703, 1),
    (420, 6704, 1),
    (482, 6705, 1),
    (554, 6706, 1),
    (576, 6707, 1),
    (640, 6708, 1),
    (704, 6709, 1),
    (777, 6710, 1),
    (799, 6711, 1),
    (863, 6712, 1),
    (927, 6713, 1),
    (1000, 6714, 1),
];
const CASE4_B: &Table = &[
    (100, 6688, 1),
    (198, 6689, 1),
    (298, 6690, 1),
    (398, 6691, 1),
    (499, 7579, 1),
    (601, 6693, 1),
    (703, 6694, 1),
    (801, 6695, 1),
    (902, 6696, 1),
    (1000, 6697, 1),
];

/// One `if (getRandom(1000) < chance) { i2 = true; final int i1 = getRandom(1000); if (i1 < ..) ..}`.
fn roll(ctx: &mut ScriptCtx<'_>, player: ObjectId, chance: i32, table: &Table) -> bool {
    if ctx.get_random(1000) >= chance {
        return false;
    }
    let i1 = ctx.get_random(1000);
    if let Some(&(_, item, count)) = table.iter().find(|(bound, _, _)| i1 < *bound) {
        ctx.give_items(player, item, count);
    }
    true
}

/// Java `getReward(player)`.
fn get_reward(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> bool {
    let mut i2 = false;
    match ctx.get_random(5) {
        0 => {
            i2 = true;
            ctx.give_adena(player, 10_000, true);
        }
        1 => {
            i2 |= roll(ctx, player, 848, CASE1_A);
            i2 |= roll(ctx, player, 323, CASE1_B);
        }
        2 => {
            i2 |= roll(ctx, player, 847, CASE2_A);
            i2 |= roll(ctx, player, 251, CASE2_B);
        }
        3 => {
            i2 |= roll(ctx, player, 31, CASE3_A);
            i2 |= roll(ctx, player, 5, CASE3_B);
        }
        4 => {
            i2 |= roll(ctx, player, 329, CASE4_A);
            i2 |= roll(ctx, player, 54, CASE4_B);
        }
        _ => {}
    }

    ctx.take_items(player, SEALED_BOX, 1);
    i2
}

/// The goblet of a boss: `actionForEachPlayer`.
fn goblet_of(npc_id: i32) -> Option<i32> {
    match npc_id {
        HALISHA_ALECTIA => Some(GOBLET_OF_ALECTIA),
        HALISHA_TISHAS => Some(GOBLET_OF_TISHAS),
        HALISHA_MEKARA => Some(GOBLET_OF_MEKARA),
        HALISHA_MORIGUL => Some(GOBLET_OF_MORIGUL),
        _ => None,
    }
}

/// Java `Q00620_FourGoblets`.
pub struct Q00620FourGoblets;

impl Script for Q00620FourGoblets {
    fn name(&self) -> &'static str {
        "Q00620_FourGoblets"
    }

    fn path(&self) -> &'static str {
        "quests/Q00620_FourGoblets"
    }

    fn id(&self) -> i32 {
        620
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NAMELESS_SPIRIT]);
        s.add_talk_id(&[
            NAMELESS_SPIRIT,
            GHOST_OF_WIGOTH_1,
            GHOST_OF_WIGOTH_2,
            GHOST_CHAMBERLAIN_OF_ELMOREDEN_1,
            CONQUERORS_SEPULCHER_MANAGER,
            EMPERORS_SEPULCHER_MANAGER,
            GREAT_SAGES_SEPULCHER_MANAGER,
            JUDGES_SEPULCHER_MANAGER,
        ]);
        s.add_kill_id(&[HALISHA_ALECTIA, HALISHA_TISHAS, HALISHA_MEKARA, HALISHA_MORIGUL]);
        let mob1: Vec<i32> = MOB1.iter().map(|m| m.0).collect();
        s.add_kill_id(&mob1);
        let mob2: Vec<i32> = MOB2.iter().map(|m| m.0).collect();
        s.add_kill_id(&mob2);
        let mob3: Vec<i32> = MOB3.iter().map(|m| m.0).collect();
        s.add_kill_id(&mob3);
        s.register_quest_items(&[
            SEALED_BOX,
            GOBLET_OF_ALECTIA,
            GOBLET_OF_TISHAS,
            GOBLET_OF_MEKARA,
            GOBLET_OF_MORIGUL,
            USED_GRAVE_PASS,
        ]);
    }

    fn action_for_each_player(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        _is_summon: bool,
    ) {
        let qs = ctx.get_quest_state(player, false);
        if let Some(qs) = qs
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false)
            && let Some(goblet) = goblet_of(npc.id)
        {
            if !ctx.has_quest_items(player, goblet) && !ctx.has_quest_items(player, ANTIQUE_BROOCH) {
                ctx.give_items(player, goblet, 1);
            }

            qs.set_memo_state_ex(ctx, 1, 2);
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "31453-02.htm" | "31453-03.htm" | "31453-04.htm" | "31453-05.htm" | "31453-06.htm"
            | "31453-07.htm" | "31453-08.htm" | "31453-09.htm" | "31453-10.htm" | "31453-11.htm"
            | "31453-16.html" | "31453-17.html" | "31453-18.html" | "31453-19.html"
            | "31453-20.html" | "31453-21.html" | "31453-22.html" | "31453-23.html"
            | "31453-24.html" | "31453-25.html" | "31453-27.html" | "31452-04.html" => {
                htmltext = Some(event.to_owned());
            }
            "31453-12.htm" => {
                qs.set_memo_state(ctx, 0);
                qs.start_quest(ctx);
                if ctx.has_quest_items(player, ANTIQUE_BROOCH) {
                    qs.set_cond(ctx, 2, false);
                }

                htmltext = Some(event.to_owned());
            }
            "31453-15.html" => {
                ctx.take_items_many(player, -1, &[CHAPEL_KEY, USED_GRAVE_PASS]);
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "31453-28.html" => {
                if ctx.has_all_quest_items(player, &GOBLETS) {
                    ctx.give_items(player, ANTIQUE_BROOCH, 1);
                    qs.set_cond(ctx, 2, true);
                    ctx.take_items_many(player, 1, &GOBLETS);
                    htmltext = Some(event.to_owned());
                }
            }
            "31454-02.html" => {
                let (x, y, z) = ENTER_LOC;
                ctx.teleport_player(player, x, y, z, 0, false);
                htmltext = Some(event.to_owned());
            }
            "31454-04.html" => {
                let memo_state_ex = qs.get_memo_state_ex(ctx, 1);
                if (memo_state_ex == 2 || memo_state_ex == 3)
                    && ctx.get_quest_items_count(player, BROKEN_RELIC_PART) >= 1000
                {
                    htmltext = Some(event.to_owned());
                }
            }
            "6881" | "6883" | "6885" | "6887" | "6891" | "6893" | "6895" | "6897" | "6899" | "7580" => {
                let memo_state_ex = qs.get_memo_state_ex(ctx, 1);
                if (memo_state_ex == 2 || memo_state_ex == 3)
                    && ctx.get_quest_items_count(player, BROKEN_RELIC_PART) >= 1000
                {
                    // The event is the item id.
                    if let Ok(item_id) = event.parse::<i32>() {
                        ctx.give_items(player, item_id, 1);
                    }
                    ctx.take_items(player, BROKEN_RELIC_PART, 1000);
                    htmltext = Some("31454-05.html".to_owned());
                }
            }
            "31454-07.html" => {
                let memo_state_ex = qs.get_memo_state_ex(ctx, 1);
                if (memo_state_ex == 2 || memo_state_ex == 3) && ctx.has_quest_items(player, SEALED_BOX) {
                    // NOTE(java-quirk): `getRandom(100) < 100` is always true, but it rolls.
                    if ctx.get_random(100) < 100 {
                        let i2 = get_reward(ctx, player);
                        htmltext = Some(if i2 { event } else { "31454-08.html" }.to_owned());
                    } else {
                        ctx.take_items(player, SEALED_BOX, 1);
                        htmltext = Some("31454-09.html".to_owned());
                    }
                }
            }
            "EXIT" => {
                ctx.take_items(player, CHAPEL_KEY, -1);
                let (x, y, z) = EXIT_LOC;
                ctx.teleport_player(player, x, y, z, 0, false);
                return Some(String::new());
            }
            "31919-02.html" => {
                if ctx.has_quest_items(player, SEALED_BOX) {
                    if ctx.get_random(100) < 50 {
                        let i2 = get_reward(ctx, player);
                        htmltext = Some(if i2 { event } else { "31919-03.html" }.to_owned());
                    } else {
                        ctx.take_items(player, SEALED_BOX, 1);
                        htmltext = Some("31919-04.html".to_owned());
                    }
                } else {
                    htmltext = Some("31919-05.html".to_owned());
                }
            }
            // Ghost Chamberlain of Elmoreden: teleport to the 4 sepulchers, then (Java has no
            // `break`) on to the Imperial Tomb entrance.
            "TELEPORT_TO_4S" | "TELEPORT_TO_IT" => {
                let destinations: &[(i32, i32, i32)] = if event == "TELEPORT_TO_4S" {
                    &[(178298, -84574, -7216), (186942, -75602, -2834)]
                } else {
                    &[(186942, -75602, -2834)]
                };
                for &(x, y, z) in destinations {
                    if ctx.has_quest_items(player, ANTIQUE_BROOCH) {
                        ctx.teleport_player(player, x, y, z, 0, false);
                        return None;
                    }

                    if ctx.has_quest_items(player, USED_GRAVE_PASS) {
                        ctx.take_items(player, USED_GRAVE_PASS, 1);
                        ctx.teleport_player(player, x, y, z, 0, false);
                        return None;
                    }

                    htmltext = Some(format!("{}-00.html", npc?.id));
                }
            }
            _ => {}
        }

        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, is_summon: bool) {
        match npc.id {
            HALISHA_ALECTIA | HALISHA_TISHAS | HALISHA_MEKARA | HALISHA_MORIGUL => {
                ctx.execute_for_each_player(player, npc, is_summon, true, false);
            }
            _ => {
                let Some(qs) = ctx.random_party_member_state(player, -1, 3, Some(npc)) else {
                    return;
                };
                let member = qs.player();
                if let Some(&(_, chance)) = MOB1.iter().find(|m| m.0 == npc.id) {
                    ctx.give_item_randomly(member, Some(npc), SEALED_BOX, 1, 1, 0, chance, true);
                } else if let Some(&(_, percent)) = MOB2.iter().find(|m| m.0 == npc.id) {
                    let item_count = if ctx.get_random(100) < percent { 2 } else { 1 };
                    ctx.give_item_randomly(member, Some(npc), SEALED_BOX, item_count, item_count, 0, 1.0, true);
                } else if let Some(&(_, percent)) = MOB3.iter().find(|m| m.0 == npc.id) {
                    let item_count = if ctx.get_random(100) < percent { 5 } else { 4 };
                    ctx.give_item_randomly(member, Some(npc), SEALED_BOX, item_count, item_count, 0, 1.0, true);
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.state(ctx) == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "31453-01.htm"
            } else {
                "31453-13.html"
            }
            .to_owned();
        } else if qs.state(ctx) == STARTED {
            match npc.id {
                NAMELESS_SPIRIT => {
                    htmltext = if !ctx.has_quest_items(player, ANTIQUE_BROOCH) {
                        if ctx.has_all_quest_items(player, &GOBLETS) {
                            "31453-26.html"
                        } else {
                            "31453-14.html"
                        }
                    } else {
                        "31453-29.html"
                    }
                    .to_owned();
                }
                GHOST_OF_WIGOTH_1 => {
                    htmltext = if !ctx.has_quest_items(player, ANTIQUE_BROOCH) {
                        if ctx.has_all_quest_items(player, &GOBLETS) {
                            "31452-01.html"
                        } else if ctx.get_quest_items_count_of(player, &GOBLETS) < 3 {
                            "31452-02.html"
                        } else {
                            "31452-03.html"
                        }
                    } else {
                        "31452-05.html"
                    }
                    .to_owned();
                }
                GHOST_OF_WIGOTH_2 => {
                    let memo_state_ex = qs.get_memo_state_ex(ctx, 1);
                    let broken_relic_part_count = ctx.get_quest_items_count(player, BROKEN_RELIC_PART);
                    let has_box = ctx.has_quest_items(player, SEALED_BOX);
                    let enough = broken_relic_part_count >= 1000;
                    if memo_state_ex == 2 {
                        if ctx.has_all_quest_items(player, &GOBLETS) {
                            htmltext = match (has_box, enough) {
                                (false, false) => "31454-01.html",
                                (false, true) => "31454-03.html",
                                (true, false) => "31454-06.html",
                                (true, true) => "31454-10.html",
                            }
                            .to_owned();
                        } else {
                            htmltext = match (has_box, enough) {
                                (false, false) => "31454-11.html",
                                (false, true) => "31454-12.html",
                                (true, false) => "31454-13.html",
                                (true, true) => "31454-14.html",
                            }
                            .to_owned();
                        }
                        qs.set_memo_state_ex(ctx, 1, 3);
                    } else if memo_state_ex == 3 {
                        htmltext = match (has_box, enough) {
                            (false, false) => "31454-15.html",
                            (false, true) => "31454-12.html",
                            (true, false) => "31454-13.html",
                            (true, true) => "31454-14.html",
                        }
                        .to_owned();
                        qs.set_memo_state_ex(ctx, 1, 3);
                    }
                }
                GHOST_CHAMBERLAIN_OF_ELMOREDEN_1 => htmltext = "31919-01.html".to_owned(),
                CONQUERORS_SEPULCHER_MANAGER => htmltext = "31921-01.html".to_owned(),
                EMPERORS_SEPULCHER_MANAGER => htmltext = "31922-01.html".to_owned(),
                GREAT_SAGES_SEPULCHER_MANAGER => htmltext = "31923-01.html".to_owned(),
                JUDGES_SEPULCHER_MANAGER => htmltext = "31924-01.html".to_owned(),
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

    const NAME: &str = "Q00620_FourGoblets";

    const NPCS: &str = r#"
        <npc id="31453" type="Folk" level="70" name="Nameless Spirit"/>
        <npc id="31454" type="Folk" level="70" name="Ghost of Wigoth"/>
        <npc id="31919" type="Folk" level="70" name="Ghost Chamberlain"/>
        <npc id="25339" type="Monster" level="80" name="Halisha Alectia"/>
        <npc id="18120" type="Monster" level="70" name="Guard"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7254" type="EtcItem" name="Broken Relic Part"><set name="is_stackable" val="true"/></item>
        <item id="7255" type="EtcItem" name="Sealed Box"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7256" type="EtcItem" name="Goblet of Alectia"><set name="is_questitem" val="true"/></item>
        <item id="7257" type="EtcItem" name="Goblet of Tishas"><set name="is_questitem" val="true"/></item>
        <item id="7258" type="EtcItem" name="Goblet of Mekara"><set name="is_questitem" val="true"/></item>
        <item id="7259" type="EtcItem" name="Goblet of Morigul"><set name="is_questitem" val="true"/></item>
        <item id="7262" type="EtcItem" name="Antique Brooch"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00620FourGoblets);
        let pages = Pages::of(&Q00620FourGoblets)
            .page("31453-01.htm", &["31453-12.htm"])
            .plain(&[
                "31453-12.htm",
                "31453-13.html",
                "31453-14.html",
                "31453-26.html",
                "31453-29.html",
            ])
            .page("31453-26.html", &["31453-28.html"])
            .plain(&["31453-28.html"])
            .plain(&["31454-02.html", "31919-03.html", "31919-04.html", "31919-05.html"])
            .page("31919-01.html", &["31919-02.html"])
            .page("31919-02.html", &[])
            .npcs(&[31453, 31454, 31919]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let spirit = t.spawn(31453, 20);
        let wigoth = t.spawn(31454, 40);
        (t, spirit, wigoth)
    }

    #[test]
    fn level_74_starts_and_the_goblets_give_the_brooch() {
        let (mut t, spirit, _) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, spirit).contains("31453-01.htm"));
        assert!(t.event(1, NAME, "31453-12.htm").contains("31453-12.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "memoState", "0"),
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.quest_page(1, spirit).contains("31453-14.html"));

        t.world.run_as_script(NAME, |ctx| {
            for goblet in GOBLETS {
                ctx.give_items(Oid(1), goblet, 1);
            }
        });
        assert!(t.quest_page(1, spirit).contains("31453-26.html"));
        assert!(t.event(1, NAME, "31453-28.html").contains("31453-28.html"));
        assert_eq!(t.count(1, ANTIQUE_BROOCH), 1);
        for goblet in GOBLETS {
            assert_eq!(t.count(1, goblet), 0);
        }
        assert!(t.quest_page(1, spirit).contains("31453-29.html"));
    }

    #[test]
    fn a_character_below_74_is_refused() {
        let (mut t, spirit, _) = setup();
        t.enter(1, 73, vec![]);
        assert!(t.quest_page(1, spirit).contains("31453-13.html"));
    }

    #[test]
    fn killing_a_boss_gives_the_goblet_and_sets_the_memo() {
        let (mut t, spirit, _) = setup();
        t.enter(1, 80, vec![]);
        t.quest_page(1, spirit);
        t.event(1, NAME, "31453-12.htm");
        t.take_writes();
        let boss = t.spawn(HALISHA_ALECTIA, 60);
        t.kill(1, boss);
        assert_eq!(t.count(1, GOBLET_OF_ALECTIA), 1);
        assert_eq!(t.take_writes(), [rows::insert(NAME, "memoStateEx1", "2")]);
    }

    #[test]
    fn the_monsters_drop_sealed_boxes_to_a_party_member() {
        let (mut t, spirit, _) = setup();
        t.enter(1, 80, vec![]);
        t.quest_page(1, spirit);
        t.event(1, NAME, "31453-12.htm");
        let mut boxes = 0;
        for _ in 0..10 {
            let guard = t.spawn(18120, 60);
            t.kill(1, guard);
            t.world.delete_npc(guard);
            boxes = t.count(1, SEALED_BOX);
        }
        assert!(boxes >= 10, "1 or 2 boxes a kill: {boxes}");
    }

    #[test]
    fn the_chamberlain_pays_a_reward_or_takes_the_box_away() {
        let (mut t, spirit, _) = setup();
        let chamberlain = t.spawn(31919, 50);
        t.enter(1, 80, vec![item(1001, SEALED_BOX, 5)]);
        t.quest_page(1, spirit);
        t.event(1, NAME, "31453-12.htm");
        let mut pages = Vec::new();
        for _ in 0..5 {
            t.quest_page(1, chamberlain);
            pages.push(t.event(1, NAME, "31919-02.html"));
        }
        assert_eq!(t.count(1, SEALED_BOX), 0);
        assert!(pages.iter().all(|p| p.contains("31919-0")));
        // No box left.
        t.quest_page(1, chamberlain);
        assert!(t.event(1, NAME, "31919-02.html").contains("31919-05.html"));
    }
}
