//! Quest 108: Jumble, Tumble, Diamond Fuss (a dwarf runs a chain of contracts through Reep,
//! Torocco, Maron, Brunon, the two warehouse keepers and Gouph, with gems from the Goblin
//! Brigands and a star diamond from a Blade Bat).
// Java: dist/game/data/scripts/quests/Q00108_JumbleTumbleDiamondFuss/Q00108_JumbleTumbleDiamondFuss.java
//
// Pattern: several talk NPCs that each move the chain on by cond, two gem drops by chance with a
// limit (`giveItemRandomly(.., 1, 10, chance, false)`), and the NewbieGuide memo as in quest 104.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const COLLECTOR_GOUPH: i32 = 30523;
const TRADER_REEP: i32 = 30516;
const CARRIER_TOROCCO: i32 = 30555;
const MINER_MARON: i32 = 30529;
const BLACKSMITH_BRUNON: i32 = 30526;
const WAREHOUSE_KEEPER_MURDOC: i32 = 30521;
const WAREHOUSE_KEEPER_AIRY: i32 = 30522;

// Monsters
const GOBLIN_BRIGAND_LEADER: i32 = 20323;
const GOBLIN_BRIGAND_LIEUTENANT: i32 = 20324;
const BLADE_BAT: i32 = 20480;

// Items
const GOUPHS_CONTRACT: i32 = 1559;
const REEPS_CONTRACT: i32 = 1560;
const ELVEN_WINE: i32 = 1561;
const BRUNONS_DICE: i32 = 1562;
const BRUNONS_CONTRACT: i32 = 1563;
const AQUAMARINE: i32 = 1564;
const CHRYSOBERYL: i32 = 1565;
const GEM_BOX: i32 = 1566;
const COAL_PIECE: i32 = 1567;
const BRUNONS_LETTER: i32 = 1568;
const BERRY_TART: i32 = 1569;
const BAT_DIAGRAM: i32 = 1570;
const STAR_DIAMOND: i32 = 1571;

/// Java `REWARDS` (`ItemHolder`s): item id and count.
const REWARDS: [(i32, i64); 6] = [
    (1060, 100), // Lesser Healing Potion
    (4412, 10),  // Echo Crystal - Theme of Battle
    (4413, 10),  // Echo Crystal - Theme of Love
    (4414, 10),  // Echo Crystal - Theme of Solitude
    (4415, 10),  // Echo Crystal - Theme of Feast
    (4416, 10),  // Echo Crystal - Theme of Celebration
];
const SILVERSMITH_HAMMER: i32 = 1511;
const SPIRITSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5790, 3000);
const SOULSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5789, 7000);

// Misc
const MIN_LEVEL: i32 = 10;
const MAX_GEM_COUNT: i64 = 10;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `GOBLIN_DROP_CHANCES`: the gem chance of each goblin.
fn goblin_drop_chance(npc_id: i32) -> Option<f64> {
    match npc_id {
        GOBLIN_BRIGAND_LEADER => Some(0.8),
        GOBLIN_BRIGAND_LIEUTENANT => Some(0.6),
        _ => None,
    }
}

/// Java `Q00108_JumbleTumbleDiamondFuss`.
pub struct Q00108JumbleTumbleDiamondFuss;

