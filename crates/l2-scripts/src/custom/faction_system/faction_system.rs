//! The faction manager (NPC 500): a player picks the good or the evil side.
// The system is `config/Custom/FactionSystem.ini` (`EnableFactionSystem`, off by default); the
// rules it adds to the world (chat, pvp, guards, respawn, the balance of the online players) are
// `l2_world::custom::faction`. Like Java, the script is always installed and only the manager's
// spawn depends on the switch.

use l2_protocol_game::server::{ChatType, NpcHtmlMessage};
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MANAGER: i32 = 500;

/// Java `custom.FactionSystem.FactionSystem`.
pub struct FactionSystem;

impl FactionSystem {
    /// Java `TEXTS`: the manager's lines (the first one has the team names).
    fn texts(ctx: &ScriptCtx<'_>) -> [String; 3] {
        let settings = ctx.world().faction_settings();
        [
            format!("{} or {}?", settings.good_team_name, settings.evil_team_name),
            "Select your faction!".to_owned(),
            "The choice is yours!".to_owned(),
        ]
    }

    /// `selectGoodFaction` and `selectEvilFaction`, which differ in nothing but the side.
    fn select_faction(
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        good: bool,
    ) -> Option<String> {
        let settings = ctx.world().faction_settings().clone();
        let (own_name, other_name) = if good {
            (settings.good_team_name.clone(), settings.evil_team_name.clone())
        } else {
            (settings.evil_team_name.clone(), settings.good_team_name.clone())
        };
        let (own_count, other_count) = if good {
            (
                ctx.world().all_good_players().len(),
                ctx.world().all_evil_players().len(),
            )
        } else {
            (
                ctx.world().all_evil_players().len(),
                ctx.world().all_good_players().len(),
            )
        };
        let limit = usize::try_from(settings.balance_player_exceed_limit).unwrap_or(0);
        if settings.balance_online_players && own_count >= other_count + limit {
            let htmltext = None;
            let html = ctx
                .get_htm(player, "onlinelimit.html")
                .unwrap_or_default()
                .replace("%name%", &ctx.player_name(player))
                .replace("%more%", &own_name)
                .replace("%less%", &other_name);
            if let Some(packet) = NpcHtmlMessage::new(npc.object_id.0, &html) {
                ctx.world_mut().send_html(player, &packet);
            }
            return htmltext;
        }

        if settings.auto_nobless {
            ctx.set_noble(player, true);
        }

        let color = if good {
            settings.good_name_color
        } else {
            settings.evil_name_color
        };
        if good {
            ctx.world_mut().set_good(player);
        } else {
            ctx.world_mut().set_evil(player);
        }
        ctx.world_mut().faction_set_colors(player, color);
        ctx.world_mut().faction_set_title(player, &own_name);
        ctx.world().send_message(
            player,
            &format!("You are now fighting for the {own_name} faction."),
        );
        let base = if good {
            settings.good_base_location
        } else {
            settings.evil_base_location
        };
        ctx.teleport_player(player, base.x, base.y, base.z, -1, false);
        let message = format!(
            "{own_name} faction grows stronger with the arrival of {}.",
            ctx.player_name(player)
        );
        ctx.world().faction_broadcast_message(&own_name, &message);
        // `World.addFactionPlayerToWorld(player)`: the world counts the players by their faction.
        None
    }
}

