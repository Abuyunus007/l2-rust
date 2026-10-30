//! Quest 22: Tragedy in Von Hellmann Forest (Tifaren, Innocentin, the well and two ghosts).
// Java: dist/game/data/scripts/quests/Q00022_TragedyInVonHellmannForest/Q00022_TragedyInVonHellmannForest.java
//
// Pattern: a ghost story with shared state (`docs/design/scripts.md`, "Quest patterns"): the
// static `_tifarenOwner` (the one player whose ghost of the priest is out) and `_soulWellNpc` (the
// one soul of the well) live in `ctx.state`; the ghosts despawn through quest timers.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const INNOCENTIN: i32 = 31328;
const TIFAREN: i32 = 31334;
const WELL: i32 = 31527;
const GHOST_OF_PRIEST: i32 = 31528;
const GHOST_OF_ADVENTURER: i32 = 31529;

// Mobs
const MOBS: [i32; 5] = [
    21553, // Trampled Man
    21554, // Trampled Man
    21555, // Slaughter Executioner
    21556, // Slaughter Executioner
    21561, // Sacrificed Man
];
const SOUL_OF_WELL: i32 = 27217;

// Items
const CROSS_OF_EINHASAD: i32 = 7141;
const LOST_SKULL_OF_ELF: i32 = 7142;
const LETTER_OF_INNOCENTIN: i32 = 7143;
const JEWEL_OF_ADVENTURER_1: i32 = 7144;
const JEWEL_OF_ADVENTURER_2: i32 = 7145;
const SEALED_REPORT_BOX: i32 = 7146;
const REPORT_BOX: i32 = 7147;

// Misc
const MIN_LEVEL: i32 = 63;
const PRIEST_LOC: (i32, i32, i32) = (38354, -49777, -1128);
const SOUL_WELL_LOC: (i32, i32, i32) = (34706, -54590, -2054);
/// Java `Q00021_HiddenTruth.class.getSimpleName()`.
const HIDDEN_TRUTH: &str = "Q00021_HiddenTruth";

/// The Java statics `_tifarenOwner` and `_soulWellNpc`.
#[derive(Default)]
struct TragedyState {
    tifaren_owner: i32,
    soul_well_npc: Option<ObjectId>,
}

/// Java `Q00022_TragedyInVonHellmannForest`.
pub struct Q00022TragedyInVonHellmannForest;