impl Script for Q00108JumbleTumbleDiamondFuss {
    fn name(&self) -> &'static str {
        "Q00108_JumbleTumbleDiamondFuss"
    }

    fn path(&self) -> &'static str {
        "quests/Q00108_JumbleTumbleDiamondFuss"
    }

    fn id(&self) -> i32 {
        108
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[COLLECTOR_GOUPH]);
        s.add_talk_id(&[
            COLLECTOR_GOUPH,
            TRADER_REEP,
            CARRIER_TOROCCO,
            MINER_MARON,
            BLACKSMITH_BRUNON,
            WAREHOUSE_KEEPER_MURDOC,
            WAREHOUSE_KEEPER_AIRY,
        ]);
        s.add_kill_id(&[GOBLIN_BRIGAND_LEADER, GOBLIN_BRIGAND_LIEUTENANT, BLADE_BAT]);
        s.register_quest_items(&[
            GOUPHS_CONTRACT,
            REEPS_CONTRACT,
            ELVEN_WINE,
            BRUNONS_DICE,
            BRUNONS_CONTRACT,
            AQUAMARINE,
            CHRYSOBERYL,
            GEM_BOX,
            COAL_PIECE,
            BRUNONS_LETTER,
            BERRY_TART,
            BAT_DIAGRAM,
            STAR_DIAMOND,
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
        match event {
            "30523-04.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    ctx.give_items(player, GOUPHS_CONTRACT, 1);
                    return Some(event.to_owned());
                }
                None
            }
            "30555-02.html" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, REEPS_CONTRACT) {
                    ctx.take_items(player, REEPS_CONTRACT, -1);
                    ctx.give_items(player, ELVEN_WINE, 1);
                    qs.set_cond(ctx, 3, true);
                    return Some(event.to_owned());
                }
                None
            }
            "30526-02.html" => {
                if qs.is_cond(ctx, 4) && ctx.has_quest_items(player, BRUNONS_DICE) {
                    ctx.take_items(player, BRUNONS_DICE, -1);
                    ctx.give_items(player, BRUNONS_CONTRACT, 1);
                    qs.set_cond(ctx, 5, true);
                    return Some(event.to_owned());
                }
                None
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match npc.id {
            COLLECTOR_GOUPH => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_race(talker) != race::DWARF {
                        "30523-01.htm"
                    } else if ctx.player_level(talker) < MIN_LEVEL {
                        "30523-02.htm"
                    } else {
                        "30523-03.htm"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => {
                        if ctx.has_quest_items(talker, GOUPHS_CONTRACT) {
                            htmltext = "30523-05.html".to_owned();
                        }
                    }
                    2..=6 => {
                        if ctx.has_at_least_one_quest_item(
                            talker,
                            &[REEPS_CONTRACT, ELVEN_WINE, BRUNONS_DICE, BRUNONS_CONTRACT],
                        ) {
                            htmltext = "30523-06.html".to_owned();
                        }
                    }
                    7 => {
                        if ctx.has_quest_items(talker, GEM_BOX) {
                            ctx.take_items(talker, GEM_BOX, -1);
                            ctx.give_items(talker, COAL_PIECE, 1);
                            qs.set_cond(ctx, 8, true);
                            htmltext = "30523-07.html".to_owned();
                        }
                    }
                    8..=11 => {
                        if ctx.has_at_least_one_quest_item(
                            talker,
                            &[COAL_PIECE, BRUNONS_LETTER, BERRY_TART, BAT_DIAGRAM],
                        ) {
                            htmltext = "30523-08.html".to_owned();
                        }
                    }
                    12 => {
                        if ctx.has_quest_items(talker, STAR_DIAMOND) {
                            if ctx.player_level(talker) < 25 {
                                if ctx.is_mage_class(talker) {
                                    let (id, count) = SPIRITSHOTS_NO_GRADE_FOR_ROOKIES;
                                    ctx.give_items(talker, id, count);
                                    ctx.play_sound_named(talker, "tutorial_voice_027");
                                } else {
                                    let (id, count) = SOULSHOTS_NO_GRADE_FOR_ROOKIES;
                                    ctx.give_items(talker, id, count);
                                    ctx.play_sound_named(talker, "tutorial_voice_026");
                                }
                            }
                            // Newbie Guide.
                            if let Some(guide) = ctx.quest_state_of(NEWBIE_GUIDE, talker, true) {
                                if !ctx.have_nr_memo(guide, GUIDE_MISSION) {
                                    ctx.set_nr_memo(guide, GUIDE_MISSION);
                                    ctx.set_nr_memo_state(guide, GUIDE_MISSION, 100_000);
                                    ctx.show_on_screen_npc_string(
                                        talker,
                                        NpcStringId::AcquisitionOfRaceSpecificWeaponCompleteNGoFindTheNewbieGuide,
                                        2,
                                        5000,
                                        &[],
                                    );
                                } else if (ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 1_000_000) / 100_000
                                    != 1
                                {
                                    ctx.set_nr_memo(guide, GUIDE_MISSION);
                                    let state = ctx.get_nr_memo_state(guide, GUIDE_MISSION);
                                    ctx.set_nr_memo_state(guide, GUIDE_MISSION, state + 100_000);
                                    ctx.show_on_screen_npc_string(
                                        talker,
                                        NpcStringId::AcquisitionOfRaceSpecificWeaponCompleteNGoFindTheNewbieGuide,
                                        2,
                                        5000,
                                        &[],
                                    );
                                }
                            }
                            ctx.add_exp_and_sp(talker, 34565, 2962);
                            ctx.give_adena(talker, 14666, true);
                            for (id, count) in REWARDS {
                                ctx.give_items(talker, id, count);
                            }
                            ctx.give_items(talker, SILVERSMITH_HAMMER, 1);
                            qs.exit_quest(ctx, false, true);
                            ctx.world().send(
                                talker,
                                &SocialAction {
                                    object_id: talker.0,
                                    action_id: 3,
                                },
                            );
                            htmltext = "30523-09.html".to_owned();
                        }
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(talker),
                _ => {}
            },
            TRADER_REEP => match qs.get_cond(ctx) {
                1 => {
                    if ctx.has_quest_items(talker, GOUPHS_CONTRACT) {
                        ctx.take_items(talker, GOUPHS_CONTRACT, -1);
                        ctx.give_items(talker, REEPS_CONTRACT, 1);
                        qs.set_cond(ctx, 2, true);
                        htmltext = "30516-01.html".to_owned();
                    }
                }
                2 => {
                    if ctx.has_quest_items(talker, REEPS_CONTRACT) {
                        htmltext = "30516-02.html".to_owned();
                    }
                }
                cond => {
                    if cond > 2 {
                        htmltext = "30516-02.html".to_owned();
                    }
                }
            },
            CARRIER_TOROCCO => match qs.get_cond(ctx) {
                2 => {
                    if ctx.has_quest_items(talker, REEPS_CONTRACT) {
                        htmltext = "30555-01.html".to_owned();
                    }
                }
                3 => {
                    if ctx.has_quest_items(talker, ELVEN_WINE) {
                        htmltext = "30555-03.html".to_owned();
                    }
                }
                7 => {
                    if ctx.has_quest_items(talker, GEM_BOX) {
                        htmltext = "30555-04.html".to_owned();
                    }
                }
                _ => {
                    if qs.is_started(ctx) {
                        htmltext = "30555-05.html".to_owned();
                    }
                }
            },
            MINER_MARON => match qs.get_cond(ctx) {
                3 => {
                    if ctx.has_quest_items(talker, ELVEN_WINE) {
                        ctx.take_items(talker, ELVEN_WINE, -1);
                        ctx.give_items(talker, BRUNONS_DICE, 1);
                        qs.set_cond(ctx, 4, true);
                        htmltext = "30529-01.html".to_owned();
                    }
                }
                4 => {
                    if ctx.has_quest_items(talker, BRUNONS_DICE) {
                        htmltext = "30529-02.html".to_owned();
                    }
                }
                cond => {
                    if cond > 4 {
                        htmltext = "30529-03.html".to_owned();
                    }
                }
            },
            BLACKSMITH_BRUNON => match qs.get_cond(ctx) {
                4 => {
                    if ctx.has_quest_items(talker, BRUNONS_DICE) {
                        htmltext = "30526-01.html".to_owned();
                    }
                }
                5 => {
                    if ctx.has_quest_items(talker, BRUNONS_CONTRACT) {
                        htmltext = "30526-03.html".to_owned();
                    }
                }
                6 => {
                    if ctx.has_quest_items(talker, BRUNONS_CONTRACT)
                        && ctx.get_quest_items_count(talker, AQUAMARINE) >= MAX_GEM_COUNT
                        && ctx.get_quest_items_count(talker, CHRYSOBERYL) >= MAX_GEM_COUNT
                    {
                        ctx.take_items_many(talker, -1, &[BRUNONS_CONTRACT, AQUAMARINE, CHRYSOBERYL]);
                        ctx.give_items(talker, GEM_BOX, 1);
                        qs.set_cond(ctx, 7, true);
                        htmltext = "30526-04.html".to_owned();
                    }
                }
                7 => {
                    if ctx.has_quest_items(talker, GEM_BOX) {
                        htmltext = "30526-05.html".to_owned();
                    }
                }
                8 => {
                    if ctx.has_quest_items(talker, COAL_PIECE) {
                        ctx.take_items(talker, COAL_PIECE, -1);
                        ctx.give_items(talker, BRUNONS_LETTER, 1);
                        qs.set_cond(ctx, 9, true);
                        htmltext = "30526-06.html".to_owned();
                    }
                }
                9 => {
                    if ctx.has_quest_items(talker, BRUNONS_LETTER) {
                        htmltext = "30526-07.html".to_owned();
                    }
                }
                10..=12 => {
                    if ctx.has_at_least_one_quest_item(talker, &[BERRY_TART, BAT_DIAGRAM, STAR_DIAMOND]) {
                        htmltext = "30526-08.html".to_owned();
                    }
                }
                _ => {}
            },
            WAREHOUSE_KEEPER_MURDOC => match qs.get_cond(ctx) {
                9 => {
                    if ctx.has_quest_items(talker, BRUNONS_LETTER) {
                        ctx.take_items(talker, BRUNONS_LETTER, -1);
                        ctx.give_items(talker, BERRY_TART, 1);
                        qs.set_cond(ctx, 10, true);
                        htmltext = "30521-01.html".to_owned();
                    }
                }
                10 => {
                    if ctx.has_quest_items(talker, BERRY_TART) {
                        htmltext = "30521-02.html".to_owned();
                    }
                }
                11 | 12 => htmltext = "30521-03.html".to_owned(),
                _ => {}
            },
            WAREHOUSE_KEEPER_AIRY => match qs.get_cond(ctx) {
                10 => {
                    if ctx.has_quest_items(talker, BERRY_TART) {
                        ctx.take_items(talker, BERRY_TART, -1);
                        ctx.give_items(talker, BAT_DIAGRAM, 1);
                        qs.set_cond(ctx, 11, true);
                        htmltext = "30522-01.html".to_owned();
                    }
                }
                11 => {
                    if ctx.has_quest_items(talker, BAT_DIAGRAM) {
                        htmltext = "30522-02.html".to_owned();
                    }
                }
                12 => {
                    if ctx.has_quest_items(talker, STAR_DIAMOND) {
                        htmltext = "30522-03.html".to_owned();
                    }
                }
                _ => {
                    if qs.is_started(ctx) {
                        htmltext = "30522-04.html".to_owned();
                    }
                }
            },
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            return;
        }
        match npc.id {
            GOBLIN_BRIGAND_LEADER | GOBLIN_BRIGAND_LIEUTENANT => {
                if qs.is_cond(ctx, 5) && ctx.has_quest_items(killer, BRUNONS_CONTRACT) {
                    let Some(drop_chance) = goblin_drop_chance(npc.id) else {
                        return;
                    };
                    let mut play_sound = false;
                    if ctx.give_item_randomly(
                        killer,
                        Some(npc),
                        AQUAMARINE,
                        1,
                        1,
                        MAX_GEM_COUNT,
                        drop_chance,
                        false,
                    ) {
                        if ctx.get_quest_items_count(killer, CHRYSOBERYL) >= MAX_GEM_COUNT {
                            qs.set_cond(ctx, 6, true);
                            return;
                        }
                        play_sound = true;
                    }
                    if ctx.give_item_randomly(
                        killer,
                        Some(npc),
                        CHRYSOBERYL,
                        1,
                        1,
                        MAX_GEM_COUNT,
                        drop_chance,
                        false,
                    ) {
                        if ctx.get_quest_items_count(killer, AQUAMARINE) >= MAX_GEM_COUNT {
                            qs.set_cond(ctx, 6, true);
                            return;
                        }
                        play_sound = true;
                    }
                    if play_sound {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            BLADE_BAT => {
                if qs.is_cond(ctx, 11)
                    && ctx.has_quest_items(killer, BAT_DIAGRAM)
                    && ctx.give_item_randomly(killer, Some(npc), STAR_DIAMOND, 1, 1, 1, 0.2, true)
                {
                    ctx.take_items(killer, BAT_DIAGRAM, -1);
                    qs.set_cond(ctx, 12, false);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::{
        BLACKSMITH_BRUNON, CARRIER_TOROCCO, COLLECTOR_GOUPH, MINER_MARON,
        Q00108JumbleTumbleDiamondFuss as Quest, TRADER_REEP, WAREHOUSE_KEEPER_AIRY, WAREHOUSE_KEEPER_MURDOC,
    };
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00108_JumbleTumbleDiamondFuss";
    const GOBLIN_LEADER: i32 = 20323;
    const BLADE_BAT: i32 = 20480;
    const GOUPHS_CONTRACT: i32 = 1559;
    const REEPS_CONTRACT: i32 = 1560;
    const ELVEN_WINE: i32 = 1561;
    const BRUNONS_DICE: i32 = 1562;
    const BRUNONS_CONTRACT: i32 = 1563;
    const AQUAMARINE: i32 = 1564;
    const CHRYSOBERYL: i32 = 1565;
    const GEM_BOX: i32 = 1566;
    const COAL_PIECE: i32 = 1567;
    const BRUNONS_LETTER: i32 = 1568;
    const BERRY_TART: i32 = 1569;
    const BAT_DIAGRAM: i32 = 1570;
    const STAR_DIAMOND: i32 = 1571;

    const NPCS: &str = r#"
        <npc id="30523" type="Folk" level="70" name="Collector Gouph"/>
        <npc id="30516" type="Folk" level="70" name="Trader Reep"/>
        <npc id="30555" type="Folk" level="70" name="Carrier Torocco"/>
        <npc id="30529" type="Folk" level="70" name="Miner Maron"/>
        <npc id="30526" type="Folk" level="70" name="Blacksmith Brunon"/>
        <npc id="30521" type="Folk" level="70" name="Warehouse Keeper Murdoc"/>
        <npc id="30522" type="Folk" level="70" name="Warehouse Keeper Airy"/>
        <npc id="20323" type="Monster" level="20" name="Goblin Brigand Leader"/>
        <npc id="20324" type="Monster" level="20" name="Goblin Brigand Lieutenant"/>
        <npc id="20480" type="Monster" level="20" name="Blade Bat"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1559" type="EtcItem" name="Gouph's Contract"><set name="is_questitem" val="true"/></item>
        <item id="1560" type="EtcItem" name="Reep's Contract"><set name="is_questitem" val="true"/></item>
        <item id="1561" type="EtcItem" name="Elven Wine"><set name="is_questitem" val="true"/></item>
        <item id="1562" type="EtcItem" name="Brunon's Dice"><set name="is_questitem" val="true"/></item>
        <item id="1563" type="EtcItem" name="Brunon's Contract"><set name="is_questitem" val="true"/></item>
        <item id="1564" type="EtcItem" name="Aquamarine"><set name="is_questitem" val="true"/></item>
        <item id="1565" type="EtcItem" name="Chrysoberyl"><set name="is_questitem" val="true"/></item>
        <item id="1566" type="EtcItem" name="Gem Box"><set name="is_questitem" val="true"/></item>
        <item id="1567" type="EtcItem" name="Coal Piece"><set name="is_questitem" val="true"/></item>
        <item id="1568" type="EtcItem" name="Brunon's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1569" type="EtcItem" name="Berry Tart"><set name="is_questitem" val="true"/></item>
        <item id="1570" type="EtcItem" name="Bat Diagram"><set name="is_questitem" val="true"/></item>
        <item id="1571" type="EtcItem" name="Star Diamond"><set name="is_questitem" val="true"/></item>
        <item id="1511" type="EtcItem" name="Silversmith Hammer"/>
        <item id="1060" type="EtcItem" name="Lesser Healing Potion"><set name="is_stackable" val="true"/></item>
        <item id="5789" type="EtcItem" name="Soulshot for Rookies"><set name="is_stackable" val="true"/></item>"#;

    fn pages() -> Pages {
        Pages::of(&Quest)
            .page("30523-03.htm", &["30523-04.htm"])
            .page("30555-01.html", &["30555-02.html"])
            .page("30526-01.html", &["30526-02.html"])
            .plain(&[
                "30523-01.htm",
                "30523-02.htm",
                "30523-04.htm",
                "30523-05.html",
                "30523-06.html",
                "30523-07.html",
                "30523-08.html",
                "30523-09.html",
                "30516-01.html",
                "30516-02.html",
                "30555-02.html",
                "30555-03.html",
                "30555-04.html",
                "30555-05.html",
                "30529-01.html",
                "30529-02.html",
                "30529-03.html",
                "30526-02.html",
                "30526-03.html",
                "30526-04.html",
                "30526-05.html",
                "30526-06.html",
                "30526-07.html",
                "30526-08.html",
                "30521-01.html",
                "30521-02.html",
                "30521-03.html",
                "30522-01.html",
                "30522-02.html",
                "30522-03.html",
                "30522-04.html",
            ])
            .npcs(&[
                COLLECTOR_GOUPH,
                TRADER_REEP,
                CARRIER_TOROCCO,
                MINER_MARON,
                BLACKSMITH_BRUNON,
                WAREHOUSE_KEEPER_MURDOC,
                WAREHOUSE_KEEPER_AIRY,
            ])
    }

    /// The NPCs of the chain, spread out in the test world.
    struct Npcs {
        gouph: Oid,
        reep: Oid,
        torocco: Oid,
        maron: Oid,
        brunon: Oid,
        murdoc: Oid,
        airy: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Quest);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            gouph: t.spawn(COLLECTOR_GOUPH, 20),
            reep: t.spawn(TRADER_REEP, 30),
            torocco: t.spawn(CARRIER_TOROCCO, 40),
            maron: t.spawn(MINER_MARON, 50),
            brunon: t.spawn(BLACKSMITH_BRUNON, 60),
            murdoc: t.spawn(WAREHOUSE_KEEPER_MURDOC, 70),
            airy: t.spawn(WAREHOUSE_KEEPER_AIRY, 80),
        };
        (t, npcs)
    }

    #[test]
    fn a_dwarf_of_level_10_starts_and_gets_the_contract() {
        let (mut t, n) = setup();
        t.enter_as(1, 10, race::DWARF, player_class::SCAVENGER, vec![]);
        assert!(t.quest_page(1, n.gouph).contains("30523-03.htm"));
        assert!(t.event(1, NAME, "30523-04.htm").contains("30523-04.htm"));
        assert_eq!(t.count(1, GOUPHS_CONTRACT), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn the_start_is_refused_below_10_and_for_other_races() {
        let (mut t, n) = setup();
        t.enter_as(1, 9, race::DWARF, player_class::SCAVENGER, vec![]);
        assert!(t.quest_page(1, n.gouph).contains("30523-02.htm"));
        t.enter_as(2, 20, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, n.gouph).contains("30523-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_contracts_the_gems_the_box_and_the_reward() {
        let (mut t, n) = setup();
        t.enter_as(1, 20, race::DWARF, player_class::SCAVENGER, vec![]);
        assert!(t.quest_page(1, n.gouph).contains("30523-03.htm"));
        assert!(t.event(1, NAME, "30523-04.htm").contains("30523-04.htm"));
        t.take_writes();

        // Reep: Gouph's contract becomes Reep's contract, cond 2.
        assert!(t.quest_page(1, n.reep).contains("30516-01.html"));
        assert_eq!((t.count(1, GOUPHS_CONTRACT), t.count(1, REEPS_CONTRACT)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Torocco: Reep's contract becomes elven wine, cond 3.
        assert!(t.quest_page(1, n.torocco).contains("30555-01.html"));
        assert!(t.event(1, NAME, "30555-02.html").contains("30555-02.html"));
        assert_eq!((t.count(1, REEPS_CONTRACT), t.count(1, ELVEN_WINE)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Maron: the wine becomes Brunon's dice, cond 4.
        assert!(t.quest_page(1, n.maron).contains("30529-01.html"));
        assert_eq!((t.count(1, ELVEN_WINE), t.count(1, BRUNONS_DICE)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Brunon: the dice become Brunon's contract, cond 5.
        assert!(t.quest_page(1, n.brunon).contains("30526-01.html"));
        assert!(t.event(1, NAME, "30526-02.html").contains("30526-02.html"));
        assert_eq!((t.count(1, BRUNONS_DICE), t.count(1, BRUNONS_CONTRACT)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // The goblin brigands drop the gems by chance, ten of each: cond 6.
        for _ in 0..1000 {
            if t.count(1, AQUAMARINE) == 10 && t.count(1, CHRYSOBERYL) == 10 {
                break;
            }
            let goblin = t.spawn(GOBLIN_LEADER, 60);
            t.kill(1, goblin);
        }
        assert_eq!((t.count(1, AQUAMARINE), t.count(1, CHRYSOBERYL)), (10, 10));
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "6")));

        // Brunon: the contract and the gems go for the gem box, cond 7.
        assert!(t.quest_page(1, n.brunon).contains("30526-04.html"));
        assert_eq!(
            (
                t.count(1, BRUNONS_CONTRACT),
                t.count(1, AQUAMARINE),
                t.count(1, GEM_BOX)
            ),
            (0, 0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);

        // Gouph: the box goes for a coal piece, cond 8.
        assert!(t.quest_page(1, n.gouph).contains("30523-07.html"));
        assert_eq!((t.count(1, GEM_BOX), t.count(1, COAL_PIECE)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "8")]);

        // Brunon: the coal becomes the letter, cond 9.
        assert!(t.quest_page(1, n.brunon).contains("30526-06.html"));
        assert_eq!((t.count(1, COAL_PIECE), t.count(1, BRUNONS_LETTER)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "9")]);

        // Murdoc: the letter becomes a berry tart, cond 10.
        assert!(t.quest_page(1, n.murdoc).contains("30521-01.html"));
        assert_eq!((t.count(1, BRUNONS_LETTER), t.count(1, BERRY_TART)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "10")]);

        // Airy: the tart becomes the bat diagram, cond 11.
        assert!(t.quest_page(1, n.airy).contains("30522-01.html"));
        assert_eq!((t.count(1, BERRY_TART), t.count(1, BAT_DIAGRAM)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "11")]);

        // A blade bat drops the star diamond by a 20% chance: cond 12, the diagram goes.
        for _ in 0..1000 {
            if t.count(1, STAR_DIAMOND) == 1 {
                break;
            }
            let bat = t.spawn(BLADE_BAT, 60);
            t.kill(1, bat);
        }
        assert_eq!((t.count(1, STAR_DIAMOND), t.count(1, BAT_DIAGRAM)), (1, 0));
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "12")));

        // Gouph: the reward (the Silversmith Hammer), the exp and the exit.
        assert!(t.quest_page(1, n.gouph).contains("30523-09.html"));
        assert_eq!(t.count(1, 1511), 1);
        assert_eq!(t.count(1, 1060), 100);
        assert_eq!(t.count(1, 57), 14666);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
