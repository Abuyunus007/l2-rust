//! Disciple's Necropolis, Past (instance 112), the Seven Signs quest 196 (Seal of the Emperor).
// Java: dist/game/data/scripts/instances/SSQDisciplesNecropolisPast/SSQDisciplesNecropolisPast.java

use l2_protocol_game::server::{ChatType, NpcSay};
use l2_protocol_game::{NpcStringId, SystemMessageId};
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};

// NPCs
const SEAL_DEVICE: i32 = 27384;
const PROMISE_OF_MAMMON: i32 = 32585;
const SHUNAIMAN: i32 = 32586;
const LEON: i32 = 32587;
const DISCIPLES_GATEKEEPER: i32 = 32657;
const LILITH: i32 = 32715;
const LILITHS_STEWARD: i32 = 32716;
const LILITHS_ELITE: i32 = 32717;
const ANAKIM: i32 = 32718;
const ANAKIMS_GUARDIAN: i32 = 32719;
const ANAKIMS_GUARD: i32 = 32720;
const ANAKIMS_EXECUTOR: i32 = 32721;
const LILIM_BUTCHER: i32 = 27371;
const LILIM_MAGUS: i32 = 27372;
const LILIM_KNIGHT_ERRANT: i32 = 27373;
const SHILENS_EVIL_THOUGHTS1: i32 = 27374;
const SHILENS_EVIL_THOUGHTS2: i32 = 27375;
const LILIM_KNIGHT: i32 = 27376;
const LILIM_SLAYER: i32 = 27377;
const LILIM_GREAT_MAGUS: i32 = 27378;
const LILIM_GUARD_KNIGHT: i32 = 27379;

// Items
const SACRED_SWORD_OF_EINHASAD: i32 = 15310;
const SEAL_OF_BINDING: i32 = 13846;

// Skills
const SEAL_ISOLATION: SkillKey = SkillKey { id: 5980, level: 3 };

/// `SKILLS.get(npcId)`: the "Presentation" skill of the fighting NPCs.
fn presentation_skill(npc_id: i32) -> Option<SkillKey> {
    let id = match npc_id {
        32715 => 6187, // Presentation - Lilith Battle
        32716 => 6188, // Presentation - Lilith's Steward Battle1
        32717 => 6190, // Presentation - Lilith's Bodyguards Battle1
        32718 => 6191, // Presentation - Anakim Battle
        32719 => 6192, // Presentation - Anakim's Guardian Battle1
        32720 => 6194, // Presentation - Anakim's Guard Battle
        32721 => 6195, // Presentation - Anakim's Executor Battle
        _ => return None,
    };
    Some(SkillKey { id, level: 1 })
}

// Locations
const ENTER: (i32, i32, i32) = (-89554, 216078, -7488);
const EXIT: (i32, i32, i32) = (171895, -17501, -4903);

// NpcStringId
const LILITH_SHOUT: [NpcStringId; 3] = [
    NpcStringId::HowDareYouTryToContendAgainstMeInStrengthRidiculous,
    NpcStringId::AnakimInTheNameOfGreatShilienIWillCutYourThroat,
    NpcStringId::YouCannotBeTheMatchOfLilithILlTeachYouALesson,
];

// Misc
const TEMPLATE_ID: i32 = 112;
const DOOR_1: i32 = 17240102;
const DOOR_2: i32 = 17240104;
const DOOR_3: i32 = 17240106;
const DOOR_4: i32 = 17240108;
const DOOR_5: i32 = 17240110;
const DISCIPLES_NECROPOLIS_DOOR: i32 = 17240111;

/// `LILITH_SPAWN`: `(npc, x, y, z, heading)` in the order Java's `HashMap` walks it (the
/// bucket of an Integer key is its low bits: 32715, 32716, 32717).
const LILITH_SPAWN: [(i32, i32, i32, i32, i32); 3] = [
    (LILITH, -83175, 217021, -7504, 49151),
    (LILITHS_STEWARD, -83327, 216938, -7492, 50768),
    (LILITHS_ELITE, -83003, 216909, -7492, 4827),
];
/// `ANAKIM_SPAWN` in the order Java's `HashMap` walks it: buckets 0, 1, 14, 15 for 32720,
/// 32721, 32718, 32719.
const ANAKIM_SPAWN: [(i32, i32, i32, i32, i32); 4] = [
    (ANAKIMS_GUARD, -83086, 216519, -7495, 15910),
    (ANAKIMS_EXECUTOR, -83031, 216604, -7492, 17071),
    (ANAKIM, -83179, 216479, -7504, 16384),
    (ANAKIMS_GUARDIAN, -83321, 216507, -7492, 16166),
];

