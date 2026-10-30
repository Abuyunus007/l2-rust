//! Quest 510: A Clan's Prestige (Valdis in the Town of Aden, the clan's Tyrannosaurus claws).
// Java: dist/game/data/scripts/quests/Q00510_AClansPrestige/Q00510_AClansPrestige.java
//
// Pattern: a clan quest. Only the clan leader (level 5 or more) can start it; the claws of the
// clan's monsters count for the clan's reputation when the leader is near the killer, and the
// NPC turns them into reputation points at once.

use l2_protocol_game::SystemMessageId;
use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const VALDIS: i32 = 31331;

// Quest item
const TYRANNOSAURUS_CLAW: i32 = 8767;

// Monsters
const MOBS: [i32; 3] = [22215, 22216, 22217];

// Misc
const MIN_CLAN_LEVEL: i32 = 5;

/// Java `Q00510_AClansPrestige`.
pub struct Q00510AClansPrestige;

impl Script for Q00510AClansPrestige {
    fn name(&self) -> &'static str {
        "Q00510_AClansPrestige"
    }

    fn path(&self) -> &'static str {
        "quests/Q00510_AClansPrestige"
    }

    fn id(&self) -> i32 {
        510
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[VALDIS]);
        s.add_talk_id(&[VALDIS]);
        s.add_kill_id(&MOBS);
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
            "31331-3.html" => qs.start_quest(ctx),
            "31331-6.html" => qs.exit_quest(ctx, true, true),
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `player.getClan()`: no clan, no claw.
        let Some(leader) = ctx.world().clan_of(killer).map(|c| c.leader) else {
            return;
        };
        // The quest is the leader's: the killer's own state when the killer leads, else the
        // leader's state when the leader is online and within `AltPartyRange` (3D, strict).
        let qs = if ctx.is_clan_leader(killer) {
            ctx.get_quest_state(killer, false)
        } else {
            match leader {
                Some(leader)
                    if ctx.world().player(leader).is_some()
                        && ctx
                            .distance_3d(killer, leader)
                            .is_some_and(|d| d < f64::from(ctx.alt_party_range())) =>
                {
                    ctx.get_quest_state(leader, false)
                }
                _ => None,
            }
        };
        if let Some(qs) = qs
            && qs.is_started(ctx)
        {
            ctx.reward_items(killer, TYRANNOSAURUS_CLAW, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        // Java `clan == null`: `clan_level_of` is `None` without a clan.
        let has_clan = ctx.clan_level_of(player).is_some();
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                let level_ok = ctx.clan_level_of(player).is_some_and(|l| l >= MIN_CLAN_LEVEL);
                htmltext = if has_clan && ctx.is_clan_leader(player) && level_ok {
                    "31331-1.htm"
                } else {
                    "31331-0.htm"
                }
                .to_owned();
            }
            STARTED => {
                if !has_clan || !ctx.is_clan_leader(player) {
                    qs.exit_quest(ctx, true, false);
                    return Some("31331-8.html".to_owned());
                }
                if !ctx.has_quest_items(player, TYRANNOSAURUS_CLAW) {
                    htmltext = "31331-4.html".to_owned();
                } else {
                    // NOTE(java-quirk): the reward formula jumps from 30 per claw to 59 + 30 per
                    // claw at ten claws; ported as it is.
                    let count = ctx.get_quest_items_count(player, TYRANNOSAURUS_CLAW) as i32;
                    let reward = if count < 10 { 30 * count } else { 59 + 30 * count };
                    ctx.play_sound(player, QuestSound::ItemsoundQuestFanfare1);
                    ctx.take_items(player, TYRANNOSAURUS_CLAW, -1);
                    ctx.add_clan_reputation_score(player, reward);
                    ctx.send_system_message_int(
                        player,
                        SystemMessageId::YouHaveSuccessfullyCompletedAClanQuestS1PointsHaveBeenAddedToYourClanSReputationScore,
                        reward,
                    );
                    // FOLLOW-UP(engine): Java `clan.broadcastToOnlineMembers(new PledgeShowInfoUpdate(clan))`
                    // (`World::broadcast_clan_info_update` is `pub(crate)`), so the clan window
                    // shows the new score only at its next update.
                    htmltext = "31331-7.html".to_owned();
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00510_AClansPrestige";

    const NPCS: &str = r#"
        <npc id="31331" type="Folk" level="70" name="Valdis"/>
        <npc id="22215" type="Monster" level="70" name="Tyrannosaurus"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8767" type="EtcItem" name="Tyrannosaurus Claw"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00510AClansPrestige);
        let pages = Pages::of(&Q00510AClansPrestige)
            .page("31331-1.htm", &["31331-2.htm"])
            .page("31331-2.htm", &["31331-3.html"])
            .plain(&["31331-0.htm", "31331-3.html", "31331-8.html"])
            .page("31331-4.html", &["31331-5.html", "31331-6.html"])
            .page("31331-7.html", &["31331-5.html", "31331-6.html"])
            .plain(&["31331-5.html", "31331-6.html"])
            .npcs(&[VALDIS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let valdis = t.spawn(VALDIS, 20);
        (t, valdis)
    }

    // The test world has no clan table, so the clan branches (a clan of level 5 led by the
    // player) are not reachable here. These tests cover the branches without a clan.

    #[test]
    fn a_player_without_a_clan_is_sent_away() {
        let (mut t, valdis) = setup();
        t.enter(1, 60, vec![]);
        assert!(t.quest_page(1, valdis).contains("31331-0.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_kill_without_a_clan_gives_no_claw() {
        let (mut t, _valdis) = setup();
        t.enter(1, 60, vec![]);
        let mob = t.spawn(22215, 60);
        t.kill(1, mob);
        assert_eq!(t.count(1, TYRANNOSAURUS_CLAW), 0);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_started_quest_without_a_clan_ends_without_a_sound() {
        let (mut t, valdis) = setup();
        t.enter(1, 60, vec![]);
        // A state that is started but whose player has no clan: the NPC ends it at once.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
        });
        t.take_writes();
        t.take();
        assert!(t.quest_page(1, valdis).contains("31331-8.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
