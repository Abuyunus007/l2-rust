//! The base of the 34 sagas (the third class change quests, Q00067..Q00100): the whole quest
//! is here, each saga only brings its NPCs, items, monsters, classes, places and texts.
// Java: dist/game/data/scripts/quests/AbstractSagaQuest.java
//
// Java's `abstract class AbstractSagaQuest extends Quest` with protected array fields becomes a
// [`SagaData`] constant per saga plus free functions that take it; [`saga_quest!`] writes the
// `impl Script` of a saga (the hooks the base overrides, each calling the function here). A
// saga file is its constant and one `saga_quest!` line (see `q00070_saga_of_the_phoenix_knight`).
//
// Java's `static SPAWN_LIST` (who a quest monster was spawned for) is one map for all sagas:
// [`ScriptCtx::shared_state`].

use std::collections::HashMap;

use l2_protocol_game::server::{ChatType, NpcSay};
use l2_util::ObjectId;
use l2_world::{Qs, ScriptCtx, ScriptNpc, ScriptSetup};

/// What a saga sets in its constructor (Java's protected fields).
pub struct SagaData {
    /// Java `getId()` of the saga (`super(questId)`).
    pub quest_id: i32,
    /// `_npc`: 0 the start and end NPC, 1..3 and 5..11 the steps, 4 the NPC that is spawned
    /// at step 17 (it has a first-talk page).
    pub npc: [i32; 12],
    /// `_items`: 0 and 2 are not quest items (the Ice Crystal and the Divine Stone of Wisdom),
    /// 3 the Halisha marks, 10 the item of the accept, 11 an optional second item of step 4.
    pub items: [i32; 12],
    /// `_mob`: 0 the monster of step 8, 1 the Archon of Halisha of step 15, 2 the monster of
    /// step 17.
    pub mob: [i32; 3],
    /// `_classId`: the new class (`[1]` for a Female Soul Breaker in saga 68).
    pub class_id: &'static [i32],
    /// `_prevClass`: the class the saga is for.
    pub prev_class: &'static [i32],
    /// `_npcSpawnLocations`: where `mob[0]`, `mob[2]` and `npc[4]` appear.
    pub npc_spawn_locations: [(i32, i32, i32); 3],
    /// `_text`: the monsters' lines; `PLAYERNAME` is replaced with a player's name.
    pub text: [&'static str; 18],
}

/// Java `AbstractSagaQuest.SPAWN_LIST`: a spawned quest monster -> the player it is for.
#[derive(Default)]
struct SpawnList(HashMap<ObjectId, ObjectId>);

// @formatter:off
const QUEST_CLASSES: [&[i32]; 34] = [
    &[0x7f],
    &[0x80, 0x81],
    &[0x82],
    &[0x05],
    &[0x14],
    &[0x15],
    &[0x02],
    &[0x03],
    &[0x2e],
    &[0x30],
    &[0x33],
    &[0x34],
    &[0x08],
    &[0x17],
    &[0x24],
    &[0x09],
    &[0x18],
    &[0x25],
    &[0x10],
    &[0x11],
    &[0x1e],
    &[0x0c],
    &[0x1b],
    &[0x28],
    &[0x0e],
    &[0x1c],
    &[0x29],
    &[0x0d],
    &[0x06],
    &[0x22],
    &[0x21],
    &[0x2b],
    &[0x37],
    &[0x39],
];
// @formatter:on

const ARCHON_HALISHA_NORM: [i32; 5] = [18212, 18214, 18215, 18216, 18218];

/// Java `21646 <= id < 21652`: the archon's minions.
const ARCHON_MINIONS: std::ops::Range<i32> = 21646..21652;
/// Java `27214 <= id < 27217`: the guardian angels of step 6.
const GUARDIAN_ANGELS: std::ops::Range<i32> = 27214..27217;

/// Writes the `impl Script` of a saga: `saga_quest!(Q00070SagaOfThePhoenixKnight,
/// "Q00070_SagaOfThePhoenixKnight", DATA);` (the type, Java's class name, its [`SagaData`]).
macro_rules! saga_quest {
    ($ty:ident, $name:literal, $data:expr) => {
        impl l2_world::Script for $ty {
            fn name(&self) -> &'static str {
                $name
            }

            fn path(&self) -> &'static str {
                concat!("quests/", $name)
            }

            fn id(&self) -> i32 {
                $data.quest_id
            }

            fn setup(&self, s: &mut l2_world::ScriptSetup) {
                $crate::quests::abstract_saga_quest::register_npcs(&$data, s);
            }

            fn on_event(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                event: &str,
                npc: Option<l2_world::ScriptNpc>,
                player: Option<l2_util::ObjectId>,
            ) -> Option<String> {
                $crate::quests::abstract_saga_quest::on_event(&$data, ctx, event, npc, player)
            }

            fn on_attack(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                npc: l2_world::ScriptNpc,
                attacker: l2_util::ObjectId,
                _damage: i32,
                _is_summon: bool,
                _skill: Option<l2_world::SkillKey>,
            ) {
                $crate::quests::abstract_saga_quest::on_attack(&$data, ctx, npc, attacker);
            }

            fn on_first_talk(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                npc: l2_world::ScriptNpc,
                player: l2_util::ObjectId,
            ) -> Option<String> {
                $crate::quests::abstract_saga_quest::on_first_talk(&$data, ctx, npc, player)
            }

            fn on_kill(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                npc: l2_world::ScriptNpc,
                killer: l2_util::ObjectId,
                _is_summon: bool,
            ) {
                $crate::quests::abstract_saga_quest::on_kill(&$data, ctx, npc, killer);
            }

            fn on_skill_see(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                npc: l2_world::ScriptNpc,
                caster: l2_util::ObjectId,
                _skill: l2_world::SkillKey,
                targets: &[l2_util::ObjectId],
                _is_summon: bool,
            ) {
                $crate::quests::abstract_saga_quest::on_skill_see(&$data, ctx, npc, caster, targets);
            }

            fn on_talk(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                npc: l2_world::ScriptNpc,
                player: l2_util::ObjectId,
            ) -> Option<String> {
                $crate::quests::abstract_saga_quest::on_talk(&$data, ctx, npc, player)
            }
        }
    };
}
pub(crate) use saga_quest;

/// Java `registerNPCs()`, the last line of every saga's constructor.
pub fn register_npcs(d: &SagaData, s: &mut ScriptSetup) {
    s.add_start_npc(&[d.npc[0]]);
    s.add_attack_id(&[d.mob[2]]);
    s.add_attack_id(&[d.mob[1]]);
    s.add_skill_see_id(&[d.mob[1]]);
    s.add_first_talk_id(&[d.npc[4]]);
    s.add_talk_id(&d.npc);
    s.add_kill_id(&d.mob);
    let mut quest_item_ids = d.items;
    // remove Ice Crystal and Divine Stone of Wisdom
    quest_item_ids[0] = 0;
    quest_item_ids[2] = 0;
    s.register_quest_items(&quest_item_ids);
    s.add_kill_id(&ARCHON_MINIONS.collect::<Vec<_>>());
    s.add_kill_id(&ARCHON_HALISHA_NORM);
    s.add_kill_id(&GUARDIAN_ANGELS.collect::<Vec<_>>());
}

