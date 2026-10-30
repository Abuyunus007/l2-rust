//! The Cruma Tower alarm of the quests "Art of Persuasion" and "Nikola's Cooperation".
// Java: dist/game/data/scripts/ai/areas/CrumaTower/Alarm/Alarm.java
//
// The quests spawn the alarm and keep the player and the device on its variables (`player0`,
// `npc0`, object ids as longs).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ALARM: i32 = 32367;
// Misc
const ART_OF_PERSUASION_ID: i32 = 184;
const NIKOLAS_COOPERATION_ID: i32 = 185;
/// Java `Q00184_ArtOfPersuasion.class.getSimpleName()`.
const ART_OF_PERSUASION: &str = "Q00184_ArtOfPersuasion";
/// Java `Q00185_NikolasCooperation.class.getSimpleName()`.
const NIKOLAS_COOPERATION: &str = "Q00185_NikolasCooperation";

/// Java `ai.areas.CrumaTower.Alarm.Alarm`.
pub struct Alarm;

fn quest_name(quest_id: i32) -> Option<&'static str> {
    match quest_id {
        ART_OF_PERSUASION_ID => Some(ART_OF_PERSUASION),
        NIKOLAS_COOPERATION_ID => Some(NIKOLAS_COOPERATION),
        _ => None,
    }
}

/// Java `npc.getVariables().getObject(name, Player.class)` / `Npc.class`: the quests keep the
/// object id.
fn object_var(ctx: &ScriptCtx<'_>, npc: ScriptNpc, name: &str) -> Option<ObjectId> {
    let id = ctx.world().npc_variables(npc.object_id)?.get_long(name, 0);
    i32::try_from(id).ok().filter(|id| *id != 0).map(ObjectId)
}

fn verify_memo_state(ctx: &mut ScriptCtx<'_>, player: ObjectId, quest_id: i32, memo_state: i32) -> bool {
    let Some(name) = quest_name(quest_id) else {
        return false;
    };
    let Some(qs) = ctx.quest_state_of(name, player, false) else {
        return false;
    };
    memo_state < 0 || qs.is_memo_state(ctx, memo_state)
}

fn set_memo_state(ctx: &mut ScriptCtx<'_>, player: ObjectId, quest_id: i32, memo_state: i32) {
    let Some(name) = quest_name(quest_id) else {
        return;
    };
    if let Some(qs) = ctx.quest_state_of(name, player, false) {
        qs.set_memo_state(ctx, memo_state);
    }
}

fn get_memo_state_ex(ctx: &mut ScriptCtx<'_>, player: ObjectId, quest_id: i32, slot: i32) -> i32 {
    let Some(name) = quest_name(quest_id) else {
        return -1;
    };
    match ctx.quest_state_of(name, player, false) {
        Some(qs) => qs.get_memo_state_ex(ctx, slot),
        None => -1,
    }
}

fn set_memo_state_ex(ctx: &mut ScriptCtx<'_>, player: ObjectId, quest_id: i32, slot: i32, value: i32) {
    let Some(name) = quest_name(quest_id) else {
        return;
    };
    if let Some(qs) = ctx.quest_state_of(name, player, false) {
        qs.set_memo_state_ex(ctx, slot, value);
    }
}

/// `npc0.getVariables().getBoolean("SPAWNED")`.
fn device_spawned(ctx: &ScriptCtx<'_>, npc0: Option<ObjectId>) -> bool {
    npc0.is_some_and(|n| {
        ctx.world()
            .npc_variables(n)
            .is_some_and(|v| v.get_bool("SPAWNED", false))
    })
}

fn clear_device_spawned(ctx: &mut ScriptCtx<'_>, npc0: ObjectId) {
    ctx.world_mut().npc_variables_mut(npc0).set_bool("SPAWNED", false);
}

