//! Quest 66: Certified Arbalester (Master Rindy, Selsia, Clayton, Poitan, Gaius, Gauen, ...).
// Java: dist/game/data/scripts/quests/Q00066_CertifiedArbalester/Q00066_CertifiedArbalester.java
//
// Pattern: a kill and delivery chain with the memo state as the step counter, sibling of
// `q00064_certified_berserker.rs`; the diamond reward is paid once per character.

use l2_util::ObjectId;
use l2_world::script::{QuestSound, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const WAREHOUSE_KEEPER_HOLVAS: i32 = 30058;
const MAGISTER_GAIUS: i32 = 30171;
const BLACKSMITH_POITAN: i32 = 30458;
const MAGISTER_CLAYTON: i32 = 30464;
const MAGISTER_GAUEN: i32 = 30717;
const MAGISTER_KAIENA: i32 = 30720;
const MASTER_RINDY: i32 = 32201;
const GRAND_MASTER_MELDINA: i32 = 32214;
const MASTER_SELSIA: i32 = 32220;

// Items
const ENMITY_CRYSTAL: i32 = 9773;
const ENMITY_CRYSTAL_CORE: i32 = 9774;
const MANUSCRIPT_PAGE: i32 = 9775;
const ENCODED_PAGE_ON_THE_ANCIENT_RACE: i32 = 9776;
const KAMAEL_INQUISITOR_TRAINEE_MARK: i32 = 9777;
const FRAGMENT_OF_ATTACK_ORDERS: i32 = 9778;
const GRANDIS_ATTACK_ORDERS: i32 = 9779;
const MANASHENS_TALISMAN: i32 = 9780;
const RESEARCH_ON_THE_GIANTS_AND_THE_ANCIENT_RACE: i32 = 9781;

// Reward
const DIMENSIONAL_DIAMOND: i32 = 7562;
const KAMAEL_INQUISITOR_MARK: i32 = 9782;

// Monster
const GRANITIC_GOLEM: i32 = 20083;
const HANGMAN_TREE: i32 = 20144;
const AMBER_BASILISK: i32 = 20199;
const STRAIN: i32 = 20200;
const GHOUL: i32 = 20201;
const DEAD_SEEKER: i32 = 20202;
const GRANDIS: i32 = 20554;
const MANASHEN_GARGOYLE: i32 = 20563;
const TIMAK_ORC: i32 = 20583;
const TIMAK_ORC_ARCHER: i32 = 20584;
const DELU_LIZARDMAN_SHAMAN: i32 = 20781;
const WATCHMAN_OF_THE_PLAINS: i32 = 21102;
const ROUGHLY_HEWN_ROCK_GOLEM: i32 = 21103;
const DELU_LIZARDMAN_SUPPLIER: i32 = 21104;
const DELU_LIZARDMAN_AGENT: i32 = 21105;
const CURSED_SEER: i32 = 21106;
const DELU_LIZARDMAN_COMMANDER: i32 = 21107;

// Quest Monster
const CRIMSON_LADY: i32 = 27336;

// Misc
const MIN_LEVEL: i32 = 39;
const DIAMOND_REWARD_VAR: &str = "2ND_CLASS_DIAMOND_REWARD";

/// Java `Q00066_CertifiedArbalester`.
pub struct Q00066CertifiedArbalester;

impl Script for Q00066CertifiedArbalester {
    fn name(&self) -> &'static str {
        "Q00066_CertifiedArbalester"
    }

    fn path(&self) -> &'static str {
        "quests/Q00066_CertifiedArbalester"
    }

    fn id(&self) -> i32 {
        66
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MASTER_RINDY]);
        s.add_talk_id(&[
            MASTER_RINDY,
            WAREHOUSE_KEEPER_HOLVAS,
            MAGISTER_GAIUS,
            BLACKSMITH_POITAN,
            MAGISTER_CLAYTON,
            MAGISTER_GAUEN,
            MAGISTER_KAIENA,
            GRAND_MASTER_MELDINA,
            MASTER_SELSIA,
        ]);
        s.add_kill_id(&[
            GRANITIC_GOLEM,
            HANGMAN_TREE,
            AMBER_BASILISK,
            STRAIN,
            GHOUL,
            DEAD_SEEKER,
            GRANDIS,
            MANASHEN_GARGOYLE,
            TIMAK_ORC,
            TIMAK_ORC_ARCHER,
            DELU_LIZARDMAN_SHAMAN,
            WATCHMAN_OF_THE_PLAINS,
            ROUGHLY_HEWN_ROCK_GOLEM,
            DELU_LIZARDMAN_SUPPLIER,
            DELU_LIZARDMAN_AGENT,
            CURSED_SEER,
            DELU_LIZARDMAN_COMMANDER,
            CRIMSON_LADY,
        ]);
        s.register_quest_items(&[
            ENMITY_CRYSTAL,
            ENMITY_CRYSTAL_CORE,
            MANUSCRIPT_PAGE,
            ENCODED_PAGE_ON_THE_ANCIENT_RACE,
            KAMAEL_INQUISITOR_TRAINEE_MARK,
            FRAGMENT_OF_ATTACK_ORDERS,
            GRANDIS_ATTACK_ORDERS,
            MANASHENS_TALISMAN,
            RESEARCH_ON_THE_GIANTS_AND_THE_ANCIENT_RACE,
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
                // NOTE(java-quirk): unlike Q00065 it does not check `qs.isCreated()`.
                if ctx.player_level(player) >= MIN_LEVEL
                    && ctx.player_class_id(player) == player_class::WARDER
                    && !ctx.has_quest_items(player, KAMAEL_INQUISITOR_MARK)
                {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    if ctx.player_variable_int(player, DIAMOND_REWARD_VAR, 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 64);
                        ctx.set_player_variable(player, DIAMOND_REWARD_VAR, "1");
                        htmltext = Some("32201-07a.htm".to_owned());
                    } else {
                        htmltext = Some("32201-07.htm".to_owned());
                    }
                }
            }
            "32201-08.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30058-03.html" | "30058-04.html" => {
                if qs.is_memo_state(ctx, 7) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30058-05.html" => {
                if qs.is_memo_state(ctx, 7) {
                    qs.set_memo_state(ctx, 8);
                    qs.set_cond(ctx, 7, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30058-08.html" => {
                if qs.is_memo_state(ctx, 9) {
                    ctx.give_items(player, ENCODED_PAGE_ON_THE_ANCIENT_RACE, 1);
                    qs.set_memo_state(ctx, 10);
                    qs.set_cond(ctx, 9, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30171-03.html" => {
                if qs.is_memo_state(ctx, 23) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30171-05.html" => {
                if qs.is_memo_state(ctx, 23) {
                    ctx.take_items(player, GRANDIS_ATTACK_ORDERS, -1);
                    qs.set_memo_state(ctx, 24);
                    htmltext = Some(event.to_owned());
                }
            }
            "30171-06.html" | "30171-07.html" => {
                if qs.is_memo_state(ctx, 24) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30171-08.html" => {
                if qs.is_memo_state(ctx, 24) {
                    qs.set_memo_state(ctx, 25);
                }
                // NOTE(java-quirk): the cond is set whatever the memo state is.
                qs.set_cond(ctx, 14, true);
                htmltext = Some(event.to_owned());
            }
            "30458-03.html" => {
                if qs.is_memo_state(ctx, 5) {
                    ctx.take_items(player, ENMITY_CRYSTAL_CORE, 1);
                    qs.set_memo_state(ctx, 6);
                    htmltext = Some(event.to_owned());
                }
            }
            "30458-05.html" | "30458-06.html" | "30458-07.html" | "30458-08.html" => {
                if qs.is_memo_state(ctx, 6) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30458-09.html" => {
                if qs.is_memo_state(ctx, 6) {
                    qs.set_memo_state(ctx, 7);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30464-03.html" | "30464-04.html" | "30464-05.html" => {
                if qs.is_memo_state(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30464-06.html" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30464-09.html" => {
                if qs.is_memo_state(ctx, 4) {
                    ctx.give_items(player, ENMITY_CRYSTAL_CORE, 1);
                    qs.set_memo_state(ctx, 5);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30464-11.html" => htmltext = Some(event.to_owned()),
            "30717-03.html" | "30717-05.html" | "30717-06.html" | "30717-07.html" | "30717-08.html" => {
                if qs.is_memo_state(ctx, 28) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30717-09.html" => {
                if qs.is_memo_state(ctx, 28) {
                    qs.set_memo_state(ctx, 29);
                    qs.set_cond(ctx, 17, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30720-03.html" => {
                if qs.is_memo_state(ctx, 29) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30720-04.html" => {
                if qs.is_memo_state(ctx, 29) {
                    qs.set_memo_state(ctx, 30);
                    qs.set_cond(ctx, 18, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32214-03.html" => {
                if qs.is_memo_state(ctx, 10) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32214-04.html" => {
                if qs.is_memo_state(ctx, 10) {
                    ctx.take_items(player, ENCODED_PAGE_ON_THE_ANCIENT_RACE, 1);
                    ctx.give_items(player, KAMAEL_INQUISITOR_TRAINEE_MARK, 1);
                    qs.set_memo_state(ctx, 11);
                    qs.set_cond(ctx, 10, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32220-03.html" => {
                if qs.is_memo_state(ctx, 11) {
                    ctx.take_items(player, KAMAEL_INQUISITOR_TRAINEE_MARK, -1);
                    qs.set_memo_state(ctx, 12);
                    htmltext = Some(event.to_owned());
                }
            }
            "32220-05.html" => {
                if qs.is_memo_state(ctx, 12) {
                    qs.set_memo_state(ctx, 13);
                    htmltext = Some(event.to_owned());
                }
            }
            "32220-06.html" => {
                if qs.is_memo_state(ctx, 13) {
                    qs.set_memo_state_ex(ctx, 1, 0);
                    htmltext = Some(event.to_owned());
                }
            }
            "32220-09.html" | "32220-10.html" => {
                if qs.is_memo_state(ctx, 13) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32220-11.html" | "32220-12.html" | "32220-13.html" => {
                if qs.is_memo_state(ctx, 13) {
                    qs.set_memo_state(ctx, 13);
                    qs.set_memo_state_ex(ctx, 1, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "32220-13a.html" => {
                if qs.is_memo_state(ctx, 13) {
                    qs.set_memo_state(ctx, 20);
                    qs.set_memo_state_ex(ctx, 1, 0);
                    htmltext = Some(event.to_owned());
                }
            }
            "32220-13b.html" => {
                if qs.is_memo_state(ctx, 20) {
                    qs.set_memo_state(ctx, 21);
                    qs.set_cond(ctx, 11, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32220-19.html" | "32220-21.html" | "32220-22.html" | "32220-23.html" | "32220-24.html"
            | "32220-25.html" => {
                if qs.is_memo_state(ctx, 31) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32220-26.html" => {
                if qs.is_memo_state(ctx, 31) {
                    qs.set_memo_state_ex(ctx, 1, 0);
                    qs.set_memo_state(ctx, 32);
                    qs.set_cond(ctx, 19, true);
                    htmltext = Some(event.to_owned());
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
        match npc.id {
            GRANITIC_GOLEM | HANGMAN_TREE => {
                if qs.is_memo_state(ctx, 8) && ctx.get_quest_items_count(killer, MANUSCRIPT_PAGE) < 30 {
                    page_step(ctx, qs, killer, MANUSCRIPT_PAGE, 30, 8);
                    if ctx.get_random(1000) < 100 && ctx.get_quest_items_count(killer, MANUSCRIPT_PAGE) < 29 {
                        ctx.give_items(killer, MANUSCRIPT_PAGE, 1);
                    }
                }
            }
            AMBER_BASILISK => {
                if qs.is_memo_state(ctx, 8)
                    && ctx.get_quest_items_count(killer, MANUSCRIPT_PAGE) < 30
                    && ctx.get_random(1000) < 980
                {
                    page_step(ctx, qs, killer, MANUSCRIPT_PAGE, 30, 8);
                }
            }
            STRAIN => {
                if qs.is_memo_state(ctx, 8)
                    && ctx.get_quest_items_count(killer, MANUSCRIPT_PAGE) < 30
                    && ctx.get_random(1000) < 860
                {
                    page_step(ctx, qs, killer, MANUSCRIPT_PAGE, 30, 8);
                }
            }
            GHOUL | DEAD_SEEKER => {
                if qs.is_memo_state(ctx, 8) && ctx.get_quest_items_count(killer, MANUSCRIPT_PAGE) < 30 {
                    page_step(ctx, qs, killer, MANUSCRIPT_PAGE, 30, 8);
                    if ctx.get_random(1000) < 20 && ctx.get_quest_items_count(killer, MANUSCRIPT_PAGE) < 29 {
                        ctx.give_items(killer, MANUSCRIPT_PAGE, 1);
                    }
                }
            }
            GRANDIS => {
                if (qs.is_memo_state(ctx, 21)
                    || (qs.is_memo_state(ctx, 22)
                        && ctx.get_quest_items_count(killer, FRAGMENT_OF_ATTACK_ORDERS) < 10))
                    && ctx.get_random(1000) < 780
                {
                    if qs.is_memo_state(ctx, 21) && !ctx.has_quest_items(killer, FRAGMENT_OF_ATTACK_ORDERS) {
                        qs.set_memo_state(ctx, 22);
                        qs.set_cond(ctx, 12, true);
                        ctx.give_items(killer, FRAGMENT_OF_ATTACK_ORDERS, 1);
                    } else if qs.is_memo_state(ctx, 22)
                        && ctx.get_quest_items_count(killer, FRAGMENT_OF_ATTACK_ORDERS) >= 9
                    {
                        qs.set_memo_state(ctx, 23);
                        qs.set_cond(ctx, 13, true);
                        ctx.take_items(killer, FRAGMENT_OF_ATTACK_ORDERS, -1);
                        ctx.give_items(killer, GRANDIS_ATTACK_ORDERS, 1);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        ctx.give_items(killer, FRAGMENT_OF_ATTACK_ORDERS, 1);
                    }
                }
            }
            MANASHEN_GARGOYLE => {
                if (qs.is_memo_state(ctx, 25)
                    || (qs.is_memo_state(ctx, 26)
                        && ctx.get_quest_items_count(killer, MANASHENS_TALISMAN) < 10))
                    && ctx.get_random(1000) < 840
                {
                    if qs.is_memo_state(ctx, 25) && !ctx.has_quest_items(killer, MANASHENS_TALISMAN) {
                        qs.set_memo_state(ctx, 26);
                        qs.set_cond(ctx, 15, true);
                    } else if qs.is_memo_state(ctx, 26)
                        && ctx.get_quest_items_count(killer, MANASHENS_TALISMAN) >= 9
                    {
                        qs.set_memo_state(ctx, 27);
                        qs.set_cond(ctx, 16, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                    ctx.give_items(killer, MANASHENS_TALISMAN, 1);
                }
            }
            TIMAK_ORC | TIMAK_ORC_ARCHER => {
                if qs.is_memo_state(ctx, 32) {
                    let i4 = qs.get_memo_state_ex(ctx, 1);
                    if i4 < 5 {
                        qs.set_memo_state_ex(ctx, 1, i4 + 1);
                    } else if i4 >= 4 {
                        qs.set_memo_state_ex(ctx, 1, 0);
                        ctx.add_spawn_at(CRIMSON_LADY, npc.object_id, true, 0);
                    }
                }
            }
            DELU_LIZARDMAN_SHAMAN | DELU_LIZARDMAN_SUPPLIER => {
                if qs.is_memo_state(ctx, 3) && ctx.get_quest_items_count(killer, ENMITY_CRYSTAL) < 30 {
                    page_step(ctx, qs, killer, ENMITY_CRYSTAL, 30, 4);
                    if ctx.get_random(1000) < 80 && ctx.get_quest_items_count(killer, ENMITY_CRYSTAL) < 29 {
                        ctx.give_items(killer, ENMITY_CRYSTAL, 1);
                    }
                }
            }
            WATCHMAN_OF_THE_PLAINS => {
                if qs.is_memo_state(ctx, 3)
                    && ctx.get_quest_items_count(killer, ENMITY_CRYSTAL) < 30
                    && ctx.get_random(1000) < 840
                {
                    page_step(ctx, qs, killer, ENMITY_CRYSTAL, 30, 4);
                }
            }
            ROUGHLY_HEWN_ROCK_GOLEM => {
                if qs.is_memo_state(ctx, 3)
                    && ctx.get_quest_items_count(killer, ENMITY_CRYSTAL) < 30
                    && ctx.get_random(1000) < 860
                {
                    page_step(ctx, qs, killer, ENMITY_CRYSTAL, 30, 4);
                }
            }
            DELU_LIZARDMAN_AGENT => {
                if qs.is_memo_state(ctx, 3) && ctx.get_quest_items_count(killer, ENMITY_CRYSTAL) < 30 {
                    page_step(ctx, qs, killer, ENMITY_CRYSTAL, 30, 4);
                    if ctx.get_random(1000) < 240 && ctx.get_quest_items_count(killer, ENMITY_CRYSTAL) < 29 {
                        ctx.give_items(killer, ENMITY_CRYSTAL, 1);
                    }
                }
            }
            CURSED_SEER => {
                if qs.is_memo_state(ctx, 3) && ctx.get_quest_items_count(killer, ENMITY_CRYSTAL) < 30 {
                    page_step(ctx, qs, killer, ENMITY_CRYSTAL, 30, 4);
                    if ctx.get_random(1000) < 40 && ctx.get_quest_items_count(killer, ENMITY_CRYSTAL) < 29 {
                        ctx.give_items(killer, ENMITY_CRYSTAL, 1);
                    }
                }
            }
            DELU_LIZARDMAN_COMMANDER => {
                if qs.is_memo_state(ctx, 3) && ctx.get_quest_items_count(killer, ENMITY_CRYSTAL) < 30 {
                    if ctx.get_quest_items_count(killer, ENMITY_CRYSTAL) >= 28 {
                        qs.set_cond(ctx, 4, true);
                    }
                    if ctx.get_quest_items_count(killer, ENMITY_CRYSTAL) < 29 {
                        ctx.give_items(killer, ENMITY_CRYSTAL, 2);
                        if ctx.get_random(1000) < 220 && ctx.get_quest_items_count(killer, ENMITY_CRYSTAL) < 28 {
                            ctx.give_items(killer, ENMITY_CRYSTAL, 1);
                            if ctx.get_quest_items_count(killer, ENMITY_CRYSTAL) >= 27 {
                                qs.set_cond(ctx, 4, true);
                            } else {
                                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                            }
                        }
                    } else {
                        ctx.give_items(killer, ENMITY_CRYSTAL, 1);
                    }
                }
            }
            CRIMSON_LADY => {
                if qs.is_memo_state(ctx, 32) {
                    ctx.give_items(killer, RESEARCH_ON_THE_GIANTS_AND_THE_ANCIENT_RACE, 1);
                    qs.set_memo_state(ctx, 32);
                    qs.set_cond(ctx, 20, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            if npc.id == MASTER_RINDY {
                if ctx.player_class_id(player) == player_class::WARDER
                    && !ctx.has_quest_items(player, KAMAEL_INQUISITOR_MARK)
                {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        htmltext = "32201-01.htm".to_owned();
                    } else {
                        htmltext = "32201-02.html".to_owned();
                    }
                } else {
                    htmltext = "32201-03.html".to_owned();
                }
            }
        } else if qs.is_started(ctx) {
            match npc.id {
                MASTER_RINDY => {
                    if memo_state == 1 {
                        qs.set_memo_state(ctx, 2);
                        qs.set_cond(ctx, 2, true);
                        htmltext = "32201-09.html".to_owned();
                    } else if memo_state == 2 {
                        htmltext = "32201-10.html".to_owned();
                    } else if memo_state > 2 && memo_state < 11 {
                        htmltext = "32201-11.html".to_owned();
                    } else if memo_state >= 11 {
                        htmltext = "32201-12.html".to_owned();
                    }
                }
                WAREHOUSE_KEEPER_HOLVAS => {
                    if memo_state < 7 {
                        htmltext = "30058-01.html".to_owned();
                    } else if memo_state == 7 {
                        htmltext = "30058-02.html".to_owned();
                    } else if memo_state == 8 {
                        if ctx.get_quest_items_count(player, MANUSCRIPT_PAGE) < 30 {
                            htmltext = "30058-06.html".to_owned();
                        } else {
                            ctx.take_items(player, MANUSCRIPT_PAGE, -1);
                            qs.set_memo_state(ctx, 9);
                            htmltext = "30058-07.html".to_owned();
                        }
                    } else if memo_state == 9 {
                        ctx.give_items(player, ENCODED_PAGE_ON_THE_ANCIENT_RACE, 1);
                        qs.set_memo_state(ctx, 10);
                        qs.set_cond(ctx, 9, true);
                        htmltext = "30058-09.html".to_owned();
                    } else if memo_state > 9 {
                        htmltext = "30058-10.html".to_owned();
                    }
                }
                MAGISTER_GAIUS => {
                    if memo_state < 23 {
                        htmltext = "30171-01.html".to_owned();
                    } else if memo_state == 23 {
                        htmltext = "30171-02.html".to_owned();
                    } else if memo_state == 24 {
                        htmltext = "30171-06.html".to_owned();
                    } else if memo_state == 25 {
                        htmltext = "30171-09.html".to_owned();
                    } else if memo_state == 26 {
                        htmltext = "30171-10.html".to_owned();
                    } else if memo_state == 27 {
                        htmltext = "30171-11.html".to_owned();
                    } else if memo_state == 28 {
                        htmltext = "30171-12.html".to_owned();
                    } else if memo_state == 29 {
                        htmltext = "30171-13.html".to_owned();
                    }
                }
                BLACKSMITH_POITAN => {
                    if memo_state < 5 {
                        htmltext = "30458-01.html".to_owned();
                    } else if memo_state == 5 {
                        htmltext = "30458-02.html".to_owned();
                    } else if memo_state == 6 {
                        htmltext = "30458-04.html".to_owned();
                    } else if memo_state == 7 {
                        htmltext = "30458-10.html".to_owned();
                    }
                }
                MAGISTER_CLAYTON => {
                    if memo_state < 2 {
                        htmltext = "30464-01.html".to_owned();
                    } else if memo_state == 2 {
                        qs.set_memo_state(ctx, 2);
                        htmltext = "30464-02.html".to_owned();
                    } else if memo_state == 3 {
                        if ctx.get_quest_items_count(player, ENMITY_CRYSTAL) < 30 {
                            htmltext = "30464-07.html".to_owned();
                        } else {
                            ctx.take_items(player, ENMITY_CRYSTAL, -1);
                            qs.set_memo_state(ctx, 4);
                            htmltext = "30464-08.html".to_owned();
                        }
                    } else if memo_state == 4 {
                        ctx.give_items(player, ENMITY_CRYSTAL_CORE, 1);
                        qs.set_memo_state(ctx, 5);
                        qs.set_cond(ctx, 5, true);
                        htmltext = "30464-10.html".to_owned();
                    } else if memo_state == 5 {
                        htmltext = "30464-12.html".to_owned();
                    } else if memo_state > 5 {
                        htmltext = "30464-13.html".to_owned();
                    }
                }
                MAGISTER_GAUEN => {
                    if memo_state < 27 {
                        htmltext = "30717-01.html".to_owned();
                    } else if memo_state == 27 {
                        ctx.take_items(player, MANASHENS_TALISMAN, -1);
                        qs.set_memo_state(ctx, 28);
                        htmltext = "30717-02.html".to_owned();
                    } else if memo_state == 28 {
                        htmltext = "30717-04.html".to_owned();
                    } else if memo_state >= 29 {
                        htmltext = "30717-10.html".to_owned();
                    }
                }
                MAGISTER_KAIENA => {
                    if memo_state < 29 {
                        htmltext = "30720-01.html".to_owned();
                    } else if memo_state == 29 {
                        htmltext = "30720-02.html".to_owned();
                    }
                    if memo_state >= 30 {
                        htmltext = "30720-05.html".to_owned();
                    }
                }
                GRAND_MASTER_MELDINA => {
                    if memo_state < 10 {
                        htmltext = "32214-01.html".to_owned();
                    } else if memo_state == 10 {
                        htmltext = "32214-02.html".to_owned();
                    }
                    if memo_state == 11 {
                        htmltext = "32214-05.html".to_owned();
                    }
                    if memo_state > 11 {
                        htmltext = "32214-06.html".to_owned();
                    }
                }
                MASTER_SELSIA => {
                    if memo_state < 11 {
                        htmltext = "32220-01.html".to_owned();
                    } else if memo_state == 11 {
                        htmltext = "32220-02.html".to_owned();
                    } else if memo_state == 12 {
                        htmltext = "32220-04.html".to_owned();
                    } else if memo_state == 13 {
                        if qs.get_memo_state_ex(ctx, 1) == 0 {
                            qs.set_memo_state_ex(ctx, 1, 0);
                            htmltext = "32220-07.html".to_owned();
                        } else if qs.get_memo_state_ex(ctx, 1) == 1 {
                            qs.set_memo_state_ex(ctx, 1, 0);
                            htmltext = "32220-08.html".to_owned();
                        }
                    } else if memo_state == 20 {
                        qs.set_memo_state(ctx, 21);
                        qs.set_cond(ctx, 11, true);
                        htmltext = "32220-14.html".to_owned();
                    } else if memo_state == 21 {
                        htmltext = "32220-15.html".to_owned();
                    } else if memo_state == 22 {
                        htmltext = "32220-16.html".to_owned();
                    } else if (23..30).contains(&memo_state) {
                        htmltext = "32220-17.html".to_owned();
                    } else if memo_state == 30 {
                        qs.set_memo_state(ctx, 31);
                        htmltext = "32220-18.html".to_owned();
                    } else if memo_state == 31 {
                        htmltext = "32220-20.html".to_owned();
                    } else if memo_state == 32 {
                        if !ctx.has_quest_items(player, RESEARCH_ON_THE_GIANTS_AND_THE_ANCIENT_RACE) {
                            htmltext = "32220-27.html".to_owned();
                        } else {
                            ctx.give_adena(player, 77_666, true);
                            ctx.give_items(player, KAMAEL_INQUISITOR_MARK, 1);
                            ctx.add_exp_and_sp(player, 429_546, 29_476);
                            qs.exit_quest(ctx, false, true);
                            ctx.send_social_action(player, 3);
                            htmltext = "32220-28.html".to_owned();
                        }
                    }
                }
                _ => {}
            }
        }

        if qs.is_completed(ctx) && npc.id == MASTER_RINDY {
            if ctx.player_class_id(player) == player_class::ARBALESTER {
                htmltext = "32201-05.html".to_owned();
            } else {
                htmltext = "32201-06.html".to_owned();
            }
        }
        Some(htmltext)
    }
}

/// The block every counting drop of Java repeats: at `limit - 1` items the cond `cond` is set,
/// below that the item sound plays, then one item is given.
fn page_step(ctx: &mut ScriptCtx<'_>, qs: l2_world::script::Qs, killer: ObjectId, item: i32, limit: i64, cond: i32) {
    if ctx.get_quest_items_count(killer, item) >= limit - 1 {
        qs.set_cond(ctx, cond, true);
    } else {
        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
    }
    ctx.give_items(killer, item, 1);
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00066_CertifiedArbalester";

    const NPCS: &str = r#"
        <npc id="30464" type="Folk" level="70" name="Clayton"/>
        <npc id="32201" type="Folk" level="70" name="Rindy"/>
        <npc id="32220" type="Folk" level="70" name="Selsia"/>
        <npc id="20083" type="Monster" level="40" name="Granitic Golem"/>
        <npc id="20781" type="Monster" level="40" name="Delu Lizardman Shaman"/>
        <npc id="20583" type="Monster" level="40" name="Timak Orc"/>
        <npc id="27336" type="Monster" level="40" name="Crimson Lady"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="9773" type="EtcItem" name="Enmity Crystal"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="9775" type="EtcItem" name="Manuscript Page"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="9781" type="EtcItem" name="Research"><set name="is_questitem" val="true"/></item>
        <item id="9782" type="EtcItem" name="Kamael Inquisitor Mark"/>"#;

    struct Npcs {
        rindy: Oid,
        clayton: Oid,
        selsia: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00066CertifiedArbalester);
        let pages = Pages::of(&Q00066CertifiedArbalester)
            .page("32201-01.htm", &["ACCEPT"])
            .plain(&[
                "32201-02.html",
                "32201-03.html",
                "32201-05.html",
                "32201-06.html",
                "32201-07.htm",
                "32201-07a.htm",
                "32220-28.html",
            ])
            .npcs(&[MASTER_RINDY, MAGISTER_CLAYTON, MASTER_SELSIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            rindy: t.spawn(MASTER_RINDY, 20),
            clayton: t.spawn(MAGISTER_CLAYTON, 40),
            selsia: t.spawn(MASTER_SELSIA, 60),
        };
        (t, npcs)
    }

    fn ev(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00066CertifiedArbalester.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    fn talk(t: &mut ScriptTest, npc: Oid) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(npc) {
                out = Q00066CertifiedArbalester.on_talk(ctx, npc, Oid(1));
            }
        });
        out
    }

    fn memo(t: &mut ScriptTest, value: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_memo_state(ctx, value);
            }
        });
        t.take_writes();
    }

    #[test]
    fn only_a_level_39_warder_without_the_mark_starts_it() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 38, race::KAMAEL, player_class::WARDER, vec![]);
        assert!(t.quest_page(1, npcs.rindy).contains("32201-02.html"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::KAMAEL, player_class::TROOPER, vec![]);
        assert!(t.quest_page(1, npcs.rindy).contains("32201-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_start_pays_64_diamonds_once_and_the_steps_write_the_rows() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::KAMAEL, player_class::WARDER, vec![]);
        assert!(t.quest_page(1, npcs.rindy).contains("32201-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("32201-07a.htm"));
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 64);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );

        assert_eq!(ev(&mut t, "32201-08.html").as_deref(), Some("32201-08.html"));
        assert_eq!(ev(&mut t, "30464-06.html").as_deref(), Some("30464-06.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2"),
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "cond", "3"),
            ]
        );

        // The lizardmen drop the enmity crystals up to 30 (cond 4), Clayton takes them.
        for _ in 0..200 {
            if t.count(1, ENMITY_CRYSTAL) >= 30 {
                break;
            }
            let mob = t.spawn(DELU_LIZARDMAN_SHAMAN, 80);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, ENMITY_CRYSTAL), 30);
        assert_eq!(talk(&mut t, npcs.clayton).as_deref(), Some("30464-08.html"));
        assert_eq!(t.count(1, ENMITY_CRYSTAL), 0);
    }

    #[test]
    fn selsia_pays_the_mark_and_the_completed_quest_shows_the_arbalester_page() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::KAMAEL, player_class::WARDER, vec![]);
        t.quest_page(1, npcs.rindy);
        t.event(1, NAME, "ACCEPT");
        memo(&mut t, 32);
        assert_eq!(talk(&mut t, npcs.selsia).as_deref(), Some("32220-27.html"));

        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), RESEARCH_ON_THE_GIANTS_AND_THE_ANCIENT_RACE, 1);
        });
        assert_eq!(talk(&mut t, npcs.selsia).as_deref(), Some("32220-28.html"));
        assert_eq!(t.count(1, KAMAEL_INQUISITOR_MARK), 1);
        assert_eq!(t.count(1, 57), 77_666);
        let w: Vec<_> = t
            .take_writes()
            .into_iter()
            .filter(|w| !format!("{w:?}").contains("__compltdStateFlags"))
            .collect();
        assert!(w.ends_with(&[
            rows::delete(NAME, false),
            rows::update(NAME, "<state>", "Completed")
        ]));
        assert_eq!(talk(&mut t, npcs.rindy).as_deref(), Some("32201-06.html"));
    }
}
