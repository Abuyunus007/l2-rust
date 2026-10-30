//! Quest 509: A Clan's Fame (Valdis in the Town of Aden, the raid bosses of the clan).
// Java: dist/game/data/scripts/quests/Q00509_AClansFame/Q00509_AClansFame.java
//
// Pattern: a clan quest like Q00510. The leader picks one of four raid bosses (the quest variable
// `raid`), a radar points to it, and the boss's item (taken to the NPC) gives the clan reputation.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::RadarControl;
use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const VALDIS: i32 = 31331;

// Raid bosses (`addKillId`)
const RAID_BOSS: [i32; 4] = [25290, 25293, 25523, 25322];

// Misc
const MIN_CLAN_LEVEL: i32 = 6;

/// Java `REWARD_POINTS`: for `raid` 1..4, (boss, item to bring, reputation points).
fn reward_points(raid: i32) -> Option<(i32, i32, i32)> {
    match raid {
        // Daimon The White-Eyed
        1 => Some((25290, 8489, 1378)),
        // Hestia, Guardian Deity Of The Hot Springs
        2 => Some((25293, 8490, 1378)),
        // Plague Golem
        3 => Some((25523, 8491, 1070)),
        // Demon's Agent Falston
        4 => Some((25322, 8492, 782)),
        _ => None,
    }
}

/// Java `Q00509_AClansFame`.
pub struct Q00509AClansFame;