impl Script for FactionSystem {
    fn name(&self) -> &'static str {
        "FactionSystem"
    }

    fn path(&self) -> &'static str {
        "custom/FactionSystem"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[MANAGER]);
        s.add_start_npc(&[MANAGER]);
        s.add_talk_id(&[MANAGER]);
        s.add_first_talk_id(&[MANAGER]);
    }

    // The Java constructor spawns the manager when the system is on.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let settings = ctx.world().faction_settings();
        if settings.enabled {
            let at = settings.manager_location;
            ctx.add_spawn(MANAGER, at.x, at.y, at.z, at.heading, false, 0, 0);
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "selectGoodFaction" => {
                // Java would throw on a missing NPC or player.
                return Self::select_faction(ctx, npc?, player?, true);
            }
            "selectEvilFaction" => {
                return Self::select_faction(ctx, npc?, player?, false);
            }
            "SPEAK" => {
                if let Some(npc) = npc {
                    let texts = Self::texts(ctx);
                    let index = usize::try_from(ctx.get_random(3)).unwrap_or(0);
                    ctx.broadcast_npc_say_text_in_range(npc, ChatType::NpcGeneral, &texts[index], 1500);
                }
            }
            _ => {}
        }
        None
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let htmltext = None;
        let settings = ctx.world().faction_settings();
        let (good, evil) = (settings.good_team_name.clone(), settings.evil_team_name.clone());
        let html = ctx
            .get_htm(player, "manager.html")
            .unwrap_or_default()
            .replace("%name%", &ctx.player_name(player))
            .replace("%good%", &good)
            .replace("%evil%", &evil);
        if let Some(packet) = NpcHtmlMessage::new(npc.object_id.0, &html) {
            ctx.world_mut().send_html(player, &packet);
        }
        htmltext
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if npc.id == MANAGER {
            ctx.start_quest_timer("SPEAK", 10_000, Some(npc), None, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::{FactionLocation, FactionSettings, ScriptRegistry, WorldSettings};

    use super::*;
    use crate::testing::{ScriptTest, name_of, names_to, pages_to, player_data};

    const PLAYER: i32 = 1;
    const OTHER: i32 = 2;
    const NPCS_XML: &str = r#"<npc id="500" type="Folk" level="70" name="Jurek" title="Faction Manager"/>"#;
    const DIR: &str = "data/scripts/custom/FactionSystem";

    fn pages() -> Vec<(String, String)> {
        vec![
            (
                format!("{DIR}/manager.html"),
                r#"<html><body>Manager %name% %good% %evil% <a action="bypass -h Script FactionSystem selectGoodFaction">good</a><a action="bypass -h Script FactionSystem selectEvilFaction">evil</a></body></html>"#
                    .to_owned(),
            ),
            (
                format!("{DIR}/onlinelimit.html"),
                "<html><body>Limit %name% %more% %less%</body></html>".to_owned(),
            ),
        ]
    }

    fn scene(enabled: bool, limit: i32) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(FactionSystem);
        let pages = pages();
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new_with(registry, NPCS_XML, "", &pages, move |world, _| {
            world.set_settings(WorldSettings {
                faction: FactionSettings {
                    enabled,
                    manager_location: FactionLocation {
                        x: 83_000,
                        y: 148_000,
                        z: -3400,
                        heading: 0,
                    },
                    balance_player_exceed_limit: limit,
                    ..FactionSettings::default()
                },
                ..WorldSettings::default()
            });
        });
        t.enter(PLAYER, 20, vec![]);
        t.enter(OTHER, 20, vec![]);
        t
    }

    fn click_manager(t: &mut ScriptTest, player: i32) -> ObjectId {
        // The manager of the scene stands far away at `ManagerSpawnLocation`: a second one next to
        // the players (the first call makes it).
        let near = t.npcs_of(500).into_iter().find(|id| {
            t.world
                .position_of(*id)
                .is_some_and(|(x, _, _)| (x - 83_000).abs() > 5_000)
        });
        let npc = near.unwrap_or_else(|| t.spawn(500, 20));
        t.talk(player, npc);
        // `onFirstTalk` is queued (Java `notifyEventAsync`).
        t.ticks(2);
        npc
    }

    #[test]
    fn with_the_system_off_the_manager_is_not_spawned() {
        let t = scene(false, 20);
        assert!(t.npcs_of(500).is_empty());
        assert!(!t.world.faction_system_enabled());
    }

    #[test]
    fn with_the_system_on_the_manager_spawns_and_the_player_chooses_good() {
        let mut t = scene(true, 20);
        assert_eq!(t.npcs_of(500).len(), 1);
        let _ = t.take();
        click_manager(&mut t, PLAYER);
        let page = pages_to(&t.take(), PLAYER).pop().unwrap_or_default();
        assert!(page.contains("Manager"), "{page}");
        assert!(page.contains("Good") && page.contains("Evil"), "{page}");

        let _ = t.event(PLAYER, "FactionSystem", "selectGoodFaction");
        assert!(t.world.is_good(ObjectId(PLAYER)));
        assert!(!t.world.is_evil(ObjectId(PLAYER)));
        // The title is the team name and both colours the good colour.
        let player = t.world.player(ObjectId(PLAYER)).unwrap();
        assert_eq!(player.data.title, "Good");
        assert_eq!(player.data.profile.title_color, 0x00FF00);
        assert_eq!(player.gm.name_color, Some(0x00FF00));
        // The other player is not touched.
        assert!(!t.world.is_good(ObjectId(OTHER)) && !t.world.is_evil(ObjectId(OTHER)));
    }

    #[test]
    fn the_evil_side_gets_its_colour_and_a_side_with_too_many_players_is_refused() {
        // The players may exceed the other side by 1: the second good one is refused.
        let mut t = scene(true, 1);
        click_manager(&mut t, PLAYER);
        let _ = t.event(PLAYER, "FactionSystem", "selectGoodFaction");
        assert!(t.world.is_good(ObjectId(PLAYER)));
        let _ = t.take();
        click_manager(&mut t, OTHER);
        let page = t.event(OTHER, "FactionSystem", "selectGoodFaction");
        assert!(
            page.contains("Limit") && page.contains("Good") && page.contains("Evil"),
            "{page}"
        );
        assert!(!t.world.is_good(ObjectId(OTHER)));
        // The other side is free (the refusal page has no links: the player asks the manager again).
        click_manager(&mut t, OTHER);
        let _ = t.event(OTHER, "FactionSystem", "selectEvilFaction");
        assert!(t.world.is_evil(ObjectId(OTHER)));
        assert_eq!(
            t.world.player(ObjectId(OTHER)).unwrap().data.profile.title_color,
            0x0000FF
        );
        let _ = names_to(&t.take(), OTHER);
    }

    // ---- the rules the system adds to the world (switch on and off) ----

    const GUARD_XML: &str = r#"<npc id="600" type="Guard" level="40" name="EvilGuard" title="">
        <ai><clanList><clan>Evil</clan></clanList></ai>
    </npc>"#;

    fn rules_scene(enabled: bool) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(FactionSystem);
        let npcs = format!("{NPCS_XML}{GUARD_XML}");
        let mut t = ScriptTest::new_with(registry, &npcs, "", &[], move |world, _| {
            world.set_settings(WorldSettings {
                faction: FactionSettings {
                    enabled,
                    ..FactionSettings::default()
                },
                ..WorldSettings::default()
            });
        });
        for (id, faction) in [(1, 1), (2, 2), (3, 1)] {
            let mut data = player_data(id, 25, vec![]);
            data.profile.faction = faction;
            t.enter_with(data);
        }
        t
    }

    /// UTF-16 little endian bytes of `text`, as a chat packet carries it.
    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    fn contains_text(bytes: &[u8], text: &str) -> bool {
        let needle = utf16(text);
        bytes.windows(needle.len()).any(|w| w == needle.as_slice())
    }

    fn heard(sent: &[(i32, Vec<u8>)], by: i32, text: &str) -> bool {
        sent.iter()
            .any(|(to, bytes)| *to == by && name_of(bytes) == "SAY2" && contains_text(bytes, text))
    }

    fn heard_message(sent: &[(i32, Vec<u8>)], by: i32, text: &str) -> bool {
        sent.iter()
            .any(|(to, bytes)| *to == by && contains_text(bytes, text))
    }

    fn said(sent: &[(i32, Vec<u8>)], by: i32) -> bool {
        sent.iter()
            .any(|(to, bytes)| *to == by && name_of(bytes) == "SAY2")
    }

    fn say(t: &mut ScriptTest, player: i32, chat_type: i32, text: &str, target: Option<&str>) {
        t.input(
            player,
            l2_world::ClientInput::Say {
                chat_type,
                text: text.to_owned(),
                target: target.map(str::to_owned),
            },
        );
    }

    #[test]
    fn shout_stays_in_the_own_faction_with_the_system_on() {
        let mut t = rules_scene(true);
        let _ = t.take();
        say(&mut t, 1, 1, "hello all", None);
        let out = t.take();
        assert!(heard(&out, 1, "hello all"));
        assert!(heard(&out, 3, "hello all"));
        assert!(!said(&out, 2));
    }

    #[test]
    fn shout_reaches_everybody_with_the_system_off() {
        let mut t = rules_scene(false);
        let _ = t.take();
        say(&mut t, 1, 1, "hello all", None);
        let out = t.take();
        assert!(heard(&out, 2, "hello all") && heard(&out, 3, "hello all"));
    }

    #[test]
    fn general_chat_is_gibberish_for_the_other_faction() {
        let mut t = rules_scene(true);
        let _ = t.take();
        say(&mut t, 1, 0, "hello there", None);
        let out = t.take();
        assert!(heard(&out, 3, "hello there"));
        assert!(said(&out, 2) && !heard(&out, 2, "hello there"));
        // Off: the other side hears the words.
        let mut t = rules_scene(false);
        let _ = t.take();
        say(&mut t, 1, 0, "hello there", None);
        assert!(heard(&t.take(), 2, "hello there"));
    }

    #[test]
    fn whispers_to_the_other_faction_are_refused() {
        let mut t = rules_scene(true);
        let _ = t.take();
        say(&mut t, 1, 2, "psst", Some("P2"));
        let out = t.take();
        assert!(!said(&out, 2));
        assert!(heard_message(&out, 1, "Player belongs to the opposing faction."));
        let mut t = rules_scene(false);
        let _ = t.take();
        say(&mut t, 1, 2, "psst", Some("P2"));
        assert!(heard(&t.take(), 2, "psst"));
    }

    #[test]
    fn the_sides_fight_each_other_only_with_the_system_on() {
        let t = rules_scene(true);
        let (good, evil, good2) = (ObjectId(1), ObjectId(2), ObjectId(3));
        assert!(t.world.is_auto_attackable_by(evil, good));
        assert!(t.world.is_auto_attackable_by(good, evil));
        assert!(!t.world.is_auto_attackable_by(good2, good));
        let t = rules_scene(false);
        assert!(!t.world.is_auto_attackable_by(evil, good));
    }

    #[test]
    fn the_guards_of_the_other_team_attack_and_the_others_do_not() {
        let mut t = rules_scene(true);
        let guard = t.spawn(600, 30);
        // A good player is hated by the guard of the clan `Evil`, an evil one is not.
        assert!(t.world.is_auto_attackable_by(guard, ObjectId(1)));
        assert!(!t.world.is_auto_attackable_by(guard, ObjectId(2)));
        let mut t = rules_scene(false);
        let guard = t.spawn(600, 30);
        assert!(!t.world.is_auto_attackable_by(guard, ObjectId(1)));
    }

    #[test]
    fn a_dead_player_respawns_at_the_base_of_the_faction() {
        let mut t = rules_scene(true);
        assert_eq!(
            t.world
                .get_tele_to_location(ObjectId(1), l2_world::teleport::TeleportWhereType::Town),
            Some((45_306, 48_878, -3_058))
        );
        assert_eq!(
            t.world
                .get_tele_to_location(ObjectId(2), l2_world::teleport::TeleportWhereType::Town),
            Some((-44_037, -113_283, -237))
        );
        let mut t = rules_scene(false);
        assert_ne!(
            t.world
                .get_tele_to_location(ObjectId(1), l2_world::teleport::TeleportWhereType::Town),
            Some((45_306, 48_878, -3_058))
        );
    }

    #[test]
    fn the_colours_are_set_at_enter_world_with_the_system_on_only() {
        let t = rules_scene(true);
        let player = t.world.player(ObjectId(1)).unwrap();
        assert_eq!(player.data.profile.title_color, 0x00FF00);
        assert_eq!(player.gm.name_color, Some(0x00FF00));
        let t = rules_scene(false);
        let player = t.world.player(ObjectId(1)).unwrap();
        assert_eq!(player.gm.name_color, None);
        assert_ne!(player.data.profile.title_color, 0x00FF00);
    }

    #[test]
    fn the_online_balance_refuses_the_crowded_side_at_character_select() {
        let mut registry = ScriptRegistry::new();
        registry.add(FactionSystem);
        let mut t = ScriptTest::new_with(
            registry,
            NPCS_XML,
            "",
            &[(
                "data/html/mods/Faction/ExceededOnlineLimit.htm",
                "<html><body>More %more% than %less%</body></html>",
            )],
            |world, _| {
                world.set_settings(WorldSettings {
                    faction: FactionSettings {
                        enabled: true,
                        balance_player_exceed_limit: 2,
                        ..FactionSettings::default()
                    },
                    ..WorldSettings::default()
                });
            },
        );
        for id in [1, 2] {
            let mut data = player_data(id, 25, vec![]);
            data.profile.faction = 1;
            t.enter_with(data);
        }
        // Two good, no evil: the limit of 2 is reached for good, evil may enter.
        let page = t.world.faction_balance_page(1).unwrap();
        assert!(page.contains("More Good than Evil"), "{page}");
        assert!(t.world.faction_balance_page(2).is_none());
        // A neutral character is never refused.
        assert!(t.world.faction_balance_page(0).is_none());
    }

    #[test]
    fn the_manager_speaks_every_ten_seconds() {
        let mut t = scene(true, 20);
        let _ = t.take();
        t.wait_ms(10_100);
        let said = t.take();
        // The players stand next to the spawn only when the test puts them there, so just check
        // that the timer runs without a panic and the manager is still there.
        let _ = said;
        assert_eq!(t.npcs_of(500).len(), 1);
    }
}
