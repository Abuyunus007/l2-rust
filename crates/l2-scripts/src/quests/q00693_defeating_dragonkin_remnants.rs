//! Quest 693: Defeating Dragonkin Remnants (Officer Edric in front of the Seed of Destruction;
//! the party registers for the mounted troop battle and is paid in a chest).
// Java: dist/game/data/scripts/quests/Q00693_DefeatingDragonkinRemnants/Q00693_DefeatingDragonkinRemnants.java
//
// Pattern: a quest that only registers the party; `difficulty` and `members` are set by the
// mounted troop instances (`ai/areas/gracia/instances/seed_of_destruction/mounted_troops`).

use l2_protocol_game::SystemMessageId;
use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::gracia::seeds::sod_state;

// NPC
const EDRIC: i32 = 32527;
// Misc
const MIN_LEVEL: i32 = 75;
const ENTER_TELEPORT_LOC: (i32, i32, i32) = (-242754, 219982, -10011);

/// Java `Q00693_DefeatingDragonkinRemnants`.
pub struct Q00693DefeatingDragonkinRemnants;

impl Script for Q00693DefeatingDragonkinRemnants {
    fn name(&self) -> &'static str {
        "Q00693_DefeatingDragonkinRemnants"
    }

    fn path(&self) -> &'static str {
        "quests/Q00693_DefeatingDragonkinRemnants"
    }

    fn id(&self) -> i32 {
        693
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[EDRIC]);
        s.add_first_talk_id(&[EDRIC]);
        s.add_talk_id(&[EDRIC]);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        if sod_state(ctx) == 2 {
            return Some("32527-00.html".to_owned());
        }
        Some("32527-00a.html".to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = Some(ctx.get_no_quest_msg(talker));
        if npc.id == EDRIC {
            if ctx.player_level(talker) < 75 {
                htmltext = Some("32527-lowlevel.htm".to_owned());
            } else if qs.get_memo_state(ctx) < 1 {
                htmltext = Some("32527-01.htm".to_owned());
            } else if qs.is_started(ctx) && qs.get_memo_state(ctx) >= 1 {
                let party = ctx.world().party_of(talker);
                if qs.get_memo_state(ctx) >= 3 {
                    let difficulty = qs.get_int(ctx, "difficulty");
                    let members = qs.get_int(ctx, "members");
                    if qs.is_memo_state(ctx, 3) && reward_player(ctx, talker, difficulty, members) {
                        htmltext = Some("32527-reward.html".to_owned());
                    } else {
                        htmltext = Some("32527-noreward.html".to_owned());
                    }
                    qs.unset(ctx, "difficulty");
                    qs.unset(ctx, "members");
                    ctx.play_sound(talker, QuestSound::ItemsoundQuestFinish);
                    qs.exit_quest(ctx, true, false);
                } else if qs.is_memo_state(ctx, 2) {
                    htmltext = Some("32527-11.html".to_owned());
                } else if let Some(party) = party {
                    let leader = ctx.world().party_leader(party);
                    if leader != Some(talker) {
                        let name = leader.map(|l| ctx.player_name(l)).unwrap_or_default();
                        htmltext = ctx
                            .get_htm(talker, "32527-noleader.html")
                            .map(|page| page.replace("%leader%", &name));
                    } else {
                        htmltext = Some("32527-06.htm".to_owned());
                        for member in ctx.world().party_members(party) {
                            let ok = ctx
                                .get_quest_state(member, false)
                                .is_some_and(|q| q.is_started(ctx) && q.is_memo_state(ctx, 1));
                            if !ok {
                                let name = ctx.player_name(member);
                                return ctx
                                    .get_htm(talker, "32527-noquest.html")
                                    .map(|page| page.replace("%member%", &name));
                            }
                        }
                    }
                } else {
                    htmltext = Some("32527-noparty.html".to_owned());
                }
            }
        }
        htmltext
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        let mut htmltext = Some(event.to_owned());
        match event {
            "32527-05.htm" => {
                if check_instances(ctx, player) {
                    ctx.play_sound(player, QuestSound::ItemsoundQuestAccept);
                    if ctx.player_level(player) >= MIN_LEVEL {
                        qs.start_quest(ctx);
                        qs.set_memo_state(ctx, 1);
                        ctx.play_sound(player, QuestSound::ItemsoundQuestAccept);
                        htmltext = Some(event.to_owned());
                    }
                } else {
                    htmltext = Some("32527-12.html".to_owned());
                }
            }
            "reenter" => {
                htmltext = Some(String::new());
                if qs.get_int(ctx, "difficulty") >= 1 {
                    let world = ctx
                        .world()
                        .player_instance_world(player)
                        .map(|w| (w.template_id, w.instance_id));
                    if let Some((template_id, instance_id)) = world {
                        if (123..=126).contains(&template_id) {
                            let (x, y, z) = ENTER_TELEPORT_LOC;
                            ctx.teleport_player(player, x, y, z, instance_id, true);
                        } else {
                            ctx.send_system_message(
                                player,
                                SystemMessageId::YouHaveEnteredAnotherInstanceZoneThereforeYouCannotEnterCorrespondingDungeon,
                            );
                        }
                    } else {
                        htmltext = Some("32527-noreward.html".to_owned());
                        qs.unset(ctx, "difficulty");
                        qs.unset(ctx, "members");
                        ctx.play_sound(player, QuestSound::ItemsoundQuestFinish);
                        qs.exit_quest(ctx, true, false);
                    }
                }
            }
            _ => {}
        }
        htmltext
    }
}