// Movie ids (`Movie.SSQ_SEALING_EMPEROR_1ST`, `_2ND`).
const SSQ_SEALING_EMPEROR_1ST: i32 = 12;
const SSQ_SEALING_EMPEROR_2ND: i32 = 13;

const Q00196: &str = "Q00196_SevenSignsSealOfTheEmperor";

/// Java `instances.SSQDisciplesNecropolisPast.SSQDisciplesNecropolisPast`.
pub struct SSQDisciplesNecropolisPast;

impl Script for SSQDisciplesNecropolisPast {
    fn name(&self) -> &'static str {
        "SSQDisciplesNecropolisPast"
    }

    fn path(&self) -> &'static str {
        "instances/SSQDisciplesNecropolisPast"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[SEAL_DEVICE]);
        s.add_first_talk_id(&[SHUNAIMAN, LEON, DISCIPLES_GATEKEEPER]);
        s.add_kill_id(&LILIMS);
        s.add_aggro_range_enter_id(&LILIMS);
        s.add_spawn_id(&[SEAL_DEVICE]);
        s.add_start_npc(&[PROMISE_OF_MAMMON]);
        s.add_talk_id(&[PROMISE_OF_MAMMON, SHUNAIMAN, LEON, DISCIPLES_GATEKEEPER]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // `getPlayerWorld(player)` of a null player is a NullPointerException.
        let player = player?;
        let instance_id = ctx.world().player_instance_world(player).map(|w| w.instance_id)?;
        match event {
            "FINISH" => {
                if ctx.get_quest_items_count(player, SEAL_OF_BINDING) >= 4 {
                    ctx.world_mut().abort_attack(player);
                    ctx.play_movie(player, SSQ_SEALING_EMPEROR_2ND);
                    ctx.start_quest_timer("TELEPORT", 27000, None, Some(player), false);
                }
            }
            "TELEPORT" => {
                // `teleToLocation(ENTER, player.getInstanceId(), 0)`.
                let instance = ctx.world().instance_id_of(player);
                ctx.world_mut().tele_to_location(
                    player,
                    TeleportTarget::at(ENTER.0, ENTER.1, ENTER.2).with_instance(instance),
                );
            }
            "FIGHT" => fight(ctx, instance_id, player),
            _ => {}
        }
        // `super.onEvent(event, npc, player)`.
        None
    }

    fn on_aggro_range_enter(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _player: ObjectId,
        _is_summon: bool,
    ) {
        let text = match npc.id {
            LILIM_BUTCHER | LILIM_GUARD_KNIGHT => NpcStringId::ThisPlaceOnceBelongedToLordShilen,
            LILIM_MAGUS | LILIM_GREAT_MAGUS => NpcStringId::WhoDaresEnterThisPlace,
            LILIM_KNIGHT_ERRANT | LILIM_KNIGHT => {
                NpcStringId::ThoseWhoAreAfraidShouldGetAwayAndThoseWhoAreBraveShouldFight
            }
            LILIM_SLAYER => NpcStringId::LeaveNow,
            _ => return,
        };
        if ctx.is_script_value(npc, 0) {
            say(ctx, npc, ChatType::NpcGeneral, text);
            ctx.set_script_value(npc, 1);
        }
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if ctx.world().player_instance_world(player).is_none() {
            return;
        }
        if ctx.is_script_value(npc, 0) && ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * 0.1 {
            ctx.give_items(player, SEAL_OF_BINDING, 1);
            ctx.send_system_message(
                player,
                SystemMessageId::TheSealingDeviceGlittersAndMovesActivationCompleteNormally,
            );
            ctx.set_script_value(npc, 1);
            ctx.start_quest_timer("FINISH", 1000, Some(npc), Some(player), false);
            // NOTE(java-quirk): the FIGHT timer was started without an NPC, so this cancel (with
            // the seal device) finds nothing and the fight goes on until the player is not allowed
            // in any instance.
            ctx.cancel_quest_timer("FIGHT", Some(npc), Some(player));
        }

        if ctx.get_random(100) < 50 {
            ctx.world_mut().do_cast(npc.object_id, SEAL_ISOLATION);
        }
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.htm", npc.id))
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if let Some(instance_id) = ctx.world().player_instance_world(killer).map(|w| w.instance_id) {
            check_doors(ctx, instance_id);
        }

        let text = match npc.id {
            LILIM_MAGUS | LILIM_GREAT_MAGUS => NpcStringId::LordShilenSomeDayYouWillAccomplishThisMission,
            LILIM_KNIGHT_ERRANT | LILIM_KNIGHT | LILIM_GUARD_KNIGHT => NpcStringId::WhyAreYouGettingInOurWay,
            LILIM_SLAYER => NpcStringId::ForShilen,
            _ => return,
        };
        say(ctx, npc, ChatType::NpcGeneral, text);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.world_mut().set_mortal(npc.object_id, false);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let mut htmltext = ctx.get_no_quest_msg(talker);
        let (exists, cond3_or_4, cond_ge_3) = match ctx.player_quest_state(talker, Q00196) {
            Some(qs) => (true, qs.is_cond(3) || qs.is_cond(4), qs.cond() >= 3),
            None => (false, false, false),
        };
        if !exists {
            return Some(htmltext);
        }

        match npc.id {
            PROMISE_OF_MAMMON => {
                if cond3_or_4 {
                    enter_instance(ctx, talker);
                    return Some(String::new());
                }
            }
            LEON => {
                if cond_ge_3 {
                    ctx.take_items(talker, SACRED_SWORD_OF_EINHASAD, -1);
                    // `world.removeAllowed(talker)` of a null world is a NullPointerException.
                    let instance_id = ctx.world().player_instance_world(talker).map(|w| w.instance_id)?;
                    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                        world.remove_allowed(talker);
                    }
                    // `teleToLocation(EXIT, 0)`: the Location carries instance 0.
                    ctx.world_mut().tele_to_location(
                        talker,
                        TeleportTarget::at(EXIT.0, EXIT.1, EXIT.2).with_instance(0),
                    );
                    htmltext = "32587-01.html".to_owned();
                }
            }
            DISCIPLES_GATEKEEPER => {
                if cond_ge_3 {
                    let instance_id = ctx.world().instance_id_of(npc.object_id);
                    if ctx.world().instance_world(instance_id).is_some() {
                        ctx.world_mut()
                            .instance_open_door(instance_id, DISCIPLES_NECROPOLIS_DOOR);
                        ctx.play_movie(talker, SSQ_SEALING_EMPEROR_1ST);
                        ctx.start_quest_timer("FIGHT", 1000, None, Some(talker), false);
                    }
                }
                // NOTE(java-quirk): the gatekeeper answers with the "no quest" page even when
                // the fight starts; `htmltext` is not changed in this branch.
            }
            _ => {}
        }
        Some(htmltext)
    }
}