/// `text.replace("PLAYERNAME", player.getName())`.
fn text_for(ctx: &ScriptCtx<'_>, d: &SagaData, index: usize, player: ObjectId) -> String {
    d.text[index].replace("PLAYERNAME", &ctx.player_name(player))
}

/// Java `onEvent(event, npc, player)`.
pub fn on_event(
    d: &SagaData,
    ctx: &mut ScriptCtx<'_>,
    event: &str,
    npc: Option<ScriptNpc>,
    player: Option<ObjectId>,
) -> Option<String> {
    let player = player?;
    let st = ctx.get_quest_state(player, false)?;

    let page = |name: &str| Some(name.to_owned());
    let mut htmltext = None;
    match event {
        "0-011.htm" | "0-012.htm" | "0-013.htm" | "0-014.htm" | "0-015.htm" => {
            htmltext = page(event);
        }
        "accept" => {
            st.start_quest(ctx);
            ctx.give_items(player, d.items[10], 1);
            htmltext = page("0-03.htm");
        }
        "0-1" => {
            if ctx.player_level(player) < 76 {
                htmltext = page("0-02.htm");
                if st.is_created(ctx) {
                    // NOTE(java-quirk): `exitQuest` of a state that was never started returns
                    // at once, so this does nothing.
                    st.exit_quest(ctx, true, false);
                }
            } else {
                htmltext = page("0-05.htm");
            }
        }
        "0-2" => {
            if ctx.player_level(player) < 76 {
                ctx.take_items(player, d.items[10], -1);
                st.set_cond(ctx, 20, true);
                htmltext = page("0-08.htm");
            } else {
                ctx.take_items(player, d.items[10], -1);
                ctx.add_exp_and_sp(player, 2299404, 0);
                ctx.give_adena(player, 5000000, true);
                ctx.give_items(player, 6622, 1);
                change_class(d, ctx, player);
                cast(ctx, npc, player, 4339, 1);
                st.exit_quest(ctx, false, false);
                htmltext = page("0-07.htm");
            }
        }
        "1-3" => {
            st.set_cond(ctx, 3, false);
            htmltext = page("1-05.htm");
        }
        "1-4" => {
            st.set_cond(ctx, 4, false);
            ctx.take_items(player, d.items[0], 1);
            if d.items[11] != 0 {
                ctx.take_items(player, d.items[11], 1);
            }
            ctx.give_items(player, d.items[1], 1);
            htmltext = page("1-06.htm");
        }
        "2-1" => {
            st.set_cond(ctx, 2, false);
            htmltext = page("2-05.htm");
        }
        "2-2" => {
            st.set_cond(ctx, 5, false);
            ctx.take_items(player, d.items[1], 1);
            ctx.give_items(player, d.items[4], 1);
            htmltext = page("2-06.htm");
        }
        "3-5" => htmltext = page("3-07.htm"),
        "3-6" => {
            st.set_cond(ctx, 11, false);
            htmltext = page("3-02.htm");
        }
        "3-7" => {
            st.set_cond(ctx, 12, false);
            htmltext = page("3-03.htm");
        }
        "3-8" => {
            st.set_cond(ctx, 13, false);
            ctx.take_items(player, d.items[2], 1);
            ctx.give_items(player, d.items[7], 1);
            htmltext = page("3-08.htm");
        }
        "4-1" => htmltext = page("4-010.htm"),
        "4-2" => {
            ctx.give_items(player, d.items[9], 1);
            st.set_cond(ctx, 18, true);
            htmltext = page("4-011.htm");
        }
        "4-3" => {
            ctx.give_items(player, d.items[9], 1);
            st.set_cond(ctx, 18, true);
            if let Some(npc) = npc {
                let text = text_for(ctx, d, 13, player);
                auto_chat(ctx, npc, &text);
            }
            st.set(ctx, "Quest0", "0");
            ctx.cancel_quest_timer("Mob_2 has despawned", npc, Some(player));
            delete_spawn(ctx, npc);
            return None;
        }
        "5-1" => {
            st.set_cond(ctx, 6, true);
            ctx.take_items(player, d.items[4], 1);
            cast(ctx, npc, player, 4546, 1);
            htmltext = page("5-02.htm");
        }
        "6-1" => {
            st.set_cond(ctx, 8, true);
            st.set(ctx, "spawned", "0");
            ctx.take_items(player, d.items[5], 1);
            cast(ctx, npc, player, 4546, 1);
            htmltext = page("6-03.htm");
        }
        "7-1" => {
            if st.get_int(ctx, "spawned") == 1 {
                htmltext = page("7-03.htm");
            } else if st.get_int(ctx, "spawned") == 0 {
                let (x, y, z) = d.npc_spawn_locations[0];
                let mob1 = ctx.add_spawn(d.mob[0], x, y, z, 0, false, 0, 0);
                st.set(ctx, "spawned", "1");
                ctx.start_quest_timer("Mob_1 Timer 1", 500, mob1, Some(player), false);
                ctx.start_quest_timer("Mob_1 has despawned", 300000, mob1, Some(player), false);
                add_spawn(ctx, st, mob1);
                htmltext = page("7-02.htm");
            } else {
                htmltext = page("7-04.htm");
            }
        }
        "7-2" => {
            st.set_cond(ctx, 10, true);
            ctx.take_items(player, d.items[6], 1);
            cast(ctx, npc, player, 4546, 1);
            htmltext = page("7-06.htm");
        }
        "8-1" => {
            st.set_cond(ctx, 14, true);
            ctx.take_items(player, d.items[7], 1);
            cast(ctx, npc, player, 4546, 1);
            htmltext = page("8-02.htm");
        }
        "9-1" => {
            st.set_cond(ctx, 17, true);
            st.set(ctx, "Quest0", "0");
            st.set(ctx, "Tab", "0");
            ctx.take_items(player, d.items[8], 1);
            cast(ctx, npc, player, 4546, 1);
            htmltext = page("9-03.htm");
        }
        "10-1" => {
            if st.get_int(ctx, "Quest0") == 0 {
                let (x, y, z) = d.npc_spawn_locations[1];
                let mob3 = ctx.add_spawn(d.mob[2], x, y, z, 0, false, 0, 0);
                let (x, y, z) = d.npc_spawn_locations[2];
                let mob2 = ctx.add_spawn(d.npc[4], x, y, z, 0, false, 0, 0);
                add_spawn(ctx, st, mob3);
                add_spawn(ctx, st, mob2);
                // NOTE(java-quirk): Java would throw here when a spawn failed; this server
                // goes on without it.
                let mob2_id = mob2.map_or(0, |m| m.object_id.0);
                st.set(ctx, "Mob_2", &mob2_id.to_string());
                st.set(ctx, "Quest0", "1");
                st.set(ctx, "Quest1", "45");
                ctx.start_quest_timer("Mob_3 Timer 1", 500, mob3, Some(player), true);
                ctx.start_quest_timer("Mob_3 has despawned", 59000, mob3, Some(player), false);
                ctx.start_quest_timer("Mob_2 Timer 1", 500, mob2, Some(player), false);
                ctx.start_quest_timer("Mob_2 has despawned", 60000, mob2, Some(player), false);
                htmltext = page("10-02.htm");
            } else if st.get_int(ctx, "Quest1") == 45 {
                htmltext = page("10-03.htm");
            } else {
                htmltext = page("10-04.htm");
            }
        }
        "10-2" => {
            st.set_cond(ctx, 19, true);
            ctx.take_items(player, d.items[9], 1);
            cast(ctx, npc, player, 4546, 1);
            htmltext = page("10-06.htm");
        }
        "11-9" => {
            st.set_cond(ctx, 15, false);
            htmltext = page("11-03.htm");
        }
        "Mob_1 Timer 1" => {
            say(ctx, d, npc, 0, player);
            return None;
        }
        "Mob_1 has despawned" => {
            say(ctx, d, npc, 1, player);
            st.set(ctx, "spawned", "0");
            delete_spawn(ctx, npc);
            return None;
        }
        "Archon Hellisha has despawned" => {
            say(ctx, d, npc, 6, player);
            st.set(ctx, "spawned", "0");
            delete_spawn(ctx, npc);
            return None;
        }
        "Mob_3 Timer 1" => {
            let npc = npc?;
            // `findSpawn(player, World.findObject(st.getInt("Mob_2")).asNpc())`.
            let mob2_id = ObjectId(st.get_int(ctx, "Mob_2"));
            let mob2 = ctx.script_npc(mob2_id).filter(|m| find_spawn(ctx, player, *m));
            // `World.getFirstVisibleObject(npc, Npc.class, it -> it == mob2) != null`.
            if let Some(mob2) = mob2
                && ctx
                    .world()
                    .visible_objects(npc.object_id)
                    .contains(&mob2.object_id)
            {
                ctx.world_mut()
                    .add_damage_hate(npc.object_id, mob2.object_id, 0, 99999);
                ctx.world_mut()
                    .set_intention_attack(npc.object_id, mob2.object_id);
                ctx.world_mut()
                    .set_intention_attack(mob2.object_id, npc.object_id);
                say(ctx, d, Some(npc), 14, player);
                ctx.cancel_quest_timer("Mob_3 Timer 1", Some(npc), Some(player));
            }
            return None;
        }
        "Mob_3 has despawned" => {
            say(ctx, d, npc, 15, player);
            st.set(ctx, "Quest0", "2");
            delete_spawn(ctx, npc);
            return None;
        }
        "Mob_2 Timer 1" => {
            say(ctx, d, npc, 7, player);
            ctx.start_quest_timer("Mob_2 Timer 2", 1500, npc, Some(player), false);
            if st.get_int(ctx, "Quest1") == 45 {
                st.set(ctx, "Quest1", "0");
            }
            return None;
        }
        "Mob_2 Timer 2" => {
            say(ctx, d, npc, 8, player);
            ctx.start_quest_timer("Mob_2 Timer 3", 10000, npc, Some(player), false);
            return None;
        }
        "Mob_2 Timer 3" => {
            if st.get_int(ctx, "Quest0") == 0 {
                ctx.start_quest_timer("Mob_2 Timer 3", 13000, npc, Some(player), false);
                if ctx.get_random_boolean() {
                    say(ctx, d, npc, 9, player);
                } else {
                    say(ctx, d, npc, 10, player);
                }
            }
            return None;
        }
        "Mob_2 has despawned" => {
            let quest1 = st.get_int(ctx, "Quest1") + 1;
            st.set(ctx, "Quest1", &quest1.to_string());
            if st.get_int(ctx, "Quest0") == 1
                || st.get_int(ctx, "Quest0") == 2
                || st.get_int(ctx, "Quest1") > 3
            {
                st.set(ctx, "Quest0", "0");

                // TODO: This "if" will never be true.
                if st.get_int(ctx, "Quest0") == 1 {
                    say(ctx, d, npc, 11, player);
                } else {
                    say(ctx, d, npc, 12, player);
                }
                delete_spawn(ctx, npc);
            } else {
                ctx.start_quest_timer("Mob_2 has despawned", 1000, npc, Some(player), false);
            }
            return None;
        }
        _ => {}
    }
    htmltext
}

