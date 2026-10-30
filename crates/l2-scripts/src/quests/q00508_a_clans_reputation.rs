//! Quest 508: A Clan's Reputation (Sir Eric Rodemai in the Town of Aden, six raid bosses).
// Java: dist/game/data/scripts/quests/Q00508_AClansReputation/Q00508_AClansReputation.java
//
// Pattern: the same shape as Q00509 (a clan quest: the leader picks a raid boss, a radar points
// to it, the boss's item is brought back for the clan's reputation). The level gate is 5.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::RadarControl;
use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const SIR_ERIC_RODEMAI: i32 = 30868;

// Raid bosses (`addKillId`)
const RAID_BOSS: [i32; 6] = [25252, 25478, 25255, 25245, 25051, 25524];

// Misc
const MIN_CLAN_LEVEL: i32 = 5;

/// Java `REWARD_POINTS`: for `raid` 1..6, (boss, item to bring, reputation points).
fn reward_points(raid: i32) -> Option<(i32, i32, i32)> {
    match raid {
        // Palibati Queen Themis
        1 => Some((25252, 8277, 560)),
        // Shilen's Priest Hisilrome
        2 => Some((25478, 14883, 584)),
        // Gargoyle Lord Tiphon
        3 => Some((25255, 8280, 602)),
        // Last Lesser Giant Glaki
        4 => Some((25245, 8281, 784)),
        // Rahha
        5 => Some((25051, 8282, 558)),
        // Flamestone Giant
        6 => Some((25524, 8494, 768)),
        _ => None,
    }
}

/// Java `player.sendPacket(new RadarControl(0, 2, x, y, z))`: a marker shown at once, not kept
/// in the player's radar list.
fn show_radar(ctx: &ScriptCtx<'_>, player: ObjectId, x: i32, y: i32, z: i32) {
    ctx.world().send(
        player,
        &RadarControl {
            show_radar: 0,
            radar_type: 2,
            x,
            y,
            z,
        },
    );
}

/// Java `Q00508_AClansReputation`.
pub struct Q00508AClansReputation;

impl Script for Q00508AClansReputation {
    fn name(&self) -> &'static str {
        "Q00508_AClansReputation"
    }

    fn path(&self) -> &'static str {
        "quests/Q00508_AClansReputation"
    }

    fn id(&self) -> i32 {
        508
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SIR_ERIC_RODEMAI]);
        s.add_talk_id(&[SIR_ERIC_RODEMAI]);
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
            "30868-0.html" => qs.start_quest(ctx),
            "30868-1.html" => {
                qs.set(ctx, "raid", "1");
                show_radar(ctx, player, 192_376, 22_087, -3608);
            }
            "30868-2.html" => {
                qs.set(ctx, "raid", "2");
                show_radar(ctx, player, 168_288, 28_368, -3632);
            }
            "30868-3.html" => {
                qs.set(ctx, "raid", "3");
                show_radar(ctx, player, 170_048, -24_896, -3440);
            }
            "30868-4.html" => {
                qs.set(ctx, "raid", "4");
                show_radar(ctx, player, 188_809, 47_780, -5968);
            }
            "30868-5.html" => {
                qs.set(ctx, "raid", "5");
                show_radar(ctx, player, 117_760, -9072, -3264);
            }
            "30868-6.html" => {
                qs.set(ctx, "raid", "6");
                show_radar(ctx, player, 144_600, -5500, -4100);
            }
            "30868-7.html" => qs.exit_quest(ctx, true, true),
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
                htmltext = if clan_ok { "30868-0b.htm" } else { "30868-0a.htm" }.to_owned();
            }
            STARTED => {
                if clan_level.is_none() || !ctx.is_clan_leader(player) {
                    qs.exit_quest(ctx, true, false);
                    return Some("30868-8.html".to_owned());
                }
                let raid = qs.get_int(ctx, "raid");
                if let Some((_, item, rep)) = reward_points(raid) {
                    if ctx.has_quest_items(player, item) {
                        htmltext = format!("30868-{raid}b.html");
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
                        htmltext = format!("30868-{raid}a.html");
                    }
                } else {
                    htmltext = "30868-0.html".to_owned();
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

    const NAME: &str = "Q00508_AClansReputation";

    const NPCS: &str = r#"
        <npc id="30868" type="Folk" level="70" name="Sir Eric Rodemai"/>
        <npc id="25252" type="Monster" level="75" name="Palibati Queen Themis"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8277" type="EtcItem" name="Themis's Tongue"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00508AClansReputation);
        let pages = Pages::of(&Q00508AClansReputation)
            .plain(&[
                "30868-0a.htm",
                "30868-0b.htm",
                "30868-0.html",
                "30868-1.html",
                "30868-8.html",
            ])
            .npcs(&[SIR_ERIC_RODEMAI]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let eric = t.spawn(SIR_ERIC_RODEMAI, 20);
        (t, eric)
    }

    // The test world has no clan table: the clan branches with a level 5 clan led by the player
    // are not reachable. The state of the quest is driven through its events.

    #[test]
    fn a_player_without_a_clan_is_sent_away() {
        let (mut t, eric) = setup();
        t.enter(1, 60, vec![]);
        assert!(t.quest_page(1, eric).contains("30868-0a.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_raid_is_chosen_with_a_radar_and_a_started_quest_without_a_clan_ends() {
        let (mut t, eric) = setup();
        t.enter(1, 60, vec![]);
        t.quest_page(1, eric);
        t.take_writes();
        t.take();

        t.world.run_as_script(NAME, |ctx| {
            Q00508AClansReputation.on_event(ctx, "30868-0.html", None, Some(Oid(1)));
        });
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        t.world.run_as_script(NAME, |ctx| {
            Q00508AClansReputation.on_event(ctx, "30868-1.html", None, Some(Oid(1)));
        });
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"RADAR_CONTROL".to_owned()));
        assert_eq!(t.take_writes(), [rows::insert(NAME, "raid", "1")]);

        // Started without a clan: the NPC ends the quest without a sound.
        assert!(t.quest_page(1, eric).contains("30868-8.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_kill_without_a_clan_gives_nothing() {
        let (mut t, _eric) = setup();
        t.enter(1, 60, vec![]);
        let boss = t.spawn(25252, 60);
        t.kill(1, boss);
        assert_eq!(t.count(1, 8277), 0);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_reward_points_follow_the_raid() {
        assert_eq!(reward_points(2), Some((25478, 14883, 584)));
        assert_eq!(reward_points(6), Some((25524, 8494, 768)));
        assert_eq!(reward_points(7), None);
    }
}
