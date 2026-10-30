//! Quest 421: Little Wing's Big Adventure (Cronos gives the hatchling's flute; Mimyu's pet quest).
// Java: dist/game/data/scripts/quests/Q00421_LittleWingsBigAdventure/Q00421_LittleWingsBigAdventure.java
//
// Pattern: a pet quest. The flute's object id is the quest's link to the summon (`fluteObjectId`).
// The trees (the attacked monsters) drop fairy leaves while the summon hits them, a memo state
// counts the trees, and a killed tree calls 20 guardian ghosts. The end is the bugle of the flute.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const CRONOS: i32 = 30610;
const MIMYU: i32 = 30747;

// Items
const DRAGONFLUTE_OF_WIND: i32 = 3500;
const DRAGONFLUTE_OF_STAR: i32 = 3501;
const DRAGONFLUTE_OF_TWILIGHT: i32 = 3502;
const FAIRY_LEAF: i32 = 4325;
const FLUTES: [i32; 3] = [DRAGONFLUTE_OF_WIND, DRAGONFLUTE_OF_STAR, DRAGONFLUTE_OF_TWILIGHT];

// Monsters
const TREE_OF_WIND: i32 = 27185;
const TREE_OF_STAR: i32 = 27186;
const TREE_OF_TWILIGHT: i32 = 27187;
const TREE_OF_ABYSS: i32 = 27188;
const SOUL_OF_TREE_GUARDIAN: i32 = 27189;

// Skills (Java `SkillHolder(id, level)`)
const CURSE_OF_MIMYU: SkillKey = SkillKey { id: 4167, level: 1 };
const DRYAD_ROOT: SkillKey = SkillKey { id: 1201, level: 33 };
const VICIOUS_POISON: SkillKey = SkillKey { id: 4243, level: 1 };

// Rewards
const DRAGON_BUGLE_OF_WIND: i32 = 4422;
const DRAGON_BUGLE_OF_STAR: i32 = 4423;
const DRAGON_BUGLE_OF_TWILIGHT: i32 = 4424;

// Misc
const MIN_PLAYER_LEVEL: i32 = 45;
const MIN_HACHLING_LEVEL: i32 = 55;
/// Java `DESPAWN_GUARDIAN` timer: a guardian ghost goes after 5 minutes.
const GUARDIAN_DESPAWN_MS: i64 = 300_000;

/// Java `NpcData`: the message of a tree, its memo-state step (`memoStateMod`, `memoStateValue`)
/// and the hits it takes before the fairy leaf can drop (`minHits`).
#[derive(Clone, Copy)]
struct NpcData {
    message: NpcStringId,
    memo_state_mod: i32,
    memo_state_value: i32,
    min_hits: i32,
}

/// Java `NPC_DATA.get(npcId)`.
fn npc_data(npc_id: i32) -> Option<NpcData> {
    // retail min hits is 270, 400, 150 and 270
    match npc_id {
        TREE_OF_WIND => Some(NpcData {
            message: NpcStringId::HeyYouVeAlreadyDrunkTheEssenceOfWind,
            memo_state_mod: 2,
            memo_state_value: 1,
            min_hits: 27,
        }),
        TREE_OF_STAR => Some(NpcData {
            message: NpcStringId::HeyYouVeAlreadyDrunkTheEssenceOfAStar,
            memo_state_mod: 4,
            memo_state_value: 2,
            min_hits: 40,
        }),
        TREE_OF_TWILIGHT => Some(NpcData {
            message: NpcStringId::HeyYouVeAlreadyDrunkTheEssenceOfDusk,
            memo_state_mod: 8,
            memo_state_value: 4,
            min_hits: 15,
        }),
        TREE_OF_ABYSS => Some(NpcData {
            message: NpcStringId::HeyYouVeAlreadyDrunkTheEssenceOfTheAbyss,
            memo_state_mod: 16,
            memo_state_value: 8,
            min_hits: 27,
        }),
        _ => None,
    }
}

/// Java `getFlute(player)`: the flute the player has, the wind one first.
fn flute_id(ctx: &ScriptCtx<'_>, player: ObjectId) -> i32 {
    if ctx.has_quest_items(player, DRAGONFLUTE_OF_WIND) {
        DRAGONFLUTE_OF_WIND
    } else if ctx.has_quest_items(player, DRAGONFLUTE_OF_STAR) {
        DRAGONFLUTE_OF_STAR
    } else {
        DRAGONFLUTE_OF_TWILIGHT
    }
}