/// Java `onAttack(npc, player, damage, isSummon)`.
pub fn on_attack(d: &SagaData, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) {
    let Some(st2) = find_right_state(ctx, npc) else {
        return;
    };
    let cond = st2.get_cond(ctx);
    let st = ctx.get_quest_state(player, false);
    let npc_id = npc.id;
    if npc_id == d.mob[2]
        && let Some(st) = st.filter(|st| *st == st2)
        && cond == 17
    {
        let mut quest0 = st.get_int(ctx, "Quest0") + 1;
        if quest0 == 1 {
            say(ctx, d, Some(npc), 16, player);
        }

        if quest0 > 15 {
            quest0 = 1;
            say(ctx, d, Some(npc), 17, player);
            ctx.cancel_quest_timer("Mob_3 has despawned", Some(npc), Some(st2.player()));
            st.set(ctx, "Tab", "1");
            delete_spawn(ctx, Some(npc));
        }

        st.set(ctx, "Quest0", &quest0.to_string());
    } else if npc_id == d.mob[1] && cond == 15 && (st != Some(st2) || ctx.is_in_party(player)) {
        say(ctx, d, Some(npc), 5, player);
        ctx.cancel_quest_timer("Archon Hellisha has despawned", Some(npc), Some(st2.player()));
        st2.set(ctx, "spawned", "0");
        delete_spawn(ctx, Some(npc));
    }
}

