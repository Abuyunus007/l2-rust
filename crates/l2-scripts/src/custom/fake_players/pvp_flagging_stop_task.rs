//! The pvp flag of a fake player: it blinks and goes out when the fight is over.
// Java: dist/game/data/scripts/custom/FakePlayers/PvpFlaggingStopTask.java
//
// Java: "TODO: Move it to Creature." The world starts it (`FLAG_CHECK`) when a fake player attacks
// a playable (`l2_world::custom::fake_players`); the script value of the NPC is its flag: 0 none,
// 1 in combat, 2 blinking.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

/// Java `custom.FakePlayers.PvpFlaggingStopTask`.
pub struct PvpFlaggingStopTask;

impl Script for PvpFlaggingStopTask {
    fn name(&self) -> &'static str {
        "PvpFlaggingStopTask"
    }

    fn path(&self) -> &'static str {
        "custom/FakePlayers"
    }

    // Nothing is registered: the world notifies the script by its name.
    fn setup(&self, _s: &mut ScriptSetup) {}

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        let npc = npc?;
        if ctx.is_dead(npc.object_id) {
            ctx.cancel_quest_timer("FLAG_CHECK", Some(npc), None);
            ctx.cancel_quest_timer("FINISH_FLAG", Some(npc), None);
            ctx.cancel_quest_timer("REMOVE_FLAG", Some(npc), None);
            return None;
        }

        match event {
            "FLAG_CHECK" => {
                if ctx.world().fake_player_targets_playable(npc.object_id) {
                    ctx.world_mut().set_npc_script_value(npc.object_id, 1); // in combat
                    ctx.cancel_quest_timer("FINISH_FLAG", Some(npc), None);
                    ctx.cancel_quest_timer("REMOVE_FLAG", Some(npc), None);
                    let normal_time = i64::from(ctx.world().pvp_normal_time_ms());
                    ctx.start_quest_timer("FINISH_FLAG", normal_time - 20_000, Some(npc), None, false);
                    ctx.start_quest_timer("FLAG_CHECK", 5000, Some(npc), None, false);
                }
            }
            "FINISH_FLAG" => {
                if ctx.world().is_npc_script_value(npc.object_id, 1) {
                    ctx.world_mut().set_npc_script_value(npc.object_id, 2); // blink status
                    ctx.world_mut().fake_player_broadcast_info(npc.object_id); // update flag status
                    ctx.start_quest_timer("REMOVE_FLAG", 20_000, Some(npc), None, false);
                }
            }
            "REMOVE_FLAG" => {
                if ctx.world().is_npc_script_value(npc.object_id, 2) {
                    ctx.world_mut().set_npc_script_value(npc.object_id, 0); // not in combat
                    ctx.world_mut().fake_player_broadcast_info(npc.object_id); // update flag status
                }
            }
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use l2_data::{NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions};
    use l2_protocol_game::server::CharInfo;
    use l2_world::{ClientInput, FakePlayerChatEntry, FakePlayerSettings, ScriptRegistry, WorldSettings};

    use super::*;
    use crate::testing::{ScriptTest, decode, name_of, names_to};

    const PLAYER: i32 = 1;
    // A fake player: a Monster with a `<fakePlayer>` element.
    const NPCS_XML: &str = r#"<npc id="81001" type="Monster" level="10" name="Gladus" title="Duelist">
        <fakePlayer classId="88" equipRHand="5706" hair="2"/>
        <status talkable="false" attackable="true"/>
    </npc>"#;

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    fn heard(sent: &[(i32, Vec<u8>)], by: i32, text: &str) -> bool {
        let needle = utf16(text);
        sent.iter().any(|(to, bytes)| {
            *to == by
                && name_of(bytes) == "SAY2"
                && bytes.windows(needle.len()).any(|w| w == needle.as_slice())
        })
    }

    /// The fake player system with `enabled` and `settings` (its `enabled` is set from the
    /// argument), the NPC data loaded the way the server loads it for that switch.
    fn scene(enabled: bool, settings: FakePlayerSettings) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(PvpFlaggingStopTask);
        let mut t = ScriptTest::new_with(registry, "", "", &[], move |world, root| {
            let options = NpcLoadOptions {
                fake_players_enabled: enabled,
                ..NpcLoadOptions::default()
            };
            let npcs =
                Arc::new(NpcTemplates::from_xml(&format!("<list>{NPCS_XML}</list>"), &options).unwrap());
            let spawns = Arc::new(SpawnData::load(root, &npcs, &SpawnLoadOptions::default()));
            world.set_npc_data(npcs, spawns);
            world.set_settings(WorldSettings {
                fake_players: FakePlayerSettings { enabled, ..settings },
                ..WorldSettings::default()
            });
            world.load_fake_players(vec![
                FakePlayerChatEntry {
                    fpc_name: "ALL".to_owned(),
                    search_method: "EQUALS".to_owned(),
                    search_text: vec!["hi".to_owned()],
                    answers: vec!["hello".to_owned()],
                },
                FakePlayerChatEntry {
                    fpc_name: "Gladus".to_owned(),
                    search_method: "CONTAINS".to_owned(),
                    search_text: vec!["how".to_owned(), "you".to_owned()],
                    answers: vec!["fine".to_owned()],
                },
            ]);
        });
        t.enter(PLAYER, 20, vec![]);
        t
    }

    fn on() -> FakePlayerSettings {
        FakePlayerSettings {
            chat: true,
            ..FakePlayerSettings::default()
        }
    }

    fn whisper(t: &mut ScriptTest, text: &str) {
        t.input(
            PLAYER,
            ClientInput::Say {
                chat_type: 2,
                text: text.to_owned(),
                target: Some("gladus".to_owned()),
            },
        );
    }

    #[test]
    fn with_the_switch_off_there_is_no_fake_player() {
        let mut t = scene(false, on());
        assert!(t.world.spawn_npc_at(81001, 0, 0, 0, 0).is_none());
        assert!(!t.world.fake_players_enabled());
        assert_eq!(t.world.fake_player_proper_name("gladus"), None);
        // A whisper to the name is an ordinary whisper to a player who is not online.
        let _ = t.take();
        whisper(&mut t, "hi");
        assert!(!heard(&t.take(), PLAYER, "->Gladus"));
    }

    #[test]
    fn a_fake_player_is_told_to_the_clients_as_a_player() {
        let mut t = scene(true, on());
        let _ = t.take();
        let npc = t.spawn(81001, 20);
        let out = t.take();
        let names = names_to(&out, PLAYER);
        assert!(names.contains(&"CHAR_INFO".to_owned()), "{names:?}");
        assert!(!names.contains(&"NPC_INFO".to_owned()), "{names:?}");
        let (_, raw) = out.iter().find(|(_, b)| name_of(b) == "CHAR_INFO").unwrap();
        let info = decode::<CharInfo>(raw);
        assert_eq!(info.object_id, npc.0);
        assert_eq!(info.name, "Gladus");
        assert_eq!(info.class_id, 88);
        assert_eq!(info.hair_style, 2);
        assert_eq!(
            info.paperdoll_item_ids[l2_protocol_game::server::charscreen::paperdoll::RHAND],
            5706
        );
        // The pvp flag of the packet is the script value.
        assert_eq!(info.pvp_flag, 0);
    }

    #[test]
    fn a_whisper_is_answered_after_a_delay() {
        let mut t = scene(true, on());
        let _npc = t.spawn(81001, 20);
        let _ = t.take();
        whisper(&mut t, "hi");
        let out = t.take();
        // The sender sees its own whisper at once.
        assert!(heard(&out, PLAYER, "->Gladus"));
        assert!(!heard(&out, PLAYER, "hello"));
        // The answer comes 5 to 15 seconds later.
        t.wait_ms(4_900);
        assert!(!heard(&t.take(), PLAYER, "hello"));
        t.wait_ms(10_200);
        assert!(heard(&t.take(), PLAYER, "hello"));
        // A line that needs all its words.
        whisper(&mut t, "how are you");
        let _ = t.take();
        t.wait_ms(15_100);
        assert!(heard(&t.take(), PLAYER, "fine"));
    }

    #[test]
    fn without_fake_player_chat_the_whisper_is_refused() {
        let mut t = scene(
            true,
            FakePlayerSettings {
                chat: false,
                ..FakePlayerSettings::default()
            },
        );
        let _npc = t.spawn(81001, 20);
        let _ = t.take();
        whisper(&mut t, "hi");
        let out = t.take();
        assert!(!heard(&out, PLAYER, "->Gladus"));
        t.wait_ms(15_100);
        assert!(!heard(&t.take(), PLAYER, "hello"));
    }

    #[test]
    fn a_fake_player_is_only_attackable_in_combat_or_by_the_setting() {
        let mut t = scene(true, on());
        let npc = t.spawn(81001, 20);
        let player = ObjectId(PLAYER);
        // `Monster.isAutoAttackable` of a fake player: not without a fight.
        assert!(!t.world.is_auto_attackable_by(npc, player));
        // The pvp flag (script value 1, blinking 2) makes it one.
        t.world.set_npc_script_value(npc, 1);
        assert!(t.world.is_auto_attackable_by(npc, player));
        t.world.set_npc_script_value(npc, 0);
        let mut t = scene(
            true,
            FakePlayerSettings {
                auto_attackable: true,
                ..on()
            },
        );
        let npc = t.spawn(81001, 20);
        assert!(t.world.is_auto_attackable_by(npc, player));
    }

    #[test]
    fn killing_a_fake_player_gives_a_pvp_kill_with_the_setting() {
        let mut t = scene(
            true,
            FakePlayerSettings {
                kill_pvp: true,
                ..on()
            },
        );
        let npc = t.spawn(81001, 20);
        t.kill(PLAYER, npc);
        assert_eq!(t.world.player(ObjectId(PLAYER)).unwrap().data.pvp_kills, 1);
        // Without it nothing is given.
        let mut t = scene(true, on());
        let npc = t.spawn(81001, 20);
        t.kill(PLAYER, npc);
        assert_eq!(t.world.player(ObjectId(PLAYER)).unwrap().data.pvp_kills, 0);
    }

    #[test]
    fn the_flag_goes_from_combat_to_blinking_to_none() {
        let mut t = scene(true, on());
        let npc = t.spawn(81001, 20);
        let scripted = ScriptNpc {
            object_id: npc,
            id: 81001,
        };
        assert!(t.world.is_npc_script_value(npc, 0));
        // Without a target nothing happens.
        t.world
            .notify_named_script_event("PvpFlaggingStopTask", "FLAG_CHECK", Some(scripted), None);
        assert!(t.world.is_npc_script_value(npc, 0));
        // Blinking only follows combat.
        t.world
            .notify_named_script_event("PvpFlaggingStopTask", "FINISH_FLAG", Some(scripted), None);
        assert!(t.world.is_npc_script_value(npc, 0));
        t.world.set_npc_script_value(npc, 1);
        t.world
            .notify_named_script_event("PvpFlaggingStopTask", "FINISH_FLAG", Some(scripted), None);
        assert!(t.world.is_npc_script_value(npc, 2));
        t.world
            .notify_named_script_event("PvpFlaggingStopTask", "REMOVE_FLAG", Some(scripted), None);
        assert!(t.world.is_npc_script_value(npc, 0));
    }
}