const LILIMS: [i32; 9] = [
    LILIM_BUTCHER,
    LILIM_MAGUS,
    LILIM_KNIGHT_ERRANT,
    LILIM_KNIGHT,
    SHILENS_EVIL_THOUGHTS1,
    SHILENS_EVIL_THOUGHTS2,
    LILIM_SLAYER,
    LILIM_GREAT_MAGUS,
    LILIM_GUARD_KNIGHT,
];

/// `new NpcSay(npc.getObjectId(), chatType, npc.getId(), string)` broadcast by the NPC.
fn say(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, chat: ChatType, text: NpcStringId) {
    ctx.world()
        .broadcast_packet(npc.object_id, &npc_say(npc, chat, text, Vec::new()));
}

fn npc_say(npc: ScriptNpc, chat: ChatType, text: NpcStringId, parameters: Vec<String>) -> NpcSay {
    NpcSay {
        object_id: npc.object_id.0,
        text_type: chat.client_id(),
        npc_id: npc.id,
        npc_string: text.id(),
        text: String::new(),
        parameters,
    }
}

/// `player.sendPacket(new NpcSay(.. WHISPER ..))` when the NPC is within 2000 of the player.
fn whisper(
    ctx: &mut ScriptCtx<'_>,
    caster: ScriptNpc,
    player: ObjectId,
    text: NpcStringId,
    parameters: Vec<String>,
) {
    if ctx.check_if_in_range(2000, caster.object_id, player, true) {
        ctx.world()
            .send(player, &npc_say(caster, ChatType::Whisper, text, parameters));
    }
}