/// Java `onFirstTalk(npc, player)`.
pub fn on_first_talk(
    d: &SagaData,
    ctx: &mut ScriptCtx<'_>,
    npc: ScriptNpc,
    player: ObjectId,
) -> Option<String> {
    let mut htmltext = "";
    let st = ctx.get_quest_state(player, false);
    let npc_id = npc.id;
    if let Some(st) = st
        && npc_id == d.npc[4]
    {
        let cond = st.get_cond(ctx);
        if cond == 17 {
            if let Some(st2) = find_right_state(ctx, npc) {
                ctx.set_last_quest_npc_object(player, npc);
                // NOTE(java-quirk): "Tab" and "Quest0" are read from the talker's state, also
                // when the NPC was spawned for someone else.
                let tab = st.get_int(ctx, "Tab");
                let quest0 = st.get_int(ctx, "Quest0");
                if st == st2 {
                    if tab == 1 {
                        if quest0 == 0 {
                            htmltext = "4-04.htm";
                        } else if quest0 == 1 {
                            htmltext = "4-06.htm";
                        }
                    } else if quest0 == 0 {
                        htmltext = "4-01.htm";
                    } else if quest0 == 1 {
                        htmltext = "4-03.htm";
                    }
                } else if tab == 1 {
                    if quest0 == 0 {
                        htmltext = "4-05.htm";
                    } else if quest0 == 1 {
                        htmltext = "4-07.htm";
                    }
                } else if quest0 == 0 {
                    htmltext = "4-02.htm";
                }
            }
        } else if cond == 18 {
            htmltext = "4-08.htm";
        }
    }

    if htmltext.is_empty() {
        ctx.show_chat_window(npc, player);
    }
    Some(htmltext.to_owned())
}

/// Java `onKill(npc, player, isSummon)`.
pub fn on_kill(d: &SagaData, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) {
    let npc_id = npc.id;
    let st = ctx.get_quest_state(player, false);
    if ARCHON_MINIONS.contains(&npc_id) {
        if ctx.is_in_party(player) {
            let mut party_quest_members = Vec::new();
            for player1 in ctx.world().party_members_of(player) {
                let st1 = find_quest(d, ctx, player1);
                if let Some(st1) = st1
                    && st1.is_cond(ctx, 15)
                    && ctx.is_inside_radius_2d(player1, player, ctx.alt_party_range())
                {
                    party_quest_members.push(st1);
                }
            }

            if !party_quest_members.is_empty() {
                let index = ctx.get_random(party_quest_members.len() as i32) as usize;
                if let Some(&st2) = party_quest_members.get(index) {
                    give_halisha_mark(d, ctx, st2);
                }
            }
        } else {
            let st1 = find_quest(d, ctx, player);
            if let Some(st1) = st1
                && st1.is_cond(ctx, 15)
            {
                give_halisha_mark(d, ctx, st1);
            }
        }
        return;
    }

    if ARCHON_HALISHA_NORM.contains(&npc_id) {
        let st1 = find_quest(d, ctx, player);
        if let Some(st1) = st1
            && st1.is_cond(ctx, 15)
        {
            // This is just a guess....not really sure what it actually says, if anything
            say(ctx, d, Some(npc), 4, st1.player());
            ctx.give_items(player, d.items[8], 1);
            ctx.take_items(player, d.items[3], -1);
            st1.set_cond(ctx, 16, true);
        }
        return;
    }

    if GUARDIAN_ANGELS.contains(&npc_id) {
        let st1 = find_quest(d, ctx, player);
        if let Some(st1) = st1
            && st1.is_cond(ctx, 6)
        {
            let kills = st1.get_int(ctx, "kills");
            if kills < 9 {
                st1.set(ctx, "kills", &(kills + 1).to_string());
            } else {
                ctx.give_items(player, d.items[5], 1);
                // `st` is `st1`: `findQuest` returns the killer's state.
                if let Some(st) = st {
                    st.set_cond(ctx, 7, true);
                }
            }
        }
        return;
    }

    if let Some(st) = st
        && npc_id != d.mob[2]
    {
        let Some(st2) = find_right_state(ctx, npc) else {
            return;
        };
        let cond = st.get_cond(ctx);
        if npc_id == d.mob[0] && cond == 8 {
            if !ctx.is_in_party(player) && st == st2 {
                say(ctx, d, Some(npc), 12, player);
                ctx.give_items(player, d.items[6], 1);
                st.set_cond(ctx, 9, true);
            }

            ctx.cancel_quest_timer("Mob_1 has despawned", Some(npc), Some(st2.player()));
            st2.set(ctx, "spawned", "0");
            delete_spawn(ctx, Some(npc));
        } else if npc_id == d.mob[1] && cond == 15 {
            if !ctx.is_in_party(player) {
                if st == st2 {
                    say(ctx, d, Some(npc), 4, player);
                    ctx.give_items(player, d.items[8], 1);
                    ctx.take_items(player, d.items[3], -1);
                    st.set_cond(ctx, 16, true);
                } else {
                    say(ctx, d, Some(npc), 5, player);
                }
            }

            ctx.cancel_quest_timer("Archon Hellisha has despawned", Some(npc), Some(st2.player()));
            st2.set(ctx, "spawned", "0");
            delete_spawn(ctx, Some(npc));
        }
    } else if npc_id == d.mob[0] {
        if let Some(st) = find_right_state(ctx, npc) {
            ctx.cancel_quest_timer("Mob_1 has despawned", Some(npc), Some(st.player()));
            st.set(ctx, "spawned", "0");
            delete_spawn(ctx, Some(npc));
        }
    } else if npc_id == d.mob[1] {
        if let Some(st) = find_right_state(ctx, npc) {
            ctx.cancel_quest_timer("Archon Hellisha has despawned", Some(npc), Some(st.player()));
            st.set(ctx, "spawned", "0");
            delete_spawn(ctx, Some(npc));
        }
    } else if npc_id == d.mob[2] {
        if let Some(st) = find_right_state(ctx, npc) {
            st.set_cond(ctx, 18, true);
            say(ctx, d, Some(npc), 13, player);
            st.set(ctx, "Quest0", "0");
            // NOTE(java-quirk): the timer runs on the NPC `npc[4]`, not on this monster, so
            // this cancels nothing.
            ctx.cancel_quest_timer("Mob_2 has despawned", Some(npc), Some(player));
            delete_spawn(ctx, Some(npc));
        }
    }
}

/// Java `onSkillSee(npc, player, skill, targets, isSummon)`.
pub fn on_skill_see(
    d: &SagaData,
    ctx: &mut ScriptCtx<'_>,
    npc: ScriptNpc,
    player: ObjectId,
    targets: &[ObjectId],
) {
    let spawned_for = ctx.shared_state::<SpawnList>().0.get(&npc.object_id).copied();
    let Some(quest_player) = spawned_for.filter(|p| *p != player) else {
        return;
    };
    if ctx.world().player(quest_player).is_none() {
        return;
    }
    for &obj in targets {
        if obj == quest_player || obj == npc.object_id {
            let Some(st2) = find_right_state(ctx, npc) else {
                return;
            };
            say(ctx, d, Some(npc), 5, player);
            ctx.cancel_quest_timer("Archon Hellisha has despawned", Some(npc), Some(st2.player()));
            st2.set(ctx, "spawned", "0");
            delete_spawn(ctx, Some(npc));
        }
    }
}