impl Script for Q00509AClansFame {
    fn name(&self) -> &'static str {
        "Q00509_AClansFame"
    }

    fn path(&self) -> &'static str {
        "quests/Q00509_AClansFame"
    }

    fn id(&self) -> i32 {
        509
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[VALDIS]);
        s.add_talk_id(&[VALDIS]);
        s.add_kill_id(&RAID_BOSS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };
        match event {
            "31331-0.html" => qs.start_quest(ctx),
            "31331-1.html" => {
                qs.set(ctx, "raid", "1");
                ctx.world().send(
                    player,
                    &RadarControl {
                        show_radar: 0,
                        radar_type: 2,
                        x: 186_304,
                        y: -43_744,
                        z: -3193,
                    },
                );
            }
            "31331-2.html" => {
                qs.set(ctx, "raid", "2");
                ctx.world().send(
                    player,
                    &RadarControl {
                        show_radar: 0,
                        radar_type: 2,
                        x: 134_672,
                        y: -115_600,
                        z: -1216,
                    },
                );
            }
            "31331-3.html" => {
                qs.set(ctx, "raid", "3");
                ctx.world().send(
                    player,
                    &RadarControl {
                        show_radar: 0,
                        radar_type: 2,
                        x: 170_000,
                        y: -60_000,
                        z: -3500,
                    },
                );
            }
            "31331-4.html" => {
                qs.set(ctx, "raid", "4");
                ctx.world().send(
                    player,
                    &RadarControl {
                        show_radar: 0,
                        radar_type: 2,
                        x: 93_296,
                        y: -75_104,
                        z: -1824,
                    },
                );
            }
            "31331-5.html" => qs.exit_quest(ctx, true, true),
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `player.getClan()`: no clan, no item.
        let Some(leader) = ctx.world().clan_of(killer).map(|c| c.leader) else {
            return;
        };
        // The state is the leader's: the killer's when the killer leads, else the leader's when
        // the leader is online and within `AltPartyRange` (3D, strict).
        let owner = if ctx.is_clan_leader(killer) {
            Some(killer)
        } else {
            leader.filter(|&l| {
                ctx.world().player(l).is_some()
                    && ctx
                        .distance_3d(killer, l)
                        .is_some_and(|d| d < f64::from(ctx.alt_party_range()))
            })
        };
        let Some(owner) = owner else {
            return;
        };
        let raid = ctx
            .player_quest_state(owner, self.name())
            .filter(|qs| qs.is_started())
            .map(|qs| qs.get_int("raid"));
        if let Some(raid) = raid
            && let Some((boss, item, _)) = reward_points(raid)
            && npc.id == boss
            && !ctx.has_quest_items(killer, item)
        {
            ctx.reward_items(killer, item, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        // Java `player.getClan()`: `None` without a clan.
        let clan_level = ctx.clan_level_of(player);
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                let clan_ok = clan_level.is_some_and(|l| l >= MIN_CLAN_LEVEL) && ctx.is_clan_leader(player);
                htmltext = if clan_ok { "31331-0b.htm" } else { "31331-0a.htm" }.to_owned();
            }
            STARTED => {
                if clan_level.is_none() || !ctx.is_clan_leader(player) {
                    qs.exit_quest(ctx, true, false);
                    return Some("31331-6.html".to_owned());
                }
                let raid = qs.get_int(ctx, "raid");
                if let Some((_, item, rep)) = reward_points(raid) {
                    if ctx.has_quest_items(player, item) {
                        htmltext = format!("31331-{raid}b.html");
                        ctx.play_sound(player, QuestSound::ItemsoundQuestFanfare1);
                        ctx.take_items(player, item, -1);
                        ctx.add_clan_reputation_score(player, rep);
                        ctx.send_system_message_int(
                            player,
                            SystemMessageId::YouHaveSuccessfullyCompletedAClanQuestS1PointsHaveBeenAddedToYourClanSReputationScore,
                            rep,
                        );
                        // FOLLOW-UP(engine): Java `clan.broadcastToOnlineMembers(new PledgeShowInfoUpdate(clan))`
                        // (`World::broadcast_clan_info_update` is `pub(crate)`).
                    } else {
                        htmltext = format!("31331-{raid}a.html");
                    }
                } else {
                    htmltext = "31331-0.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00509_AClansFame";

    const NPCS: &str = r#"
        <npc id="31331" type="Folk" level="70" name="Valdis"/>
        <npc id="25290" type="Monster" level="75" name="Daimon the White-Eyed"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8489" type="EtcItem" name="Daimon's Head"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00509AClansFame);
        let pages = Pages::of(&Q00509AClansFame)
            .plain(&[
                "31331-0a.htm",
                "31331-0b.htm",
                "31331-0.html",
                "31331-1.html",
                "31331-6.html",
            ])
            .plain(&["31331-1a.html", "31331-1b.html"])
            .npcs(&[VALDIS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let valdis = t.spawn(VALDIS, 20);
        (t, valdis)
    }

    // The test world has no clan table: the clan branches with a level 6 clan led by the player
    // are not reachable. The state of the quest is driven through its events.

    #[test]
    fn a_player_without_a_clan_is_sent_away() {
        let (mut t, valdis) = setup();
        t.enter(1, 60, vec![]);
        assert!(t.quest_page(1, valdis).contains("31331-0a.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_raid_is_chosen_with_a_radar_and_a_started_quest_without_a_clan_ends() {
        let (mut t, valdis) = setup();
        t.enter(1, 60, vec![]);
        t.quest_page(1, valdis);
        t.take_writes();
        t.take();

        t.world.run_as_script(NAME, |ctx| {
            assert!(
                Q00509AClansFame
                    .on_event(ctx, "31331-0.html", None, Some(Oid(1)))
                    .is_some()
            );
        });
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        t.world.run_as_script(NAME, |ctx| {
            Q00509AClansFame.on_event(ctx, "31331-1.html", None, Some(Oid(1)));
        });
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"RADAR_CONTROL".to_owned()));
        assert_eq!(t.take_writes(), [rows::insert(NAME, "raid", "1")]);

        // Started without a clan: the NPC ends the quest without a sound.
        assert!(t.quest_page(1, valdis).contains("31331-6.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_kill_without_a_clan_gives_nothing() {
        let (mut t, _valdis) = setup();
        t.enter(1, 60, vec![]);
        let boss = t.spawn(25290, 60);
        t.kill(1, boss);
        assert_eq!(t.count(1, 8489), 0);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_reward_points_follow_the_raid() {
        assert_eq!(reward_points(1), Some((25290, 8489, 1378)));
        assert_eq!(reward_points(3), Some((25523, 8491, 1070)));
        assert_eq!(reward_points(0), None);
    }
}