/// The control object id of the player's summon (its flute), 0 without a summon (Java
/// `player.getSummon().getControlObjectId()`).
fn summon_control(ctx: &ScriptCtx<'_>, player: ObjectId) -> i32 {
    ctx.summon_of(player)
        .and_then(|summon| ctx.world().pet_control_object_id(summon))
        .map_or(0, |control| control.0)
}

/// Java `Q00421_LittleWingsBigAdventure`.
pub struct Q00421LittleWingsBigAdventure;

impl Script for Q00421LittleWingsBigAdventure {
    fn name(&self) -> &'static str {
        "Q00421_LittleWingsBigAdventure"
    }

    fn path(&self) -> &'static str {
        "quests/Q00421_LittleWingsBigAdventure"
    }

    fn id(&self) -> i32 {
        421
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let trees = [TREE_OF_WIND, TREE_OF_STAR, TREE_OF_TWILIGHT, TREE_OF_ABYSS];
        s.add_start_npc(&[CRONOS]);
        s.add_talk_id(&[CRONOS, MIMYU]);
        s.add_attack_id(&trees);
        s.add_kill_id(&trees);
        s.register_quest_items(&[FAIRY_LEAF]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "DESPAWN_GUARDIAN" {
            if let Some(npc) = npc {
                ctx.delete_npc(npc);
            }
            // Java `super.onEvent(...)`: the base returns null.
            return None;
        }

        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        match event {
            "30610-05.htm" => {
                if qs.is_created(ctx) {
                    if ctx.get_quest_items_count_of(player, &FLUTES) == 1 {
                        let flute = flute_id(ctx, player);
                        if ctx.get_enchant_level(player, flute) < MIN_HACHLING_LEVEL {
                            Some("30610-06.html".to_owned())
                        } else {
                            let object_id = ctx.inventory_item_object_id(player, flute).map_or(0, |o| o.0);
                            qs.start_quest(ctx);
                            qs.set_memo_state(ctx, 100);
                            qs.set_int(ctx, "fluteObjectId", object_id);
                            Some(event.to_owned())
                        }
                    } else {
                        Some("30610-06.html".to_owned())
                    }
                } else {
                    None
                }
            }
            "30747-04.html" => {
                if ctx.summon_of(player).is_none() {
                    Some("30747-02.html".to_owned())
                } else if summon_control(ctx, player) != qs.get_int(ctx, "fluteObjectId") {
                    Some("30747-03.html".to_owned())
                } else {
                    Some(event.to_owned())
                }
            }
            "30747-05.html" => {
                if ctx.summon_of(player).is_none()
                    || summon_control(ctx, player) != qs.get_int(ctx, "fluteObjectId")
                {
                    Some("30747-06.html".to_owned())
                } else {
                    ctx.give_items(player, FAIRY_LEAF, 4);
                    qs.set_cond(ctx, 2, true);
                    qs.set_memo_state(ctx, 0);
                    Some(event.to_owned())
                }
            }
            "30747-07.html" | "30747-08.html" | "30747-09.html" | "30747-10.html" => Some(event.to_owned()),
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);

        match npc.id {
            CRONOS => match qs.state(ctx) {
                CREATED => {
                    let flute_count = ctx.get_quest_items_count_of(talker, &FLUTES);
                    // Java: this quest does not show up without a flute in the inventory.
                    if flute_count != 0 {
                        htmltext = if ctx.player_level(talker) < MIN_PLAYER_LEVEL {
                            "30610-01.htm"
                        } else if flute_count > 1 {
                            "30610-02.htm"
                        } else if ctx.get_enchant_level(talker, flute_id(ctx, talker)) < MIN_HACHLING_LEVEL {
                            "30610-03.html"
                        } else {
                            "30610-04.htm"
                        }
                        .to_owned();
                    }
                }
                STARTED => htmltext = "30610-07.html".to_owned(),
                COMPLETED => htmltext = ctx.get_already_completed_msg(talker),
                _ => {}
            },
            MIMYU => {
                let memo = qs.get_memo_state(ctx);
                match memo {
                    100 => {
                        qs.set_memo_state(ctx, 200);
                        htmltext = "30747-01.html".to_owned();
                    }
                    200 => {
                        htmltext = if ctx.summon_of(talker).is_none() {
                            "30747-02.html"
                        } else if summon_control(ctx, talker) != qs.get_int(ctx, "fluteObjectId") {
                            "30747-03.html"
                        } else {
                            "30747-04.html"
                        }
                        .to_owned();
                    }
                    0 => htmltext = "30747-07.html".to_owned(),
                    1..=14 => {
                        if ctx.has_quest_items(talker, FAIRY_LEAF) {
                            htmltext = "30747-11.html".to_owned();
                        }
                    }
                    15 => {
                        if !ctx.has_quest_items(talker, FAIRY_LEAF) {
                            if ctx.summon_of(talker).is_none() {
                                htmltext = "30747-12.html".to_owned();
                            } else if summon_control(ctx, talker) == qs.get_int(ctx, "fluteObjectId") {
                                qs.set_memo_state(ctx, 16);
                                htmltext = "30747-13.html".to_owned();
                            } else {
                                htmltext = "30747-14.html".to_owned();
                            }
                        }
                    }
                    16 => {
                        if !ctx.has_quest_items(talker, FAIRY_LEAF) {
                            if ctx.summon_of(talker).is_some() {
                                htmltext = "30747-15.html".to_owned();
                            } else {
                                let flute_count = ctx.get_quest_items_count_of(talker, &FLUTES);
                                if flute_count > 1 {
                                    htmltext = "30747-17.html".to_owned();
                                } else if flute_count == 1 {
                                    let flute = flute_id(ctx, talker);
                                    if ctx.inventory_item_object_id(talker, flute).map_or(0, |o| o.0)
                                        == qs.get_int(ctx, "fluteObjectId")
                                    {
                                        // TODO (Java): what if the hatchling has items in his
                                        // inventory? They are not moved here either.
                                        let bugle = match flute {
                                            DRAGONFLUTE_OF_WIND => DRAGON_BUGLE_OF_WIND,
                                            DRAGONFLUTE_OF_STAR => DRAGON_BUGLE_OF_STAR,
                                            _ => DRAGON_BUGLE_OF_TWILIGHT,
                                        };
                                        ctx.take_items(talker, flute, -1);
                                        ctx.give_items(talker, bugle, 1);
                                        qs.exit_quest(ctx, true, true);
                                        htmltext = "30747-16.html".to_owned();
                                    } else {
                                        ctx.cast_skill(npc, talker, CURSE_OF_MIMYU);
                                        htmltext = "30747-18.html".to_owned();
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        let qs = ctx.get_quest_state(attacker, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 2)
        {
            if is_summon {
                let Some(data) = npc_data(npc.id) else {
                    return;
                };
                let memo = qs.get_memo_state(ctx);
                if memo % data.memo_state_mod < data.memo_state_value {
                    if summon_control(ctx, attacker) == qs.get_int(ctx, "fluteObjectId") {
                        let hits = qs.get_int(ctx, "hits") + 1;
                        qs.set_int(ctx, "hits", hits);
                        if hits < data.min_hits {
                            if npc.id == TREE_OF_ABYSS && ctx.get_random(100) < 2 {
                                ctx.cast_skill(npc, attacker, DRYAD_ROOT);
                            }
                        } else if ctx.get_random(100) < 6 && ctx.has_quest_items(attacker, FAIRY_LEAF) {
                            // Retail is < 2 for the chance above; this is the Java chance.
                            ctx.broadcast_npc_say(
                                npc,
                                ChatType::NpcGeneral,
                                NpcStringId::GiveMeAFairyLeaf,
                                &[],
                            );
                            ctx.take_items(attacker, FAIRY_LEAF, 1);
                            qs.set_memo_state(ctx, memo + data.memo_state_value);
                            qs.unset(ctx, "hits");
                            ctx.play_sound(attacker, QuestSound::ItemsoundQuestMiddle);
                            if qs.get_memo_state(ctx) == 15 {
                                qs.set_cond(ctx, 3, false);
                            }
                        }
                    }
                } else {
                    match ctx.get_random(3) {
                        0 => ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::WhyDoYouBotherMeAgain,
                            &[],
                        ),
                        1 => ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, data.message, &[]),
                        2 => ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::LeaveNowBeforeYouIncurTheWrathOfTheGuardianGhost,
                            &[],
                        ),
                        _ => {}
                    }
                }
            } else if ctx.get_random(100) < 30 {
                ctx.cast_skill(npc, attacker, VICIOUS_POISON);
            }
        } else if ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * 0.67 && ctx.get_random(100) < 30
        {
            ctx.cast_skill(npc, attacker, VICIOUS_POISON);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if !ctx.check_if_in_range(ctx.alt_party_range(), killer, npc.object_id, true) {
            return;
        }
        for i in 0..20 {
            if let Some(guardian) = ctx.add_spawn_at(SOUL_OF_TREE_GUARDIAN, npc.object_id, false, 0) {
                ctx.start_quest_timer(
                    "DESPAWN_GUARDIAN",
                    GUARDIAN_DESPAWN_MS,
                    Some(guardian),
                    None,
                    false,
                );
            }
            if i == 0 {
                ctx.cast_skill(npc, killer, VICIOUS_POISON);
            }
            ctx.set_intention_attack(npc, killer);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q00421_LittleWingsBigAdventure";

    const NPCS: &str = r#"
        <npc id="30610" type="Folk" level="70" name="Cronos"/>
        <npc id="30747" type="Folk" level="70" name="Mimyu"/>
        <npc id="27185" type="RaidBoss" level="60" name="Tree of Wind"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="3500" type="EtcItem" name="Dragonflute of Wind"/>
        <item id="4325" type="EtcItem" name="Fairy Leaf"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="4422" type="EtcItem" name="Dragon Bugle of Wind"/>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00421LittleWingsBigAdventure);
        let pages = Pages::of(&Q00421LittleWingsBigAdventure)
            .page("30610-04.htm", &["30610-05.htm"])
            .plain(&[
                "30610-01.htm",
                "30610-02.htm",
                "30610-03.html",
                "30610-05.htm",
                "30610-06.html",
                "30610-07.html",
            ])
            .page("30747-01.html", &["30747-04.html"])
            .page("30747-04.html", &["30747-05.html"])
            .plain(&[
                "30747-02.html",
                "30747-03.html",
                "30747-05.html",
                "30747-06.html",
                "30747-07.html",
                "30747-11.html",
            ])
            .npcs(&[CRONOS, MIMYU]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let cronos = t.spawn(CRONOS, 20);
        let mimyu = t.spawn(MIMYU, 40);
        (t, cronos, mimyu)
    }

    /// A character of `level` with a wind flute of `enchant`, object id 900.
    fn enter_with_flute(t: &mut ScriptTest, level: i32, enchant: i32) {
        let mut flute = item(900, DRAGONFLUTE_OF_WIND, 1);
        flute.enchant_level = enchant;
        t.enter_with(player_data(1, level, vec![flute]));
    }

    #[test]
    fn a_low_level_character_or_a_weak_flute_is_turned_away() {
        let (mut t, cronos, _) = setup();
        enter_with_flute(&mut t, 44, 60);
        assert!(t.quest_page(1, cronos).contains("30610-01.htm"));
        assert!(t.take_writes().is_empty());

        // Level 45 with a flute of +54: the enchant is too low for the hatchling.
        let (mut t, cronos, _) = setup();
        enter_with_flute(&mut t, 45, 54);
        assert!(t.quest_page(1, cronos).contains("30610-03.html"));
    }

    #[test]
    fn the_start_links_the_flute_as_the_summon_s_control() {
        let (mut t, cronos, _) = setup();
        enter_with_flute(&mut t, 45, 55);
        assert!(t.quest_page(1, cronos).contains("30610-04.htm"));
        assert!(t.event(1, NAME, "30610-05.htm").contains("30610-05.htm"));
        let writes = t.take_writes();
        assert!(writes.contains(&rows::insert(NAME, "cond", "1")));
        assert!(
            writes.contains(&rows::insert(NAME, "fluteObjectId", "900")),
            "{writes:?}"
        );
    }

    #[test]
    fn mimyu_without_a_summon_asks_for_one() {
        let (mut t, cronos, mimyu) = setup();
        enter_with_flute(&mut t, 45, 55);
        assert!(t.quest_page(1, cronos).contains("30610-04.htm"));
        assert!(t.event(1, NAME, "30610-05.htm").contains("30610-05.htm"));
        t.take_writes();

        // The first talk of Mimyu: memo state 200; the next one has no summon to go with.
        assert!(t.quest_page(1, mimyu).contains("30747-01.html"));
        assert!(t.quest_page(1, mimyu).contains("30747-02.html"));
    }

    #[test]
    fn the_leaf_step_gives_the_quest_back_with_no_summon() {
        // Java memo states 1 to 14 need the fairy leaves, memo 0 the summon's first step.
        let (mut t, _cronos, mimyu) = setup();
        enter_with_flute(&mut t, 45, 55);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
            qs.set_memo_state(ctx, 7);
            ctx.give_items(Oid(1), FAIRY_LEAF, 1);
        });
        assert!(t.quest_page(1, mimyu).contains("30747-11.html"));
    }
}