/// Java `onTalk(npc, player)`.
pub fn on_talk(d: &SagaData, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
    let st = ctx.get_quest_state(player, true)?;
    let mut htmltext = ctx.get_no_quest_msg(player);
    let page = |name: &str| name.to_owned();
    let npc_id = npc.id;
    if npc_id == d.npc[0] && st.is_completed(ctx) {
        htmltext = ctx.get_already_completed_msg(player);
    } else if ctx.player_class_id(player) == get_prev_class(d, ctx, player) {
        match st.get_cond(ctx) {
            0 => {
                if npc_id == d.npc[0] {
                    htmltext = page("0-01.htm");
                }
            }
            1 => {
                if npc_id == d.npc[0] {
                    htmltext = page("0-04.htm");
                } else if npc_id == d.npc[2] {
                    htmltext = page("2-01.htm");
                }
            }
            2 => {
                if npc_id == d.npc[2] {
                    htmltext = page("2-02.htm");
                } else if npc_id == d.npc[1] {
                    htmltext = page("1-01.htm");
                }
            }
            3 => {
                if npc_id == d.npc[1] && ctx.has_quest_items(player, d.items[0]) {
                    if d.items[11] == 0 || ctx.has_quest_items(player, d.items[11]) {
                        htmltext = page("1-03.htm");
                    } else {
                        htmltext = page("1-02.htm");
                    }
                }
            }
            4 => {
                if npc_id == d.npc[1] {
                    htmltext = page("1-04.htm");
                } else if npc_id == d.npc[2] {
                    htmltext = page("2-03.htm");
                }
            }
            5 => {
                if npc_id == d.npc[2] {
                    htmltext = page("2-04.htm");
                } else if npc_id == d.npc[5] {
                    htmltext = page("5-01.htm");
                }
            }
            6 => {
                if npc_id == d.npc[5] {
                    htmltext = page("5-03.htm");
                } else if npc_id == d.npc[6] {
                    htmltext = page("6-01.htm");
                }
            }
            7 => {
                if npc_id == d.npc[6] {
                    htmltext = page("6-02.htm");
                }
            }
            8 => {
                if npc_id == d.npc[6] {
                    htmltext = page("6-04.htm");
                } else if npc_id == d.npc[7] {
                    htmltext = page("7-01.htm");
                }
            }
            9 => {
                if npc_id == d.npc[7] {
                    htmltext = page("7-05.htm");
                }
            }
            10 => {
                if npc_id == d.npc[7] {
                    htmltext = page("7-07.htm");
                } else if npc_id == d.npc[3] {
                    htmltext = page("3-01.htm");
                }
            }
            11 | 12 => {
                if npc_id == d.npc[3] {
                    if ctx.has_quest_items(player, d.items[2]) {
                        htmltext = page("3-05.htm");
                    } else {
                        htmltext = page("3-04.htm");
                    }
                }
            }
            13 => {
                if npc_id == d.npc[3] {
                    htmltext = page("3-06.htm");
                } else if npc_id == d.npc[8] {
                    htmltext = page("8-01.htm");
                }
            }
            14 => {
                if npc_id == d.npc[8] {
                    htmltext = page("8-03.htm");
                } else if npc_id == d.npc[11] {
                    htmltext = page("11-01.htm");
                }
            }
            15 => {
                if npc_id == d.npc[11] {
                    htmltext = page("11-02.htm");
                } else if npc_id == d.npc[9] {
                    htmltext = page("9-01.htm");
                }
            }
            16 => {
                if npc_id == d.npc[9] {
                    htmltext = page("9-02.htm");
                }
            }
            17 => {
                if npc_id == d.npc[9] {
                    htmltext = page("9-04.htm");
                } else if npc_id == d.npc[10] {
                    htmltext = page("10-01.htm");
                }
            }
            18 => {
                if npc_id == d.npc[10] {
                    htmltext = page("10-05.htm");
                }
            }
            19 => {
                if npc_id == d.npc[10] {
                    htmltext = page("10-07.htm");
                } else if npc_id == d.npc[0] {
                    htmltext = page("0-06.htm");
                }
            }
            20 => {
                if npc_id == d.npc[0] {
                    if ctx.player_level(player) >= 76 {
                        htmltext = page("0-09.htm");
                        // in Kamael quests, npc wants to chat for a bit before changing class
                        let player_class = get_player_class(d, ctx, player);
                        if !(131..=135).contains(&player_class) {
                            st.exit_quest(ctx, false, false);
                            ctx.add_exp_and_sp(player, 2299404, 0);
                            ctx.give_adena(player, 5000000, true);
                            // XXX rewardItems?
                            ctx.give_items(player, 6622, 1);
                            change_class(d, ctx, player);
                            cast(ctx, Some(npc), player, 4339, 1);
                        }
                    } else {
                        htmltext = page("0-010.htm");
                    }
                }
            }
            _ => {}
        }
    }
    Some(htmltext)
}

/// The class change of `0-2` and of cond 20: `setPlayerClass`, the base class when it was the
/// saga's class, `broadcastUserInfo`.
fn change_class(d: &SagaData, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let player_class = get_player_class(d, ctx, player);
    let prev_class = get_prev_class(d, ctx, player);
    ctx.set_player_class(player, player_class);
    if !ctx.is_sub_class_active(player) && ctx.player_base_class(player) == prev_class {
        ctx.set_base_class(player, player_class);
    }
    ctx.broadcast_user_info(player);
}

/// Java `findQuest(player)`: the player's state when the player is of the saga's class.
fn find_quest(d: &SagaData, ctx: &mut ScriptCtx<'_>, player: ObjectId) -> Option<Qs> {
    let st = ctx.get_quest_state(player, false)?;
    let class_id = ctx.player_class_id(player);
    if d.quest_id == 68 {
        for &class in QUEST_CLASSES[1] {
            if class_id == class {
                return Some(st);
            }
        }
    } else if usize::try_from(d.quest_id - 67)
        .ok()
        .and_then(|i| QUEST_CLASSES.get(i))
        .is_some_and(|classes| classes[0] == class_id)
    {
        return Some(st);
    }
    None
}

/// Java `autoChat(npc, text)`: `NpcSay(npc.getObjectId(), ChatType.GENERAL, npc.getId(), text)`
/// to whoever sees the NPC (the template id, not the display id).
fn auto_chat(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, text: &str) {
    let packet = NpcSay {
        object_id: npc.object_id.0,
        text_type: ChatType::General.client_id(),
        npc_id: npc.id,
        npc_string: -1,
        text: text.to_owned(),
        parameters: Vec::new(),
    };
    ctx.world().broadcast_packet(npc.object_id, &packet);
}

/// `autoChat(npc, _text[index].replace("PLAYERNAME", player.getName()))`; nothing without the
/// NPC (a timer always has it).
fn say(ctx: &mut ScriptCtx<'_>, d: &SagaData, npc: Option<ScriptNpc>, index: usize, player: ObjectId) {
    if let Some(npc) = npc {
        let text = text_for(ctx, d, index, player);
        auto_chat(ctx, npc, &text);
    }
}