impl Script for Q00022TragedyInVonHellmannForest {
    fn name(&self) -> &'static str {
        "Q00022_TragedyInVonHellmannForest"
    }

    fn path(&self) -> &'static str {
        "quests/Q00022_TragedyInVonHellmannForest"
    }

    fn id(&self) -> i32 {
        22
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&MOBS);
        s.add_kill_id(&[SOUL_OF_WELL]);
        s.add_attack_id(&[SOUL_OF_WELL]);
        s.add_start_npc(&[TIFAREN]);
        s.add_talk_id(&[INNOCENTIN, TIFAREN, WELL, GHOST_OF_PRIEST, GHOST_OF_ADVENTURER]);
        s.register_quest_items(&[
            LOST_SKULL_OF_ELF,
            CROSS_OF_EINHASAD,
            REPORT_BOX,
            JEWEL_OF_ADVENTURER_1,
            JEWEL_OF_ADVENTURER_2,
            SEALED_REPORT_BOX,
        ]);
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
            "31529-02.html" | "31529-04.html" | "31529-05.html" | "31529-06.html" | "31529-07.html"
            | "31529-09.html" | "31529-13.html" | "31529-13a.html" | "31528-02.html" | "31528-05.html"
            | "31528-06.html" | "31528-07.html" | "31328-13.html" | "31328-06.html" | "31328-05.html"
            | "31328-02.html" | "31328-07.html" | "31328-08.html" | "31328-14.html" | "31328-15.html"
            | "31328-16.html" | "31328-17.html" | "31328-18.html" | "31334-12.html" => {
                htmltext = Some(event.to_owned());
            }
            "31334-02.htm" => {
                if qs.is_created(ctx) {
                    let q21_completed = ctx
                        .player_quest_state(player, HIDDEN_TRUTH)
                        .is_some_and(|q21| q21.is_completed());
                    if ctx.player_level(player) >= MIN_LEVEL && q21_completed {
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("31334-03.html".to_owned());
                    }
                }
            }
            "31334-04.html" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "31334-06.html" => {
                if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, CROSS_OF_EINHASAD) {
                    htmltext = Some(event.to_owned());
                } else {
                    ctx.take_items(player, CROSS_OF_EINHASAD, -1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some("31334-07.html".to_owned());
                }
            }
            "31334-08.html" => {
                if qs.is_cond(ctx, 3) {
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31334-13.html" => {
                let cond = qs.get_cond(ctx);
                if (5..=7).contains(&cond) && ctx.has_quest_items(player, CROSS_OF_EINHASAD) {
                    if ctx.state::<TragedyState>().tifaren_owner == 0 {
                        ctx.state::<TragedyState>().tifaren_owner = player.0;
                        if let Some(ghost2) = ctx.add_spawn(
                            GHOST_OF_PRIEST,
                            PRIEST_LOC.0,
                            PRIEST_LOC.1,
                            PRIEST_LOC.2,
                            0,
                            true,
                            0,
                            0,
                        ) {
                            ctx.set_script_value(ghost2, player.0);
                            ctx.start_quest_timer(
                                "DESPAWN_GHOST2",
                                1000 * 120,
                                Some(ghost2),
                                Some(player),
                                false,
                            );
                            let name = ctx.player_name(player);
                            ctx.broadcast_npc_say(
                                ghost2,
                                ChatType::NpcGeneral,
                                NpcStringId::DidYouCallMeS1,
                                &[&name],
                            );
                        }
                        if (cond == 5 || cond == 6) && ctx.has_quest_items(player, LOST_SKULL_OF_ELF) {
                            ctx.take_items(player, LOST_SKULL_OF_ELF, -1);
                            qs.set_cond(ctx, 7, true);
                        }
                        htmltext = Some(event.to_owned());
                    } else {
                        qs.set_cond(ctx, 6, true);
                        htmltext = Some("31334-14.html".to_owned());
                    }
                }
            }
            "31528-04.html" => {
                let npc = npc?;
                if ctx.get_script_value(npc) == player.0 {
                    ctx.play_sound(player, QuestSound::AmbsoundHorror03);
                    htmltext = Some(event.to_owned());
                }
            }
            "31528-08.html" => {
                let npc = npc?;
                if ctx.has_quest_timer("DESPAWN_GHOST2", Some(npc), Some(player))
                    && ctx.get_script_value(npc) == player.0
                {
                    ctx.cancel_quest_timer("DESPAWN_GHOST2", Some(npc), Some(player));
                    ctx.set_script_value(npc, 0);
                    ctx.start_quest_timer("DESPAWN_GHOST2", 1000 * 3, Some(npc), Some(player), false);
                    qs.set_cond(ctx, 8, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "DESPAWN_GHOST2" => {
                let npc = npc?;
                ctx.state::<TragedyState>().tifaren_owner = 0;
                if ctx.get_script_value(npc) != 0 {
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::IMConfusedMaybeItSTimeToGoBack,
                        &[],
                    );
                }
                ctx.delete_npc(npc);
            }
            "31328-03.html" => {
                if qs.is_cond(ctx, 8) {
                    ctx.take_items(player, CROSS_OF_EINHASAD, -1);
                    htmltext = Some(event.to_owned());
                }
            }
            "31328-09.html" => {
                if qs.is_cond(ctx, 8) {
                    ctx.give_items(player, LETTER_OF_INNOCENTIN, 1);
                    qs.set_cond(ctx, 9, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31328-11.html" => {
                if qs.is_cond(ctx, 14) && ctx.has_quest_items(player, REPORT_BOX) {
                    ctx.take_items(player, REPORT_BOX, -1);
                    qs.set_cond(ctx, 15, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31328-19.html" => {
                if qs.is_cond(ctx, 15) {
                    qs.set_cond(ctx, 16, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31527-02.html" => {
                if qs.is_cond(ctx, 10) && ctx.state::<TragedyState>().soul_well_npc.is_none() {
                    if let Some(soul) = ctx.add_spawn(
                        SOUL_OF_WELL,
                        SOUL_WELL_LOC.0,
                        SOUL_WELL_LOC.1,
                        SOUL_WELL_LOC.2,
                        0,
                        true,
                        0,
                        0,
                    ) {
                        ctx.state::<TragedyState>().soul_well_npc = Some(soul.object_id);
                        ctx.start_quest_timer("activateSoulOfWell", 90000, Some(soul), Some(player), false);
                        ctx.start_quest_timer("despawnSoulOfWell", 120000, Some(soul), Some(player), false);
                        ctx.set_intention_attack(soul, player);
                    }
                    ctx.play_sound(player, QuestSound::SkillsoundAntarasFear);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("31527-03.html".to_owned());
                }
            }
            "activateSoulOfWell" => {
                // this enables onAttack ELSE IF block which allows the player to proceed the quest
                let npc = npc?;
                ctx.set_script_value(npc, 1);
            }
            "despawnSoulOfWell" => {
                // if the player fails to proceed the quest in 2 minutes, the soul is unspawned
                let npc = npc?;
                if !ctx.is_dead(npc.object_id) {
                    ctx.state::<TragedyState>().soul_well_npc = None;
                }
                ctx.delete_npc(npc);
            }
            "31529-03.html" => {
                if qs.is_cond(ctx, 9) && ctx.has_quest_items(player, LETTER_OF_INNOCENTIN) {
                    qs.set_memo_state(ctx, 8);
                    htmltext = Some(event.to_owned());
                }
            }
            "31529-08.html" => {
                if qs.is_memo_state(ctx, 8) {
                    qs.set_memo_state(ctx, 9);
                    htmltext = Some(event.to_owned());
                }
            }
            "31529-11.html" => {
                if qs.is_memo_state(ctx, 9) {
                    ctx.give_items(player, JEWEL_OF_ADVENTURER_1, 1);
                    qs.set_cond(ctx, 10, true);
                    qs.set_memo_state(ctx, 10);
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
        let Some(qs) = ctx.get_quest_state(attacker, false) else {
            return;
        };
        if qs.is_cond(ctx, 10) && ctx.has_quest_items(attacker, JEWEL_OF_ADVENTURER_1) {
            if qs.is_memo_state(ctx, 10) {
                qs.set_memo_state(ctx, 11);
            } else if ctx.is_script_value(npc, 1) {
                ctx.take_items(attacker, JEWEL_OF_ADVENTURER_1, -1);
                ctx.give_items(attacker, JEWEL_OF_ADVENTURER_2, 1);
                qs.set_cond(ctx, 11, true);
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            if npc.id == SOUL_OF_WELL {
                ctx.state::<TragedyState>().soul_well_npc = None;
            } else {
                let qs = ctx.get_quest_state(killer, false);
                if let Some(qs) = qs
                    && qs.is_cond(ctx, 4)
                    && ctx.has_quest_items(killer, CROSS_OF_EINHASAD)
                    && !ctx.has_quest_items(killer, LOST_SKULL_OF_ELF)
                    && ctx.get_random(100) < 10
                {
                    ctx.give_items(killer, LOST_SKULL_OF_ELF, 1);
                    qs.set_cond(ctx, 5, true);
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match npc.id {
            TIFAREN => match qs.get_cond(ctx) {
                0 => {
                    if qs.is_created(ctx) {
                        htmltext = "31334-01.htm".to_owned();
                    } else if qs.is_completed(ctx) {
                        htmltext = ctx.get_already_completed_msg(talker);
                    }
                }
                1 | 3 => htmltext = "31334-05.html".to_owned(),
                4 | 5 => {
                    if ctx.has_quest_items(talker, CROSS_OF_EINHASAD) {
                        if !ctx.has_quest_items(talker, LOST_SKULL_OF_ELF) {
                            htmltext = "31334-09.html".to_owned();
                        } else if ctx.state::<TragedyState>().tifaren_owner == 0 {
                            htmltext = "31334-10.html".to_owned();
                        } else {
                            htmltext = "31334-11.html".to_owned();
                        }
                    }
                }
                6 | 7 => {
                    if ctx.has_quest_items(talker, CROSS_OF_EINHASAD) {
                        let owner = ctx.state::<TragedyState>().tifaren_owner;
                        if owner == 0 {
                            htmltext = "31334-17.html".to_owned();
                        } else if owner == talker.0 {
                            htmltext = "31334-15.html".to_owned();
                        } else {
                            htmltext = "31334-16.html".to_owned();
                            qs.set_cond(ctx, 6, true);
                        }
                    }
                }
                8 => {
                    if ctx.has_quest_items(talker, CROSS_OF_EINHASAD) {
                        htmltext = "31334-18.html".to_owned();
                    }
                }
                _ => {}
            },
            GHOST_OF_PRIEST => {
                ctx.play_sound(talker, QuestSound::AmbsoundHorror15);
                if ctx.get_script_value(npc) == talker.0 {
                    htmltext = "31528-01.html".to_owned();
                } else {
                    htmltext = "31528-03.html".to_owned();
                }
            }
            INNOCENTIN => match qs.get_cond(ctx) {
                2 => {
                    if !ctx.has_quest_items(talker, CROSS_OF_EINHASAD) {
                        ctx.give_items(talker, CROSS_OF_EINHASAD, 1);
                        qs.set_cond(ctx, 3, true);
                        htmltext = "31328-01.html".to_owned();
                    }
                }
                3 => {
                    if ctx.has_quest_items(talker, CROSS_OF_EINHASAD) {
                        htmltext = "31328-01b.html".to_owned();
                    }
                }
                8 => {
                    if ctx.has_quest_items(talker, CROSS_OF_EINHASAD) {
                        htmltext = "31328-02.html".to_owned();
                    } else {
                        htmltext = "31328-04.html".to_owned();
                    }
                }
                9 => htmltext = "31328-09a.html".to_owned(),
                14 => {
                    if ctx.has_quest_items(talker, REPORT_BOX) {
                        htmltext = "31328-10.html".to_owned();
                    }
                }
                15 => htmltext = "31328-12.html".to_owned(),
                16 => {
                    ctx.add_exp_and_sp(talker, 345_966, 31_578);
                    qs.exit_quest(ctx, false, true);
                    if ctx.player_level(talker) >= MIN_LEVEL {
                        htmltext = "31328-20.html".to_owned();
                    } else {
                        htmltext = "31328-21.html".to_owned();
                    }
                }
                _ => {}
            },
            WELL => match qs.get_cond(ctx) {
                10 => {
                    if ctx.has_quest_items(talker, JEWEL_OF_ADVENTURER_1) {
                        htmltext = "31527-01.html".to_owned();
                        ctx.play_sound(talker, QuestSound::AmbsoundHorror01);
                    }
                }
                12 => {
                    if ctx.has_quest_items(talker, JEWEL_OF_ADVENTURER_2)
                        && !ctx.has_quest_items(talker, SEALED_REPORT_BOX)
                    {
                        ctx.give_items(talker, SEALED_REPORT_BOX, 1);
                        qs.set_cond(ctx, 13, true);
                        htmltext = "31527-04.html".to_owned();
                    }
                }
                13..=16 => htmltext = "31527-05.html".to_owned(),
                _ => {}
            },
            GHOST_OF_ADVENTURER => match qs.get_cond(ctx) {
                9 => {
                    if ctx.has_quest_items(talker, LETTER_OF_INNOCENTIN) {
                        match qs.get_memo_state(ctx) {
                            0 => htmltext = "31529-01.html".to_owned(),
                            8 => htmltext = "31529-03a.html".to_owned(),
                            9 => htmltext = "31529-10.html".to_owned(),
                            _ => {}
                        }
                    }
                }
                10 => {
                    if ctx.has_quest_items(talker, JEWEL_OF_ADVENTURER_1) {
                        let id = qs.get_memo_state(ctx);
                        if id == 10 {
                            htmltext = "31529-12.html".to_owned();
                        } else if id == 11 {
                            htmltext = "31529-14.html".to_owned();
                        }
                    }
                }
                11 => {
                    if ctx.has_quest_items(talker, JEWEL_OF_ADVENTURER_2)
                        && !ctx.has_quest_items(talker, SEALED_REPORT_BOX)
                    {
                        htmltext = "31529-15.html".to_owned();
                        qs.set_cond(ctx, 12, true);
                    }
                }
                13 => {
                    if ctx.has_quest_items(talker, JEWEL_OF_ADVENTURER_2)
                        && ctx.has_quest_items(talker, SEALED_REPORT_BOX)
                    {
                        ctx.give_items(talker, REPORT_BOX, 1);
                        ctx.take_items(talker, SEALED_REPORT_BOX, -1);
                        ctx.take_items(talker, JEWEL_OF_ADVENTURER_2, -1);
                        qs.set_cond(ctx, 14, true);
                        htmltext = "31529-16.html".to_owned();
                    }
                }
                14 => {
                    if ctx.has_quest_items(talker, REPORT_BOX) {
                        htmltext = "31529-17.html".to_owned();
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::COMPLETED;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00022_TragedyInVonHellmannForest";

    const NPCS: &str = r#"
        <npc id="31328" type="Folk" level="70" name="Innocentin"/>
        <npc id="31334" type="Folk" level="70" name="Tifaren"/>
        <npc id="31527" type="Folk" level="70" name="Well"/>
        <npc id="31528" type="Folk" level="70" name="Ghost of Priest"/>
        <npc id="31529" type="Folk" level="70" name="Ghost of Adventurer"/>
        <npc id="21553" type="Monster" level="60" name="Trampled Man"/>
        <npc id="27217" type="Monster" level="60" name="Soul of Well"/>"#;

    const ITEMS: &str = r#"
        <item id="7141" type="EtcItem" name="Cross of Einhasad"><set name="is_questitem" val="true"/></item>
        <item id="7142" type="EtcItem" name="Lost Skull of Elf"><set name="is_questitem" val="true"/></item>
        <item id="7143" type="EtcItem" name="Letter of Innocentin"><set name="is_questitem" val="true"/></item>
        <item id="7144" type="EtcItem" name="Jewel of Adventurer 1"><set name="is_questitem" val="true"/></item>
        <item id="7145" type="EtcItem" name="Jewel of Adventurer 2"><set name="is_questitem" val="true"/></item>
        <item id="7146" type="EtcItem" name="Sealed Report Box"><set name="is_questitem" val="true"/></item>
        <item id="7147" type="EtcItem" name="Report Box"><set name="is_questitem" val="true"/></item>"#;

    /// `Q00021_HiddenTruth` as a stand-in: its state is set by the test.
    struct HiddenTruth;

    impl Script for HiddenTruth {
        fn name(&self) -> &'static str {
            HIDDEN_TRUTH
        }
        fn path(&self) -> &'static str {
            "quests/Q00021_HiddenTruth"
        }
        fn id(&self) -> i32 {
            21
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        tifaren: Oid,
        innocentin: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(HiddenTruth);
        registry.add(Q00022TragedyInVonHellmannForest);
        let pages = Pages::of(&Q00022TragedyInVonHellmannForest)
            .page("31334-01.htm", &["31334-02.htm"])
            .page("31334-02.htm", &["31334-04.html"])
            .plain(&["31334-03.html", "31334-04.html", "31334-05.html"])
            .npcs(&[TIFAREN, INNOCENTIN, WELL, GHOST_OF_PRIEST, GHOST_OF_ADVENTURER]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            tifaren: t.spawn(TIFAREN, 20),
            innocentin: t.spawn(INNOCENTIN, 40),
        };
        (t, npcs)
    }

    fn finish_hidden_truth(t: &mut ScriptTest) {
        t.world.run_as_script(HIDDEN_TRUTH, |ctx| {
            if let Some(qs) = ctx.quest_state_of(HIDDEN_TRUTH, Oid(1), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
        writes(t);
    }

    fn ev(t: &mut ScriptTest, event: &str, npc: Option<Oid>) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            out = Q00022TragedyInVonHellmannForest.on_event(ctx, event, npc, Some(Oid(1)));
        });
        out
    }

    fn talk(t: &mut ScriptTest, npc: Oid) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(npc) {
                out = Q00022TragedyInVonHellmannForest.on_talk(ctx, npc, Oid(1));
            }
        });
        out
    }

    fn set_cond(t: &mut ScriptTest, cond: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_cond(ctx, cond, false);
            }
        });
        writes(t);
    }

    /// The writes without the journal flag rows (`__compltdStateFlags`) the high conds add.
    fn writes(t: &ScriptTest) -> Vec<l2_world::QuestWrite> {
        t.take_writes()
            .into_iter()
            .filter(|w| !format!("{w:?}").contains("__compltdStateFlags"))
            .collect()
    }

    fn give(t: &mut ScriptTest, item: i32) {
        t.world.run_as_script(NAME, |ctx| ctx.give_items(Oid(1), item, 1));
    }

    #[test]
    fn it_needs_level_63_and_the_hidden_truth() {
        let (mut t, npcs) = setup();
        t.enter(1, 63, vec![]);
        assert!(t.quest_page(1, npcs.tifaren).contains("31334-01.htm"));
        // Q21 not finished: the refusal page.
        assert!(t.event(1, NAME, "31334-02.htm").contains("31334-03.html"));
        assert!(writes(&t).is_empty());

        let (mut t, npcs) = setup();
        t.enter(1, 62, vec![]);
        finish_hidden_truth(&mut t);
        assert!(t.quest_page(1, npcs.tifaren).contains("31334-01.htm"));
        assert!(t.event(1, NAME, "31334-02.htm").contains("31334-03.html"));
    }

    #[test]
    fn the_cross_the_skull_and_the_priest_ghost() {
        let (mut t, npcs) = setup();
        t.enter(1, 63, vec![]);
        finish_hidden_truth(&mut t);
        assert!(t.quest_page(1, npcs.tifaren).contains("31334-01.htm"));
        assert!(t.event(1, NAME, "31334-02.htm").contains("31334-02.htm"));
        assert!(t.event(1, NAME, "31334-04.html").contains("31334-04.html"));
        assert_eq!(
            writes(&t),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Cond 2 (Java moves 1 -> 2 on the "31334-06" leave path, here set directly): Innocentin
        // hands the cross of Einhasad: cond 3.
        set_cond(&mut t, 2);
        assert_eq!(talk(&mut t, npcs.innocentin).as_deref(), Some("31328-01.html"));
        assert_eq!(t.count(1, CROSS_OF_EINHASAD), 1);
        assert_eq!(writes(&t), [rows::update(NAME, "cond", "3")]);
        assert_eq!(
            ev(&mut t, "31334-08.html", None).as_deref(),
            Some("31334-08.html")
        );
        assert_eq!(writes(&t), [rows::update(NAME, "cond", "4")]);

        // The skull drops with a 10% roll from the trampled men: cond 5.
        for _ in 0..500 {
            if t.count(1, LOST_SKULL_OF_ELF) > 0 {
                break;
            }
            let mob = t.spawn(21553, 60);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, LOST_SKULL_OF_ELF), 1);
        writes(&t);

        // The priest ghost: the owner is Tifaren's, the skull is taken: cond 7.
        assert_eq!(
            ev(&mut t, "31334-13.html", None).as_deref(),
            Some("31334-13.html")
        );
        assert_eq!(t.count(1, LOST_SKULL_OF_ELF), 0);
        assert_eq!(writes(&t), [rows::update(NAME, "cond", "7")]);
        let ghost = t.npcs_of(GHOST_OF_PRIEST)[0];
        assert_eq!(talk(&mut t, ghost).as_deref(), Some("31528-01.html"));
        assert_eq!(talk(&mut t, npcs.tifaren).as_deref(), Some("31334-15.html"));

        // The ghost is released: cond 8, and the despawn timer clears the owner.
        assert_eq!(
            ev(&mut t, "31528-08.html", Some(ghost)).as_deref(),
            Some("31528-08.html")
        );
        assert_eq!(writes(&t), [rows::update(NAME, "cond", "8")]);
        t.wait_ms(3_500);
        assert!(!t.in_world(ghost));
        assert_eq!(talk(&mut t, npcs.tifaren).as_deref(), Some("31334-18.html"));
    }

    #[test]
    fn the_well_the_jewels_the_report_and_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 63, vec![]);
        finish_hidden_truth(&mut t);
        t.quest_page(1, npcs.tifaren);
        t.event(1, NAME, "31334-02.htm");
        t.event(1, NAME, "31334-04.html");
        give(&mut t, CROSS_OF_EINHASAD);
        set_cond(&mut t, 8);

        // Innocentin takes the cross and sends the player to the adventurer: cond 9.
        assert_eq!(
            ev(&mut t, "31328-03.html", None).as_deref(),
            Some("31328-03.html")
        );
        assert_eq!(t.count(1, CROSS_OF_EINHASAD), 0);
        assert_eq!(
            ev(&mut t, "31328-09.html", None).as_deref(),
            Some("31328-09.html")
        );
        assert_eq!(t.count(1, LETTER_OF_INNOCENTIN), 1);
        writes(&t);

        // The adventurer's ghost: memo 8, 9, then the first jewel: cond 10, memo 10.
        assert_eq!(
            ev(&mut t, "31529-03.html", None).as_deref(),
            Some("31529-03.html")
        );
        assert_eq!(
            ev(&mut t, "31529-08.html", None).as_deref(),
            Some("31529-08.html")
        );
        assert_eq!(
            ev(&mut t, "31529-11.html", None).as_deref(),
            Some("31529-11.html")
        );
        assert_eq!(t.count(1, JEWEL_OF_ADVENTURER_1), 1);
        assert_eq!(
            writes(&t),
            [
                rows::insert(NAME, "memoState", "8"),
                rows::update(NAME, "memoState", "9"),
                rows::update(NAME, "cond", "10"),
                rows::update(NAME, "memoState", "10"),
            ]
        );

        // The well wakes the soul; the first hit moves the memo to 11, a hit after
        // `activateSoulOfWell` gives the second jewel (cond 11).
        assert_eq!(
            ev(&mut t, "31527-02.html", None).as_deref(),
            Some("31527-02.html")
        );
        let soul = t.npcs_of(SOUL_OF_WELL)[0];
        assert_eq!(
            ev(&mut t, "31527-02.html", None).as_deref(),
            Some("31527-03.html")
        );
        let hit = |t: &mut ScriptTest| {
            t.world.run_as_script(NAME, |ctx| {
                if let Some(npc) = ctx.script_npc(soul) {
                    Q00022TragedyInVonHellmannForest.on_attack(ctx, npc, Oid(1), 1, false, None);
                }
            });
        };
        writes(&t);
        hit(&mut t);
        assert_eq!(writes(&t), [rows::update(NAME, "memoState", "11")]);
        hit(&mut t);
        assert_eq!(t.count(1, JEWEL_OF_ADVENTURER_2), 0);
        assert_eq!(ev(&mut t, "activateSoulOfWell", Some(soul)), None);
        hit(&mut t);
        assert_eq!(t.count(1, JEWEL_OF_ADVENTURER_1), 0);
        assert_eq!(t.count(1, JEWEL_OF_ADVENTURER_2), 1);
        assert_eq!(writes(&t), [rows::update(NAME, "cond", "11")]);
        t.world.delete_npc(soul);

        // Ghost (cond 12), well (sealed box, cond 13), ghost (report box, cond 14).
        let ghost = t.spawn(GHOST_OF_ADVENTURER, 30);
        let well = t.spawn(WELL, 35);
        assert_eq!(talk(&mut t, ghost).as_deref(), Some("31529-15.html"));
        assert_eq!(talk(&mut t, well).as_deref(), Some("31527-04.html"));
        assert_eq!(talk(&mut t, ghost).as_deref(), Some("31529-16.html"));
        assert_eq!(t.count(1, REPORT_BOX), 1);
        assert_eq!(
            writes(&t),
            [
                rows::update(NAME, "cond", "12"),
                rows::update(NAME, "cond", "13"),
                rows::update(NAME, "cond", "14"),
            ]
        );

        // Innocentin: report box taken (cond 15), cond 16, then the reward.
        assert_eq!(
            ev(&mut t, "31328-11.html", None).as_deref(),
            Some("31328-11.html")
        );
        assert_eq!(
            ev(&mut t, "31328-19.html", None).as_deref(),
            Some("31328-19.html")
        );
        writes(&t);
        assert_eq!(talk(&mut t, npcs.innocentin).as_deref(), Some("31328-20.html"));
        assert_eq!(
            writes(&t),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