/// Java `checkDoors(npc, world)`: the doors open at the 4th, 10th, 18th, 28th and 40th kill.
fn check_doors(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let count_kill = param_int(ctx, instance_id, "countKill") + 1;
    set_param(ctx, instance_id, "countKill", &count_kill.to_string());
    let door = match count_kill {
        4 => DOOR_1,
        10 => DOOR_2,
        18 => DOOR_3,
        28 => DOOR_4,
        40 => DOOR_5,
        _ => return,
    };
    ctx.world_mut().instance_open_door(instance_id, door);
}

/// Java `onEvent` "FIGHT": the Anakim group and the Lilith group cast at each other.
fn fight(ctx: &mut ScriptCtx<'_>, instance_id: i32, player: ObjectId) {
    let anakim_group = group(ctx, instance_id, "anakimGroup");
    let lilith_group = group(ctx, instance_id, "lilithGroup");
    for caster in &anakim_group {
        let caster = *caster;
        if !ctx.world().is_casting_now(caster.object_id) {
            make_cast(ctx, caster, &lilith_group);
        }

        if caster.id == ANAKIM {
            if ctx.is_script_value(caster, 0) {
                say(
                    ctx,
                    caster,
                    ChatType::NpcShout,
                    NpcStringId::YouSuchAFoolTheVictoryOverThisWarBelongsToShilien,
                );
                ctx.set_script_value(caster, 1);
            } else if ctx.get_random(100) < 10 {
                if let Some(text) = ctx.get_random_entry(&LILITH_SHOUT) {
                    say(ctx, caster, ChatType::NpcShout, text);
                }
            }
        }
    }

    for caster in &lilith_group {
        let caster = *caster;
        if !ctx.world().is_casting_now(caster.object_id) {
            make_cast(ctx, caster, &anakim_group);
        }

        if caster.id == LILITH {
            if ctx.is_script_value(caster, 0) {
                say(
                    ctx,
                    caster,
                    ChatType::NpcShout,
                    NpcStringId::ForTheEternityOfEinhasad,
                );
                whisper(
                    ctx,
                    caster,
                    player,
                    NpcStringId::MyPowerSWeakeningHurryAndTurnOnTheSealingDevice,
                    Vec::new(),
                );
                ctx.set_script_value(caster, 1);
            } else if ctx.get_random(100) < 10 {
                match ctx.get_random(3) {
                    0 => {
                        say(
                            ctx,
                            caster,
                            ChatType::NpcShout,
                            NpcStringId::DearShillienSOffspringsYouAreNotCapableOfConfrontingUs,
                        );
                        whisper(
                            ctx,
                            caster,
                            player,
                            NpcStringId::All4SealingDevicesMustBeTurnedOn,
                            Vec::new(),
                        );
                    }
                    1 => {
                        say(
                            ctx,
                            caster,
                            ChatType::NpcShout,
                            NpcStringId::ILlShowYouTheRealPowerOfEinhasad,
                        );
                        whisper(
                            ctx,
                            caster,
                            player,
                            NpcStringId::LilithSAttackIsGettingStrongerGoAheadAndTurnItOn,
                            Vec::new(),
                        );
                    }
                    _ => {
                        say(
                            ctx,
                            caster,
                            ChatType::NpcShout,
                            NpcStringId::DearMilitaryForceOfLightGoDestroyTheOffspringsOfShillien,
                        );
                        let name = ctx
                            .world()
                            .player(player)
                            .map(|p| p.data.name.clone())
                            .unwrap_or_default();
                        whisper(
                            ctx,
                            caster,
                            player,
                            NpcStringId::DearS1GiveMeMoreStrength,
                            vec![name],
                        );
                    }
                }
            }
        }
    }

    ctx.start_quest_timer("FIGHT", 1000, None, Some(player), false);
}

/// Java `makeCast(npc, targets)`.
fn make_cast(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, targets: &[ScriptNpc]) {
    let target = ctx.get_random_entry(targets);
    ctx.world_mut()
        .set_npc_target(npc.object_id, target.map(|t| t.object_id));
    if let Some(skill) = presentation_skill(npc.id) {
        ctx.world_mut().do_cast(npc.object_id, skill);
    }
}