/// Java `cast(npc, target, skillId, level)`: both animations go out from the target.
fn cast(ctx: &mut ScriptCtx<'_>, npc: Option<ScriptNpc>, target: ObjectId, skill_id: i32, level: i32) {
    ctx.broadcast_magic_skill_use(target, target, target, skill_id, level, 6000, 1);
    if let Some(npc) = npc {
        ctx.broadcast_magic_skill_use(target, npc.object_id, npc.object_id, skill_id, level, 6000, 1);
    }
}

/// Java `addSpawn(st, mob)`: the monster is the player's.
fn add_spawn(ctx: &mut ScriptCtx<'_>, st: Qs, mob: Option<ScriptNpc>) {
    if let Some(mob) = mob {
        ctx.shared_state::<SpawnList>()
            .0
            .insert(mob.object_id, st.player());
    }
}

/// Java `findSpawn(player, npc) != null`.
fn find_spawn(ctx: &mut ScriptCtx<'_>, player: ObjectId, npc: ScriptNpc) -> bool {
    ctx.shared_state::<SpawnList>().0.get(&npc.object_id) == Some(&player)
}

/// Java `deleteSpawn(npc)`: only a monster of the list is deleted.
fn delete_spawn(ctx: &mut ScriptCtx<'_>, npc: Option<ScriptNpc>) {
    let Some(npc) = npc else {
        return;
    };
    if ctx.shared_state::<SpawnList>().0.remove(&npc.object_id).is_some() {
        ctx.delete_npc(npc);
    }
}

/// Java `findRightState(npc)`: the state of this saga of the online player the monster was
/// spawned for.
fn find_right_state(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) -> Option<Qs> {
    let spawned = ctx.shared_state::<SpawnList>().0.get(&npc.object_id).copied()?;
    ctx.world().player(spawned)?;
    ctx.get_quest_state(spawned, false)
}

/// Java `getPlayerClass(player)`: the class the saga makes.
fn get_player_class(d: &SagaData, ctx: &ScriptCtx<'_>, player: ObjectId) -> i32 {
    if ctx.player_class_id(player) == 0x81 {
        // NOTE(java-quirk): only saga 68 has a second class; Java would throw for the others,
        // which `on_talk` never reaches (their `getPrevClass` is -1 for this class).
        return d.class_id.get(1).copied().unwrap_or(-1);
    }
    d.class_id[0]
}

/// Java `getPrevClass(player)`: the class the saga is for.
fn get_prev_class(d: &SagaData, ctx: &ScriptCtx<'_>, player: ObjectId) -> i32 {
    if ctx.player_class_id(player) == 0x81 {
        if d.prev_class.len() == 1 {
            return -1;
        }
        return d.prev_class[1];
    }
    d.prev_class[0]
}