impl Script for Alarm {
    fn name(&self) -> &'static str {
        "Alarm"
    }

    fn path(&self) -> &'static str {
        "ai/areas/CrumaTower/Alarm"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ALARM]);
        s.add_talk_id(&[ALARM]);
        s.add_first_talk_id(&[ALARM]);
        s.add_spawn_id(&[ALARM]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        // Java reads the variables of `npc` before the switch (a timer or link always has one).
        let npc = npc?;
        let player0 = object_var(ctx, npc, "player0");
        let npc0 = object_var(ctx, npc, "npc0");
        match event {
            "SELF_DESTRUCT_IN_60" => {
                ctx.start_quest_timer("SELF_DESTRUCT_IN_30", 30000, Some(npc), None, false);
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::TheAlarmWillSelfDestructIn60SecondsEnterPasscodeToOverride,
                    &[],
                );
            }
            "SELF_DESTRUCT_IN_30" => {
                ctx.start_quest_timer("SELF_DESTRUCT_IN_10", 20000, Some(npc), None, false);
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::TheAlarmWillSelfDestructIn30SecondsEnterPasscodeToOverride,
                    &[],
                );
            }
            "SELF_DESTRUCT_IN_10" => {
                ctx.start_quest_timer("RECORDER_CRUSHED", 10000, Some(npc), None, false);
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::TheAlarmWillSelfDestructIn10SecondsEnterPasscodeToOverride,
                    &[],
                );
            }
            "RECORDER_CRUSHED" => {
                if let Some(npc0) = npc0
                    && device_spawned(ctx, Some(npc0))
                {
                    clear_device_spawned(ctx, npc0);
                    if let Some(player0) = player0 {
                        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::RecorderCrushed, &[]);
                        if verify_memo_state(ctx, player0, ART_OF_PERSUASION_ID, -1) {
                            set_memo_state(ctx, player0, ART_OF_PERSUASION_ID, 5);
                        } else if verify_memo_state(ctx, player0, NIKOLAS_COOPERATION_ID, -1) {
                            set_memo_state(ctx, player0, NIKOLAS_COOPERATION_ID, 5);
                        }
                    }
                }
                ctx.delete_npc(npc);
            }
            "32367-184_04.html" | "32367-184_06.html" | "32367-184_08.html" => {
                htmltext = Some(event.to_owned());
            }
            "2" => {
                if player0 == player {
                    let player = player?;
                    if verify_memo_state(ctx, player, ART_OF_PERSUASION_ID, 3) {
                        htmltext = Some("32367-184_02.html".to_owned());
                    } else if verify_memo_state(ctx, player, NIKOLAS_COOPERATION_ID, 3) {
                        htmltext = Some("32367-185_02.html".to_owned());
                    }
                }
            }
            "3" => {
                let player = player?;
                if verify_memo_state(ctx, player, ART_OF_PERSUASION_ID, 3) {
                    set_memo_state_ex(ctx, player, ART_OF_PERSUASION_ID, 1, 1);
                    htmltext = Some("32367-184_04.html".to_owned());
                } else if verify_memo_state(ctx, player, NIKOLAS_COOPERATION_ID, 3) {
                    set_memo_state_ex(ctx, player, NIKOLAS_COOPERATION_ID, 1, 1);
                    htmltext = Some("32367-185_04.html".to_owned());
                }
            }
            "4" => {
                let player = player?;
                if verify_memo_state(ctx, player, ART_OF_PERSUASION_ID, 3) {
                    let value = get_memo_state_ex(ctx, player, ART_OF_PERSUASION_ID, 1) + 1;
                    set_memo_state_ex(ctx, player, ART_OF_PERSUASION_ID, 1, value);
                    htmltext = Some("32367-184_06.html".to_owned());
                } else if verify_memo_state(ctx, player, NIKOLAS_COOPERATION_ID, 3) {
                    let value = get_memo_state_ex(ctx, player, NIKOLAS_COOPERATION_ID, 1) + 1;
                    set_memo_state_ex(ctx, player, NIKOLAS_COOPERATION_ID, 1, value);
                    htmltext = Some("32367-185_06.html".to_owned());
                }
            }
            "5" => {
                let player = player?;
                if verify_memo_state(ctx, player, ART_OF_PERSUASION_ID, 3) {
                    let value = get_memo_state_ex(ctx, player, ART_OF_PERSUASION_ID, 1) + 1;
                    set_memo_state_ex(ctx, player, ART_OF_PERSUASION_ID, 1, value);
                    htmltext = Some("32367-184_08.html".to_owned());
                } else if verify_memo_state(ctx, player, NIKOLAS_COOPERATION_ID, 3) {
                    let value = get_memo_state_ex(ctx, player, NIKOLAS_COOPERATION_ID, 1) + 1;
                    set_memo_state_ex(ctx, player, NIKOLAS_COOPERATION_ID, 1, value);
                    htmltext = Some("32367-185_08.html".to_owned());
                }
            }
            "6" => {
                let player = player?;
                if verify_memo_state(ctx, player, ART_OF_PERSUASION_ID, 3) {
                    if get_memo_state_ex(ctx, player, ART_OF_PERSUASION_ID, 1) >= 3 {
                        if let Some(npc0) = npc0
                            && device_spawned(ctx, Some(npc0))
                        {
                            clear_device_spawned(ctx, npc0);
                        }
                        set_memo_state(ctx, player, ART_OF_PERSUASION_ID, 4);
                        htmltext = Some("32367-184_09.html".to_owned());
                        ctx.delete_npc(npc);
                    } else {
                        set_memo_state_ex(ctx, player, ART_OF_PERSUASION_ID, 1, 0);
                        htmltext = Some("32367-184_10.html".to_owned());
                    }
                } else if verify_memo_state(ctx, player, NIKOLAS_COOPERATION_ID, 3) {
                    if get_memo_state_ex(ctx, player, NIKOLAS_COOPERATION_ID, 1) >= 3 {
                        if let Some(npc0) = npc0
                            && device_spawned(ctx, Some(npc0))
                        {
                            clear_device_spawned(ctx, npc0);
                        }
                        set_memo_state(ctx, player, NIKOLAS_COOPERATION_ID, 4);
                        htmltext = Some("32367-185_09.html".to_owned());
                        ctx.delete_npc(npc);
                    } else {
                        set_memo_state_ex(ctx, player, NIKOLAS_COOPERATION_ID, 1, 0);
                        htmltext = Some("32367-185_10.html".to_owned());
                    }
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let mut htmltext = ctx.get_no_quest_msg(talker);
        if verify_memo_state(ctx, talker, ART_OF_PERSUASION_ID, 3)
            || verify_memo_state(ctx, talker, NIKOLAS_COOPERATION_ID, 3)
        {
            htmltext = if object_var(ctx, npc, "player0") == Some(talker) {
                "32367-01.html".to_owned()
            } else {
                "32367-02.html".to_owned()
            };
        }
        Some(htmltext)
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.cancel_quest_timer("SELF_DESTRUCT_IN_60", Some(npc), None);
        ctx.start_quest_timer("SELF_DESTRUCT_IN_60", 60000, Some(npc), None, false);
        ctx.broadcast_npc_say(
            npc,
            ChatType::NpcGeneral,
            NpcStringId::IntruderAlertTheAlarmWillSelfDestructIn2Minutes,
            &[],
        );
        if let Some(player) = object_var(ctx, npc, "player0") {
            ctx.play_sound(player, QuestSound::ItemsoundSiren);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"<npc id="32367" type="Folk" level="70" name="Alarm"/>
        <npc id="32366" type="Folk" level="70" name="Device"/>"#;
    const DEVICE: i32 = 32366;

    /// A quest of Java's name, so the alarm finds a state to read and write.
    struct StandIn(&'static str, i32);

    impl Script for StandIn {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests/StandIn"
        }
        fn id(&self) -> i32 {
            self.1
        }
        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, ObjectId, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Alarm);
        registry.add(StandIn(ART_OF_PERSUASION, 184));
        registry.add(StandIn(NIKOLAS_COOPERATION, 185));
        let pages = Pages::of(&Alarm)
            .page("32367-01.html", &["2", "3", "4", "5", "6"])
            .page("32367-184_02.html", &["3", "4", "5", "6"])
            .page("32367-184_04.html", &["4", "5", "6"])
            .page("32367-184_06.html", &["4", "5", "6"])
            .page("32367-184_08.html", &["4", "5", "6"])
            .plain(&["32367-02.html", "32367-184_09.html", "32367-184_10.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        let device = t.spawn(DEVICE, 40);
        let alarm = t.spawn(ALARM, 20);
        t.world.npc_variables_mut(device).set_bool("SPAWNED", true);
        let vars = t.world.npc_variables_mut(alarm);
        vars.set_long("player0", 1);
        vars.set_long("npc0", i64::from(device.0));
        t.enter(1, 50, vec![]);
        t.world.run_as_script("Alarm", |ctx| {
            let qs = ctx.quest_state_of(ART_OF_PERSUASION, ObjectId(1), true).unwrap();
            qs.start_quest(ctx);
            qs.set_memo_state(ctx, 3);
        });
        (t, alarm, device)
    }

    #[test]
    fn the_passcode_game_counts_the_answers_and_opens_the_alarm() {
        let (mut t, alarm, device) = setup();
        t.talk(1, alarm);
        assert!(t.event(1, "Alarm", "2").contains("32367-184_02.html"));
        assert!(t.event(1, "Alarm", "3").contains("32367-184_04.html"));
        assert!(t.event(1, "Alarm", "4").contains("32367-184_06.html"));
        assert!(t.event(1, "Alarm", "5").contains("32367-184_08.html"));
        // The counter is 3 now: the alarm is defused.
        assert!(t.event(1, "Alarm", "6").contains("32367-184_09.html"));
        assert!(!t.in_world(alarm));
        assert!(!t.world.npc_variables(device).unwrap().get_bool("SPAWNED", true));
        t.world.run_as_script("Alarm", |ctx| {
            let qs = ctx.quest_state_of(ART_OF_PERSUASION, ObjectId(1), false).unwrap();
            assert!(qs.is_memo_state(ctx, 4));
        });
    }

    #[test]
    fn a_wrong_round_resets_the_counter_and_the_timers_crush_the_recorder() {
        let (mut t, alarm, device) = setup();
        t.talk(1, alarm);
        t.event(1, "Alarm", "3");
        assert!(t.event(1, "Alarm", "6").contains("32367-184_10.html"));
        assert!(t.in_world(alarm));
        // Nobody answers: 60 + 30 + 10 s later the recorder is crushed (memo state 5).
        t.wait_ms(121_000);
        assert!(!t.in_world(alarm));
        assert!(!t.world.npc_variables(device).unwrap().get_bool("SPAWNED", true));
        t.world.run_as_script("Alarm", |ctx| {
            let qs = ctx.quest_state_of(ART_OF_PERSUASION, ObjectId(1), false).unwrap();
            assert!(qs.is_memo_state(ctx, 5));
        });
    }

    #[test]
    fn only_the_owner_of_the_alarm_gets_the_first_page() {
        let (mut t, alarm, _) = setup();
        t.enter(2, 50, vec![]);
        t.world.run_as_script("Alarm", |ctx| {
            let qs = ctx.quest_state_of(ART_OF_PERSUASION, ObjectId(2), true).unwrap();
            qs.start_quest(ctx);
            qs.set_memo_state(ctx, 3);
        });
        t.take();
        t.talk(2, alarm);
        assert!(
            crate::testing::pages_to(&t.take(), 2)
                .pop()
                .unwrap()
                .contains("32367-02.html")
        );
        t.talk(1, alarm);
        assert!(
            crate::testing::pages_to(&t.take(), 1)
                .pop()
                .unwrap()
                .contains("32367-01.html")
        );
    }
}