/// `world.getParameters().getList(name, Npc.class, new ArrayList<>())`: `objectId:npcId` pairs.
fn group(ctx: &ScriptCtx<'_>, instance_id: i32, name: &str) -> Vec<ScriptNpc> {
    ctx.world()
        .instance_world(instance_id)
        .and_then(|w| w.parameters.get(name))
        .map(|list| decode_group(list))
        .unwrap_or_default()
}

fn decode_group(list: &str) -> Vec<ScriptNpc> {
    list.split(',')
        .filter_map(|pair| {
            let (object_id, id) = pair.split_once(':')?;
            Some(ScriptNpc {
                object_id: ObjectId(object_id.parse().ok()?),
                id: id.parse().ok()?,
            })
        })
        .collect()
}

fn encode_group(npcs: &[ScriptNpc]) -> String {
    let pairs: Vec<String> = npcs
        .iter()
        .map(|n| format!("{}:{}", n.object_id.0, n.id))
        .collect();
    pairs.join(",")
}

/// `world.getParameters().getInt(name, 0)`.
fn param_int(ctx: &ScriptCtx<'_>, instance_id: i32, name: &str) -> i32 {
    ctx.world()
        .instance_world(instance_id)
        .and_then(|w| w.parameters.get(name))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

fn set_param(ctx: &mut ScriptCtx<'_>, instance_id: i32, name: &str, value: &str) {
    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
        world.parameters.insert(name.to_owned(), value.to_owned());
    }
}