/// Java `giveHalishaMark(st)`: marks for the archon's minions; at 700 the archon comes.
fn give_halisha_mark(d: &SagaData, ctx: &mut ScriptCtx<'_>, st: Qs) {
    if st.get_int(ctx, "spawned") == 0 {
        let player = st.player();
        if ctx.get_quest_items_count(player, d.items[3]) >= 700 {
            ctx.take_items(player, d.items[3], 20);
            let Some((xx, yy, zz)) = ctx.world().position_of(player) else {
                return;
            };
            let archon = ctx.add_spawn(d.mob[1], xx, yy, zz, -1, false, 0, 0);
            add_spawn(ctx, st, archon);
            st.set(ctx, "spawned", "1");
            ctx.start_quest_timer(
                "Archon Hellisha has despawned",
                600000,
                archon,
                Some(player),
                false,
            );
            if let Some(archon) = archon {
                say(ctx, d, Some(archon), 13, player);
                ctx.world_mut()
                    .add_damage_hate(archon.object_id, player, 0, 99999);
                ctx.world_mut().set_intention_attack(archon.object_id, player);
            }
        } else {
            let count = ctx.get_random_range(1, 4);
            ctx.give_items(player, d.items[3], i64::from(count));
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::{player_class, race};
    use l2_world::{Script, ScriptRegistry};

    use super::*;
    use crate::quests::q00070_saga_of_the_phoenix_knight::Q00070SagaOfThePhoenixKnight as Saga;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00070_SagaOfThePhoenixKnight";

    // Q00070's arrays, by Java's index.
    const NPC0: i32 = 30849; // also npc[3]
    const NPC1: i32 = 31624;
    const NPC2: i32 = 31277; // also npc[11]
    const NPC4: i32 = 31631;
    const NPC5: i32 = 31646;
    const NPC6: i32 = 31647;
    const NPC7: i32 = 31650;
    const NPC8: i32 = 31654;
    const NPC9: i32 = 31655;
    const NPC10: i32 = 31657;
    const MOB0: i32 = 27286;
    const MOB1: i32 = 27219;
    const MOB2: i32 = 27278;
    const ICE_CRYSTAL: i32 = 7080; // items[0]
    const DIVINE_STONE: i32 = 7081; // items[2]
    const HALISHA_MARK: i32 = 7485; // items[3]
    const ITEM8: i32 = 7392;
    const ITEM9: i32 = 7423;
    const ITEM10: i32 = 7093;
    const ITEM11: i32 = 6482;

    const NPCS: &str = r#"
        <npc id="30849" type="Folk" level="70" name="Sedrick"/>
        <npc id="31624" type="Folk" level="70" name="Step 1"/>
        <npc id="31277" type="Folk" level="70" name="Step 2"/>
        <npc id="31631" type="Folk" level="70" name="Spawned helper"/>
        <npc id="31646" type="Folk" level="70" name="Step 5"/>
        <npc id="31647" type="Folk" level="70" name="Step 6"/>
        <npc id="31650" type="Folk" level="70" name="Step 7"/>
        <npc id="31654" type="Folk" level="70" name="Step 8"/>
        <npc id="31655" type="Folk" level="70" name="Step 9"/>
        <npc id="31657" type="Folk" level="70" name="Step 10"/>
        <npc id="27286" type="Monster" level="76" name="Mob 0"/>
        <npc id="27219" type="Monster" level="76" name="Archon of Halisha"/>
        <npc id="27278" type="Monster" level="76" name="Mob 2"/>
        <npc id="27214" type="Monster" level="76" name="Guardian Angel"/>
        <npc id="21646" type="Monster" level="76" name="Archon Minion"/>"#;

    fn items() -> String {
        let mut xml = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
            <item id="6622" type="EtcItem" name="Giant's Codex"><set name="is_stackable" val="true"/></item>
            <item id="7080" type="EtcItem" name="Ice Crystal"/>
            <item id="7081" type="EtcItem" name="Divine Stone of Wisdom"/>
            <item id="7485" type="EtcItem" name="Halisha's Mark"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#,
        );
        for id in [7534, 7268, 7299, 7330, 7361, 7392, 7423, 7093, 6482] {
            xml.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Saga item {id}"><set name="is_questitem" val="true"/></item>"#
            ));
        }
        xml
    }

    /// The stand-ins of Q00070's pages with the links of the real ones.
    fn pages() -> Pages {
        let mut pages = Pages::of(&Saga)
            .page("0-01.htm", &["0-1"])
            .page("0-05.htm", &["accept"])
            .page("0-06.htm", &["0-2"])
            .page("1-01.htm", &["1-3"])
            .page("1-03.htm", &["1-4"])
            .page("10-01.htm", &["10-1"])
            .page("10-05.htm", &["10-2"])
            .page("11-01.htm", &["11-9"])
            .page("2-01.htm", &["2-1"])
            .page("2-03.htm", &["2-2"])
            .page("3-01.htm", &["3-5"])
            .page("3-05.htm", &["3-8"])
            .page("3-07.htm", &["3-6", "3-7"])
            .page("4-010.htm", &["4-3"])
            .page("4-06.htm", &["4-1"])
            .page("4-07.htm", &["4-2"])
            .page("5-01.htm", &["5-1"])
            .page("6-02.htm", &["6-1"])
            .page("7-01.htm", &["7-1"])
            .page("7-05.htm", &["7-2"])
            .page("8-01.htm", &["8-1"])
            .page("9-02.htm", &["9-1"]);
        for file in [
            "0-010.htm",
            "0-02.htm",
            "0-03.htm",
            "0-04.htm",
            "0-07.htm",
            "0-08.htm",
            "0-09.htm",
            "1-02.htm",
            "1-04.htm",
            "1-05.htm",
            "1-06.htm",
            "10-02.htm",
            "10-03.htm",
            "10-04.htm",
            "10-06.htm",
            "10-07.htm",
            "11-02.htm",
            "11-03.htm",
            "2-02.htm",
            "2-04.htm",
            "2-05.htm",
            "2-06.htm",
            "3-02.htm",
            "3-03.htm",
            "3-04.htm",
            "3-06.htm",
            "3-08.htm",
            "4-01.htm",
            "4-011.htm",
            "4-02.htm",
            "4-03.htm",
            "4-04.htm",
            "4-05.htm",
            "4-08.htm",
            "4-09.htm",
            "5-02.htm",
            "5-03.htm",
            "6-01.htm",
            "6-03.htm",
            "6-04.htm",
            "7-02.htm",
            "7-03.htm",
            "7-04.htm",
            "7-06.htm",
            "7-07.htm",
            "8-02.htm",
            "8-03.htm",
            "9-01.htm",
            "9-03.htm",
            "9-04.htm",
        ] {
            pages = pages.page(file, &[]);
        }
        pages.npcs(&[NPC0, NPC1, NPC2, NPC4, NPC5, NPC6, NPC7, NPC8, NPC9, NPC10])
    }

    struct Npcs {
        n0: Oid,
        n1: Oid,
        n2: Oid,
        n5: Oid,
        n6: Oid,
        n7: Oid,
        n8: Oid,
        n9: Oid,
        n10: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Saga);
        let mut t = ScriptTest::with_pages(registry, NPCS, &items(), pages());
        let npcs = Npcs {
            n0: t.spawn(NPC0, 20),
            n1: t.spawn(NPC1, 30),
            n2: t.spawn(NPC2, 40),
            n5: t.spawn(NPC5, 50),
            n6: t.spawn(NPC6, 60),
            n7: t.spawn(NPC7, 70),
            n8: t.spawn(NPC8, 80),
            n9: t.spawn(NPC9, 90),
            n10: t.spawn(NPC10, 100),
        };
        (t, npcs)
    }

    /// A level 76 Paladin with the Ice Crystal, the step 4 item, the Divine Stone and 700
    /// Halisha marks (what other quests and hunts give).
    fn enter_paladin(t: &mut ScriptTest) {
        t.enter_as(
            1,
            76,
            race::HUMAN,
            player_class::PALADIN,
            vec![
                item(0x6000_0100, ICE_CRYSTAL, 1),
                item(0x6000_0101, ITEM11, 1),
                item(0x6000_0102, DIVINE_STONE, 1),
                item(0x6000_0103, HALISHA_MARK, 700),
            ],
        );
    }

    fn quest_int(t: &mut ScriptTest, var: &str) -> i32 {
        let mut value = -1;
        t.world.run_as_script(NAME, |ctx| {
            value = ctx
                .get_quest_state(Oid(1), false)
                .map_or(-1, |qs| qs.get_int(ctx, var));
        });
        value
    }

    fn step(t: &mut ScriptTest, npc: Oid, page: &str, event: &str, next: &str) {
        let got = t.quest_page(1, npc);
        assert!(got.contains(page), "{page}: {got}");
        let got = t.event(1, NAME, event);
        assert!(got.contains(next), "{event} -> {next}: {got}");
    }

    fn script_npc(t: &ScriptTest, id: i32) -> ScriptNpc {
        let object_id = t.npcs_of(id)[0];
        ScriptNpc { object_id, id }
    }

    #[test]
    fn the_saga_from_paladin_to_phoenix_knight() {
        let (mut t, n) = setup();
        enter_paladin(&mut t);

        // Start: 0-01 -> "0-1" (level 76) -> 0-05 -> "accept".
        step(&mut t, n.n0, "0-01.htm", "0-1", "0-05.htm");
        assert!(t.event(1, NAME, "accept").contains("0-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(t.count(1, ITEM10), 1);
        assert!(t.quest_page(1, n.n0).contains("0-04.htm"));

        // Steps 1 to 5: talks and events.
        step(&mut t, n.n2, "2-01.htm", "2-1", "2-05.htm");
        step(&mut t, n.n1, "1-01.htm", "1-3", "1-05.htm");
        step(&mut t, n.n1, "1-03.htm", "1-4", "1-06.htm");
        assert_eq!(
            (t.count(1, ICE_CRYSTAL), t.count(1, ITEM11), t.count(1, 7534)),
            (0, 0, 1)
        );
        step(&mut t, n.n2, "2-03.htm", "2-2", "2-06.htm");
        step(&mut t, n.n5, "5-01.htm", "5-1", "5-02.htm");
        assert_eq!(quest_int(&mut t, "cond"), 6);
        assert_eq!(
            t.take_writes(),
            ["2", "3", "4", "5", "6"].map(|c| rows::update(NAME, "cond", c))
        );

        // Step 6: ten guardian angels; nine count in "kills", the tenth gives the item.
        for _ in 0..10 {
            let angel = t.spawn(27214, 120);
            t.kill(1, angel);
        }
        assert_eq!(t.count(1, 7299), 1);
        let writes = t.take_writes();
        assert_eq!(writes.len(), 10);
        assert_eq!(writes[0], rows::insert(NAME, "kills", "1"));
        assert_eq!(writes[8], rows::update(NAME, "kills", "9"));
        assert_eq!(writes[9], rows::update(NAME, "cond", "7"));

        // Steps 7 and 8: mob 0 is spawned for the player at its place (far away here) and gives
        // the item to the player who kills it alone.
        step(&mut t, n.n6, "6-02.htm", "6-1", "6-03.htm");
        step(&mut t, n.n7, "7-01.htm", "7-1", "7-02.htm");
        let mob0 = script_npc(&t, MOB0);
        assert_eq!(t.world.position_of(mob0.object_id).map(|p| p.0), Some(191046));
        assert_eq!(quest_int(&mut t, "spawned"), 1);
        // A second "7-1" while it is out: 7-03.
        assert!(t.quest_page(1, n.n7).contains("7-01.htm"));
        assert!(t.event(1, NAME, "7-1").contains("7-03.htm"));
        t.kill(1, mob0.object_id);
        assert!(!t.in_world(mob0.object_id));
        assert_eq!(t.count(1, 7330), 1);
        assert_eq!((quest_int(&mut t, "cond"), quest_int(&mut t, "spawned")), (9, 0));
        step(&mut t, n.n7, "7-05.htm", "7-2", "7-06.htm");

        // Steps 10 to 14: npc[3] is npc[0]'s NPC, npc[11] is npc[2]'s.
        step(&mut t, n.n0, "3-01.htm", "3-5", "3-07.htm");
        assert!(t.event(1, NAME, "3-6").contains("3-02.htm"));
        step(&mut t, n.n0, "3-05.htm", "3-8", "3-08.htm");
        assert_eq!((t.count(1, DIVINE_STONE), t.count(1, 7361)), (0, 1));
        step(&mut t, n.n8, "8-01.htm", "8-1", "8-02.htm");
        step(&mut t, n.n2, "11-01.htm", "11-9", "11-03.htm");
        assert_eq!(quest_int(&mut t, "cond"), 15);

        // Step 15: an archon minion with 700 marks calls the Archon of Halisha, who attacks.
        t.take_writes();
        let minion = t.spawn(21646, 120);
        t.kill(1, minion);
        assert_eq!(t.count(1, HALISHA_MARK), 680);
        let archon = script_npc(&t, MOB1);
        assert!(
            t.world
                .aggro_list(archon.object_id)
                .iter()
                .any(|a| a.attacker == Oid(1))
        );
        assert!(t.take_writes().contains(&rows::update(NAME, "spawned", "1")));
        t.kill(1, archon.object_id);
        assert_eq!((t.count(1, HALISHA_MARK), t.count(1, ITEM8)), (0, 1));
        assert_eq!(quest_int(&mut t, "cond"), 16);

        // Steps 16 and 17: mob 2 and the helper npc[4] come; sixteen hits on mob 2 make it
        // flee ("Tab" 1), then the helper's pages lead to "4-3" and the item.
        step(&mut t, n.n9, "9-02.htm", "9-1", "9-03.htm");
        step(&mut t, n.n10, "10-01.htm", "10-1", "10-02.htm");
        let (mob2, helper) = (script_npc(&t, MOB2), script_npc(&t, NPC4));
        let first_talk = |t: &mut ScriptTest| {
            let mut page = None;
            t.world
                .run_as_script(NAME, |ctx| page = Saga.on_first_talk(ctx, helper, Oid(1)));
            page.unwrap_or_default()
        };
        assert_eq!(first_talk(&mut t), "4-03.htm");
        for _ in 0..16 {
            t.world
                .run_as_script(NAME, |ctx| Saga.on_attack(ctx, mob2, Oid(1), 10, false, None));
        }
        assert!(!t.in_world(mob2.object_id));
        assert_eq!(quest_int(&mut t, "Tab"), 1);
        assert_eq!(first_talk(&mut t), "4-06.htm");
        t.world.run_as_script(NAME, |ctx| {
            let page = Saga.on_event(ctx, "4-1", Some(helper), Some(Oid(1)));
            assert_eq!(page.as_deref(), Some("4-010.htm"));
            assert_eq!(Saga.on_event(ctx, "4-3", Some(helper), Some(Oid(1))), None);
        });
        assert!(!t.in_world(helper.object_id));
        assert_eq!(t.count(1, ITEM9), 1);
        assert_eq!(quest_int(&mut t, "cond"), 18);

        // Steps 18 and 19, then the class change at npc[0].
        step(&mut t, n.n10, "10-05.htm", "10-2", "10-06.htm");
        t.take_writes();
        step(&mut t, n.n0, "0-06.htm", "0-2", "0-07.htm");
        let p = t.world.player(Oid(1)).unwrap();
        assert_eq!(
            (p.data.class_id, p.data.base_class),
            (player_class::PHOENIX_KNIGHT, player_class::PHOENIX_KNIGHT)
        );
        assert_eq!(
            (t.count(1, 57), t.count(1, 6622), t.count(1, ITEM10)),
            (5_000_000, 1, 0)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, n.n0)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn another_class_gets_the_no_quest_page() {
        let (mut t, n) = setup();
        t.enter_as(1, 76, race::HUMAN, player_class::DARK_AVENGER, vec![]);
        let page = t.quest_page(1, n.n0);
        assert!(!page.contains("0-01.htm"), "{page}");
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn under_76_the_start_is_refused() {
        let (mut t, n) = setup();
        t.enter_as(1, 75, race::HUMAN, player_class::PALADIN, vec![]);
        step(&mut t, n.n0, "0-01.htm", "0-1", "0-02.htm");
        // The state stays CREATED: `exitQuest(true)` of an unstarted state writes nothing.
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_step_17_monsters_leave_after_a_minute() {
        let (mut t, n) = setup();
        enter_paladin(&mut t);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
            qs.set_cond(ctx, 17, false);
        });
        step(&mut t, n.n10, "10-01.htm", "10-1", "10-02.htm");
        let (mob2, helper) = (script_npc(&t, MOB2), script_npc(&t, NPC4));
        // While they are out: 10-03 (`Quest1` 45 until the helper's first line, then 10-04).
        assert!(t.quest_page(1, n.n10).contains("10-01.htm"));
        assert!(t.event(1, NAME, "10-1").contains("10-03.htm"));
        t.wait_ms(600);
        assert_eq!(quest_int(&mut t, "Quest1"), 0);
        // "Mob_3 has despawned" at 59 s (Quest0 2), "Mob_2 has despawned" at 60 s: both gone,
        // Quest0 back to 0 so the step can be done again.
        t.wait_ms(60_000);
        assert!(!t.in_world(mob2.object_id));
        assert!(!t.in_world(helper.object_id));
        assert_eq!(quest_int(&mut t, "Quest0"), 0);
        assert!(t.npcs_of(MOB2).is_empty() && t.npcs_of(NPC4).is_empty());
        let _ = Saga.name();
    }
}