fn check_instances(ctx: &ScriptCtx<'_>, talker: ObjectId) -> bool {
    let now = ctx.now_ms();
    !(123..=126).any(|id| now < ctx.world().instance_time(talker, id))
}

fn reward_player(ctx: &mut ScriptCtx<'_>, player: ObjectId, difficulty: i32, member_count: i32) -> bool {
    // NOTE(java-quirk): a `members` of 0 divides by zero in Java (an exception, no reward page);
    // here it is a refusal.
    if member_count == 0 {
        return false;
    }
    if ctx.get_random(1000) < ((10000 / (member_count * 10)) * (1 + (difficulty * 2))) {
        match difficulty {
            4 => ctx.give_items(player, 14638, 1), // Best Quality Battle Reward Chest
            3 => ctx.give_items(player, 14637, 1), // High-Grade Battle Reward Chest
            2 => ctx.give_items(player, 14636, 1), // Middle Quality Battle Reward Chest
            1 => ctx.give_items(player, 14635, 1), // Low Quality Battle Reward Chest
            _ => {}
        }
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00693_DefeatingDragonkinRemnants";
    const DIR: &str = "data/scripts/quests/Q00693_DefeatingDragonkinRemnants";

    const NPCS: &str = r#"
        <npc id="32527" type="Folk" level="1" name="Edric"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14635" type="EtcItem" name="Low Quality Battle Reward Chest"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00693DefeatingDragonkinRemnants);
        let pages = Pages::of(&Q00693DefeatingDragonkinRemnants)
            .page("32527-00a.html", &["32527-05.htm"])
            .plain(&[
                "32527-00.html",
                "32527-01.htm",
                "32527-05.htm",
                "32527-06.htm",
                "32527-11.html",
                "32527-12.html",
                "32527-lowlevel.htm",
                "32527-noparty.html",
                "32527-noreward.html",
                "32527-reward.html",
            ])
            .file(
                &format!("{DIR}/32527-noleader.html"),
                "<html><body>Call %leader% for me.</body></html>",
            )
            .file(
                &format!("{DIR}/32527-noquest.html"),
                "<html><body>%member% has no quest.</body></html>",
            )
            .npcs(&[EDRIC]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let edric = t.spawn(EDRIC, 20);
        (t, edric)
    }

    fn talk(t: &mut ScriptTest, player: i32) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ScriptNpc {
                id: EDRIC,
                object_id: Oid(0),
            };
            out = Q00693DefeatingDragonkinRemnants.on_talk(ctx, npc, Oid(player));
        });
        out
    }

    /// Starts the quest with the given memo state, as the instance leaves it.
    fn started(t: &mut ScriptTest, memo: i32, difficulty: i32, members: i32) {
        started_by(t, 1, memo, difficulty, members);
    }

    fn started_by(t: &mut ScriptTest, player: i32, memo: i32, difficulty: i32, members: i32) {
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.start_quest(ctx);
            qs.set_memo_state(ctx, memo);
            if difficulty > 0 {
                qs.set_int(ctx, "difficulty", difficulty);
                qs.set_int(ctx, "members", members);
            }
        });
        t.take_writes();
    }

    #[test]
    fn the_first_talk_depends_on_the_seed_state() {
        let (mut t, _) = setup();
        t.enter(1, 75, vec![]);
        let mut pages = Vec::new();
        for state in ["1", "2"] {
            t.world.run_as_script(NAME, |ctx| {
                ctx.set_global_variable("SoDState", state);
                let npc = ScriptNpc {
                    id: EDRIC,
                    object_id: Oid(0),
                };
                pages.push(Q00693DefeatingDragonkinRemnants.on_first_talk(ctx, npc, Oid(1)));
            });
        }
        assert_eq!(
            pages,
            [
                Some("32527-00a.html".to_owned()),
                Some("32527-00.html".to_owned())
            ]
        );
    }

    #[test]
    fn a_level_74_character_is_refused() {
        let (mut t, _) = setup();
        t.enter(1, 74, vec![]);
        assert_eq!(talk(&mut t, 1).as_deref(), Some("32527-lowlevel.htm"));
    }

    #[test]
    fn a_level_75_character_registers() {
        let (mut t, edric) = setup();
        t.enter(1, 75, vec![]);
        assert_eq!(talk(&mut t, 1).as_deref(), Some("32527-01.htm"));
        t.take_writes();
        // The first talk page (00a here) stands in for the one that links the registration.
        t.talk(1, edric);
        t.take();
        assert!(t.event(1, NAME, "32527-05.htm").contains("32527-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
    }

    #[test]
    fn a_started_player_without_a_party_is_sent_for_one() {
        let (mut t, _) = setup();
        t.enter(1, 75, vec![]);
        started(&mut t, 1, 0, 0);
        assert_eq!(talk(&mut t, 1).as_deref(), Some("32527-noparty.html"));
    }

    #[test]
    fn the_party_leader_gets_the_difficulty_page() {
        let (mut t, _) = setup();
        t.enter(1, 75, vec![]);
        t.enter(2, 75, vec![]);
        t.party(1, 2);
        started(&mut t, 1, 0, 0);
        // The member has no quest yet: the leader is told who.
        let page = talk(&mut t, 1).unwrap();
        assert!(page.contains("has no quest"), "{page}");
        // The member talks: not the leader.
        started_by(&mut t, 2, 1, 0, 0);
        let page = talk(&mut t, 2).unwrap();
        assert!(page.contains("Call"), "{page}");
    }

    #[test]
    fn the_reward_chest_depends_on_the_difficulty() {
        let (mut t, _) = setup();
        t.enter(1, 75, vec![]);
        // 10000 / 10 * 3 = 3000 > 1000: a sure chest at one member.
        started(&mut t, 3, 1, 1);
        assert_eq!(talk(&mut t, 1).as_deref(), Some("32527-reward.html"));
        assert_eq!(t.count(1, 14635), 1);
        assert!(t.take_writes().contains(&rows::delete(NAME, true)));
    }

    #[test]
    fn a_battle_in_progress_has_its_own_page() {
        let (mut t, _) = setup();
        t.enter(1, 75, vec![]);
        started(&mut t, 2, 1, 1);
        assert_eq!(talk(&mut t, 1).as_deref(), Some("32527-11.html"));
    }

    #[test]
    fn a_failed_battle_pays_nothing() {
        let (mut t, _) = setup();
        t.enter(1, 75, vec![]);
        started(&mut t, 4, 1, 1);
        assert_eq!(talk(&mut t, 1).as_deref(), Some("32527-noreward.html"));
        assert_eq!(t.count(1, 14635), 0);
        assert!(t.take_writes().contains(&rows::delete(NAME, true)));
    }
}