/// Java `enterInstance(talker, TEMPLATE_ID)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let offset = ctx.world().max_offset_on_teleport();
    ctx.world_mut().enter_instance(
        player,
        TEMPLATE_ID,
        |_, _| true,
        |world, player, instance_id, first_entrance| {
            if first_entrance {
                // `spawnNPC(world)`.
                for (name, spawns) in [
                    ("lilithGroup", &LILITH_SPAWN[..]),
                    ("anakimGroup", &ANAKIM_SPAWN[..]),
                ] {
                    let mut group = Vec::new();
                    for (npc_id, x, y, z, heading) in spawns {
                        if let Some(object_id) =
                            world.script_add_spawn(*npc_id, (*x, *y, *z), *heading, false, 0, instance_id)
                        {
                            group.push(ScriptNpc {
                                object_id,
                                id: *npc_id,
                            });
                        }
                    }
                    if let Some(instance_world) = world.instance_world_mut(instance_id) {
                        instance_world
                            .parameters
                            .insert(name.to_owned(), encode_group(&group));
                    }
                }
                if let Some(instance_world) = world.instance_world_mut(instance_id) {
                    instance_world.add_allowed(player);
                }
            }
            // `teleportPlayer(player, ENTER, id)`: with the random offset.
            world.tele_to_location(
                player,
                TeleportTarget::at(ENTER.0, ENTER.1, ENTER.2)
                    .with_instance(instance_id)
                    .with_random_offset(offset),
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use l2_data::DoorTemplates;
    use l2_util::ObjectId as Oid;
    use l2_world::script::STARTED;
    use l2_world::{ClientInput, IdFactory, ScriptRegistry};

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="32585" type="Folk" level="80" name="Promise of Mammon"/>
        <npc id="32586" type="Folk" level="80" name="Shunaiman"/>
        <npc id="32587" type="Folk" level="80" name="Leon"/>
        <npc id="32657" type="Folk" level="80" name="Disciple's Gatekeeper"/>
        <npc id="27384" type="Monster" level="80" name="Seal Device">
          <stats><vitals hp="1000" hpRegen="0" mp="100" mpRegen="0" /></stats></npc>
        <npc id="27371" type="Monster" level="80" name="Lilim Butcher"/>
        <npc id="27377" type="Monster" level="80" name="Lilim Slayer"/>
        <npc id="32715" type="Folk" level="80" name="Lilith"/>
        <npc id="32716" type="Folk" level="80" name="Lilith's Steward"/>
        <npc id="32717" type="Folk" level="80" name="Lilith's Elite"/>
        <npc id="32718" type="Folk" level="80" name="Anakim"/>
        <npc id="32719" type="Folk" level="80" name="Anakim's Guardian"/>
        <npc id="32720" type="Folk" level="80" name="Anakim's Guard"/>
        <npc id="32721" type="Folk" level="80" name="Anakim's Executor"/>"#;
    const ITEMS: &str = r#"
        <item id="15310" type="EtcItem" name="Sacred Sword of Einhasad"><set name="is_questitem" val="true"/></item>
        <item id="13846" type="EtcItem" name="Seal of Binding"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;
    const TEMPLATE: &str = r#"
        <exitPoint x="171895" y="-17501" z="-4903" />
        <doorlist><door id="17240102" /><door id="17240104" /><door id="17240106" /><door id="17240108" /><door id="17240110" /><door id="17240111" /></doorlist>
        <spawnlist>
          <group name="general">
            <npc id="32657" x="-84398" y="216106" z="-7504" heading="32768" />
            <npc id="27384" x="-83177" y="217353" z="-7520" heading="32768" allowRandomWalk="false" />
          </group>
        </spawnlist>"#;

    struct Quest;

    impl Script for Quest {
        fn name(&self) -> &'static str {
            Q00196
        }
        fn path(&self) -> &'static str {
            "quests/Standin"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    /// The doors of the normal world that the instance template copies.
    fn make_doors(t: &mut ScriptTest, door_ids: &[i32]) {
        let ids = Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap());
        t.world.set_id_factory(Arc::clone(&ids));
        let doors: String = door_ids
            .iter()
            .map(|id| {
                format!(
                    r#"<door id="{id}" name="door{id}" height="150" baseHpMax="1000" pos="0,0,0"
                    node1="0,0" node2="0,10" node3="10,10" node4="10,0" nodeZ="0"/>"#
                )
            })
            .collect();
        let templates = DoorTemplates::from_xml(&format!("<list>{doors}</list>")).unwrap();
        t.world.spawn_doors(&templates, &ids).unwrap();
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Quest);
        registry.add(SSQDisciplesNecropolisPast);
        let pages = Pages::of(&SSQDisciplesNecropolisPast)
            .plain(&["32586.htm", "32587.htm", "32657.htm", "32587-01.html"])
            .npcs(&[]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        make_doors(
            &mut t,
            &[17240102, 17240104, 17240106, 17240108, 17240110, 17240111],
        );
        t.world.set_instance_data(instance_data(
            &[(112, "Disciple's Necropolis (Past)")],
            &[(112, TEMPLATE)],
        ));
        let mammon = t.spawn(PROMISE_OF_MAMMON, 20);
        let leon = t.spawn(LEON, 30);
        (t, mammon, leon)
    }

    fn set_cond(t: &mut ScriptTest, cond: i32) {
        t.world.run_as_script(Q00196, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, STARTED);
            qs.set_cond(ctx, cond, false);
        });
        t.take_writes();
    }

    fn talk(t: &mut ScriptTest, npc: Oid) -> Option<String> {
        let id = t.world.npc(npc).unwrap().npc_id;
        let mut result = None;
        t.world.run_as_script("SSQDisciplesNecropolisPast", |ctx| {
            result = SSQDisciplesNecropolisPast.on_talk(ctx, ScriptNpc { object_id: npc, id }, Oid(1));
        });
        t.ticks(60);
        result
    }

    fn event(t: &mut ScriptTest, name: &str) {
        t.world.run_as_script("SSQDisciplesNecropolisPast", |ctx| {
            SSQDisciplesNecropolisPast.on_event(ctx, name, None, Some(Oid(1)));
        });
    }

    /// Player 1 at cond 3 is inside.
    fn inside() -> (ScriptTest, Oid, Oid) {
        let (mut t, mammon, leon) = setup();
        t.enter(1, 80, vec![]);
        set_cond(&mut t, 3);
        assert_eq!(talk(&mut t, mammon).as_deref(), Some(""));
        (t, mammon, leon)
    }

    #[test]
    fn a_player_without_the_quest_gets_the_no_quest_page_and_one_at_cond_3_goes_in() {
        let (mut t, mammon, _) = setup();
        t.enter(1, 80, vec![]);
        let page = talk(&mut t, mammon).unwrap();
        assert!(page.contains("html"), "{page}");
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        set_cond(&mut t, 2);
        assert!(talk(&mut t, mammon).unwrap().contains("html"));
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);

        set_cond(&mut t, 4);
        assert_eq!(talk(&mut t, mammon).as_deref(), Some(""));
        let instance_id = t.world.instance_id_of(Oid(1));
        assert_eq!(instance_id, 300_000);
        let world = t.world.instance_world(instance_id).unwrap();
        assert_eq!((world.template_id, world.allowed.clone()), (112, vec![Oid(1)]));
        // Lilith's three and Anakim's four fight in the instance.
        assert_eq!(groups(&t, "lilithGroup").len(), 3);
        let anakim: Vec<i32> = groups(&t, "anakimGroup").iter().map(|n| n.id).collect();
        assert_eq!(
            anakim,
            vec![ANAKIMS_GUARD, ANAKIMS_EXECUTOR, ANAKIM, ANAKIMS_GUARDIAN]
        );
        let offset = t.world.max_offset_on_teleport();
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert!((x - ENTER.0).abs() <= offset && (y - ENTER.1).abs() <= offset);
    }

    /// The groups of the instance of player 1, read from its parameters.
    fn groups(t: &ScriptTest, name: &str) -> Vec<ScriptNpc> {
        let world = t.world.instance_world(t.world.instance_id_of(Oid(1))).unwrap();
        decode_group(world.parameters.get(name).unwrap())
    }

    #[test]
    fn leon_takes_the_sword_and_sends_the_player_out() {
        let (mut t, _, leon) = inside();
        let instance_id = t.world.instance_id_of(Oid(1));
        t.world
            .run_as_script(Q00196, |ctx| ctx.give_items(Oid(1), SACRED_SWORD_OF_EINHASAD, 1));
        assert_eq!(talk(&mut t, leon).as_deref(), Some("32587-01.html"));
        assert_eq!(t.count(1, SACRED_SWORD_OF_EINHASAD), 0);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (EXIT.0, EXIT.1));
        assert!(t.world.instance_world(instance_id).unwrap().allowed.is_empty());
    }

    #[test]
    fn the_first_pages_are_the_npc_id_pages() {
        let (mut t, _, _) = setup();
        t.enter(1, 80, vec![]);
        let shunaiman = t.spawn(SHUNAIMAN, 10);
        t.world.run_as_script("SSQDisciplesNecropolisPast", |ctx| {
            let npc = ScriptNpc {
                object_id: shunaiman,
                id: SHUNAIMAN,
            };
            assert_eq!(
                SSQDisciplesNecropolisPast
                    .on_first_talk(ctx, npc, Oid(1))
                    .as_deref(),
                Some("32586.htm")
            );
        });
    }

    #[test]
    fn the_kills_open_the_doors_at_4_10_18_28_and_40() {
        let (mut t, _, _) = inside();
        let instance_id = t.world.instance_id_of(Oid(1));
        let slayer = t.spawn(LILIM_SLAYER, 10);
        let npc = ScriptNpc {
            object_id: slayer,
            id: LILIM_SLAYER,
        };
        let doors = [
            (4, DOOR_1),
            (10, DOOR_2),
            (18, DOOR_3),
            (28, DOOR_4),
            (40, DOOR_5),
        ];
        for kill in 1..=40 {
            t.world.run_as_script("SSQDisciplesNecropolisPast", |ctx| {
                SSQDisciplesNecropolisPast.on_kill(ctx, npc, Oid(1), false);
            });
            for (at, door) in doors {
                assert_eq!(
                    t.world.is_instance_door_open(instance_id, door),
                    kill >= at,
                    "kill {kill}, door {door}"
                );
            }
        }
        assert_eq!(param_int_of(&t, instance_id, "countKill"), 40);
    }

    fn param_int_of(t: &ScriptTest, instance_id: i32, name: &str) -> i32 {
        t.world
            .instance_world(instance_id)
            .and_then(|w| w.parameters.get(name))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }

    #[test]
    fn the_gatekeeper_opens_the_door_and_the_fight_loop_runs() {
        let (mut t, _, _) = inside();
        let instance_id = t.world.instance_id_of(Oid(1));
        let gatekeepers = t.npcs_of(DISCIPLES_GATEKEEPER);
        assert_eq!(gatekeepers.len(), 1);
        // NOTE(java-quirk): the answer is the no-quest page.
        let page = talk(&mut t, gatekeepers[0]).unwrap();
        assert!(page.contains("html"));
        assert!(
            t.world
                .is_instance_door_open(instance_id, DISCIPLES_NECROPOLIS_DOOR)
        );
        assert_eq!(t.world.quest_timer_count(), 1);
        t.wait_ms(1_100);
        // The first FIGHT: the leaders shout once (script value 1) and the loop goes on.
        let lilith = groups(&t, "lilithGroup")[0];
        let anakim = groups(&t, "anakimGroup")[2];
        assert_eq!((lilith.id, anakim.id), (LILITH, ANAKIM));
        assert!(t.world.is_npc_script_value(lilith.object_id, 1));
        assert!(t.world.is_npc_script_value(anakim.object_id, 1));
        assert_eq!(t.world.quest_timer_count(), 1);
    }

    #[test]
    fn a_seal_device_at_a_tenth_of_its_hp_gives_the_seal_and_ends_with_the_scene() {
        let (mut t, _, _) = inside();
        let devices = t.npcs_of(SEAL_DEVICE);
        assert_eq!(devices.len(), 1);
        let device = ScriptNpc {
            object_id: devices[0],
            id: SEAL_DEVICE,
        };
        t.world.run_as_script("SSQDisciplesNecropolisPast", |ctx| {
            SSQDisciplesNecropolisPast.on_spawn(ctx, device);
        });
        assert!(!t.world.is_mortal(devices[0]));
        // Full hp: nothing yet.
        t.world.run_as_script("SSQDisciplesNecropolisPast", |ctx| {
            SSQDisciplesNecropolisPast.on_attack(ctx, device, Oid(1), 10, false, None);
        });
        assert_eq!(t.count(1, SEAL_OF_BINDING), 0);
        t.world.set_current_hp(devices[0], 50.0, false);
        t.world.run_as_script("SSQDisciplesNecropolisPast", |ctx| {
            SSQDisciplesNecropolisPast.on_attack(ctx, device, Oid(1), 10, false, None);
            SSQDisciplesNecropolisPast.on_attack(ctx, device, Oid(1), 10, false, None);
        });
        // Once (script value), and FINISH is on the wheel.
        assert_eq!(t.count(1, SEAL_OF_BINDING), 1);
        assert_eq!(t.world.quest_timer_count(), 1);
        // FINISH with fewer than 4 seals does nothing; with 4 the scene runs and TELEPORT follows.
        t.wait_ms(1_100);
        assert_eq!(t.world.quest_timer_count(), 0);
        t.world
            .run_as_script(Q00196, |ctx| ctx.give_items(Oid(1), SEAL_OF_BINDING, 3));
        event(&mut t, "FINISH");
        assert_eq!(t.world.quest_timer_count(), 1);
        t.world.tele_to_location(
            Oid(1),
            TeleportTarget::at(ENTER.0 + 500, ENTER.1, ENTER.2).with_instance(300_000),
        );
        t.ticks(60);
        t.wait_ms(27_100);
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (ENTER.0, ENTER.1));
    }

    #[test]
    fn a_lilim_that_sees_the_player_speaks_once() {
        let (mut t, _, _) = inside();
        // The client loaded the target: the player sees what happens around him.
        t.input(1, ClientInput::Appearing);
        let instance_id = t.world.instance_id_of(Oid(1));
        let butcher = t
            .world
            .script_add_spawn(
                LILIM_BUTCHER,
                (ENTER.0, ENTER.1, ENTER.2),
                0,
                false,
                0,
                instance_id,
            )
            .unwrap();
        let npc = ScriptNpc {
            object_id: butcher,
            id: LILIM_BUTCHER,
        };
        t.take();
        t.world.run_as_script("SSQDisciplesNecropolisPast", |ctx| {
            SSQDisciplesNecropolisPast.on_aggro_range_enter(ctx, npc, Oid(1), false);
            assert!(ctx.is_script_value(npc, 1));
        });
        assert!(!t.take().is_empty());
        t.world.run_as_script("SSQDisciplesNecropolisPast", |ctx| {
            SSQDisciplesNecropolisPast.on_aggro_range_enter(ctx, npc, Oid(1), false);
        });
        assert!(t.take().is_empty());
    }
}
