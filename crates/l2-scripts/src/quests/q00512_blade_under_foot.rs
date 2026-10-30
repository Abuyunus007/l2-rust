//! Quest 512: Blade Under Foot (the castle wardens, Fragments of the Dungeon Leader's Mark).
// Java: dist/game/data/scripts/quests/Q00512_BladeUnderFoot/Q00512_BladeUnderFoot.java
//
// Pattern: a kill quest with a castle gate. Only the lord of the castle (the clan leader whose
// clan owns the warden's castle) starts it. Each raid boss drops marks (the whole first chance
// alone, the second chance split among the party members in range); the warden turns them into
// knight's epaulettes.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs: the castle wardens, Gludio to Schuttgart.
const WARDEN: [i32; 9] = [36403, 36404, 36405, 36406, 36407, 36408, 36409, 36410, 36411];

// Misc
const MIN_LEVEL: i32 = 70;

// Item
const FRAGMENT_OF_THE_DUNGEON_LEADER_MARK: i32 = 9798;

// Reward
const KNIGHTS_EPAULETTE: i32 = 9912;

/// Java `DropInfo`: the mark count of a kill alone, and of a party kill (split by the members).
#[derive(Clone, Copy)]
struct DropInfo {
    first_chance: i64,
    second_chance: i64,
}

/// Java `RAID_BOSSES`.
fn raid_boss(npc_id: i32) -> Option<DropInfo> {
    match npc_id {
        // Beautiful Atrielle
        25563 => Some(DropInfo {
            first_chance: 175,
            second_chance: 1443,
        }),
        // Nagen the Tomboy
        25566 => Some(DropInfo {
            first_chance: 176,
            second_chance: 1447,
        }),
        // Jax the Destroyer
        25569 => Some(DropInfo {
            first_chance: 177,
            second_chance: 1450,
        }),
        _ => None,
    }
}

/// Java `npc.getCastle().getResidenceId() == player.getClan().getCastleId() && castleId > 0`:
/// the clan owns the castle of the NPC (`false` without a clan).
fn clan_owns_npc_castle(ctx: &ScriptCtx<'_>, npc: ObjectId, player: ObjectId) -> bool {
    let Some(clan) = ctx.world().clan_of(player) else {
        return false;
    };
    let npc_castle = ctx.world().npc_castle(npc).map(|c| c.0);
    npc_castle == Some(clan.castle_id) && clan.castle_id > 0
}

/// Java `Q00512_BladeUnderFoot`.
pub struct Q00512BladeUnderFoot;

impl Script for Q00512BladeUnderFoot {
    fn name(&self) -> &'static str {
        "Q00512_BladeUnderFoot"
    }

    fn path(&self) -> &'static str {
        "quests/Q00512_BladeUnderFoot"
    }

    fn id(&self) -> i32 {
        512
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&WARDEN);
        s.add_talk_id(&WARDEN);
        s.add_kill_id(&[25563, 25566, 25569]);
        s.register_quest_items(&[FRAGMENT_OF_THE_DUNGEON_LEADER_MARK]);
    }

    fn action_for_each_player(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        _is_summon: bool,
    ) {
        if ctx.get_quest_state(player, false).is_some()
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false)
        {
            // Java `player.getParty().getMemberCount()`: `player` is a member of the killer's party.
            let player_count = ctx.world().party_members_of(player).len() as i64;
            let mut item_count = raid_boss(npc.id).map_or(0, |d| d.second_chance);
            if player_count > 0 {
                item_count /= player_count;
            }
            ctx.give_items(player, FRAGMENT_OF_THE_DUNGEON_LEADER_MARK, item_count);
            ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
        }
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
            "36403-02.htm" => {
                // Java calls `npc.isMyLord(player)` with the NPC of the bypass.
                if let Some(npc) = npc
                    && ctx.player_level(player) >= MIN_LEVEL
                {
                    if ctx.is_my_lord(npc, player) || clan_owns_npc_castle(ctx, npc.object_id, player) {
                        qs.start_quest(ctx);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("36403-03.htm".to_owned());
                    }
                }
            }
            "36403-04.html" | "36403-05.html" | "36403-06.html" | "36403-07.html" | "36403-10.html" => {
                htmltext = Some(event.to_owned());
            }
            "36403-11.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {
        if ctx.get_quest_state(killer, false).is_some() {
            if ctx.is_in_party(killer) {
                ctx.execute_for_each_player(killer, npc, is_summon, true, false);
            } else {
                let first = raid_boss(npc.id).map_or(0, |d| d.first_chance);
                ctx.give_items(killer, FRAGMENT_OF_THE_DUNGEON_LEADER_MARK, first);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    htmltext =
                        if ctx.is_my_lord(npc, player) || clan_owns_npc_castle(ctx, npc.object_id, player) {
                            "36403-01.htm"
                        } else {
                            "36403-03.htm"
                        }
                        .to_owned();
                } else {
                    htmltext = "36403-08.htm".to_owned();
                }
            }
            STARTED => {
                if ctx.has_quest_items(player, FRAGMENT_OF_THE_DUNGEON_LEADER_MARK) {
                    let marks = ctx.get_quest_items_count(player, FRAGMENT_OF_THE_DUNGEON_LEADER_MARK);
                    ctx.give_items(player, KNIGHTS_EPAULETTE, marks);
                    ctx.take_items(player, FRAGMENT_OF_THE_DUNGEON_LEADER_MARK, -1);
                    htmltext = "36403-09.html".to_owned();
                } else {
                    htmltext = "36403-12.html".to_owned();
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

    const NAME: &str = "Q00512_BladeUnderFoot";

    const NPCS: &str = r#"
        <npc id="36403" type="Folk" level="70" name="Gludio Warden"/>
        <npc id="25563" type="Monster" level="75" name="Beautiful Atrielle"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9798" type="EtcItem" name="Fragment of the Dungeon Leader's Mark"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="9912" type="EtcItem" name="Knight's Epaulette"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00512BladeUnderFoot);
        let pages = Pages::of(&Q00512BladeUnderFoot)
            .page("36403-01.htm", &["36403-02.htm"])
            .plain(&["36403-03.htm", "36403-08.htm", "36403-12.html"])
            .page("36403-02.htm", &["36403-04.html", "36403-05.html"])
            .page(
                "36403-09.html",
                &["36403-10.html", "36403-11.html", "36403-05.html"],
            )
            .plain(&["36403-04.html", "36403-05.html", "36403-10.html", "36403-11.html"])
            .npcs(&[36403]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let warden = t.spawn(36403, 20);
        (t, warden)
    }

    /// The quest started as if the castle lord had talked (the test world has no castle table,
    /// so the start event cannot pass the lord check here).
    fn start_as_lord(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.start_quest(ctx);
        });
    }

    // The test world has no castles: the lord branches are not reachable, only the refusals.

    #[test]
    fn a_level_69_character_is_told_so() {
        let (mut t, warden) = setup();
        t.enter(1, 69, vec![]);
        assert!(t.quest_page(1, warden).contains("36403-08.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn someone_without_the_castle_is_refused() {
        let (mut t, warden) = setup();
        t.enter(1, 70, vec![]);
        assert!(t.quest_page(1, warden).contains("36403-03.htm"));
        assert!(t.take_writes().is_empty());
        // The start event answers with the refusal page and starts nothing.
        t.world.run_as_script(NAME, |ctx| {
            assert_eq!(
                Q00512BladeUnderFoot.on_event(
                    ctx,
                    "36403-02.htm",
                    Some(ScriptNpc {
                        object_id: warden,
                        id: 36403,
                    }),
                    Some(Oid(1))
                ),
                Some("36403-03.htm".to_owned())
            );
        });
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_kill_alone_gives_the_whole_first_chance_then_the_epaulettes() {
        let (mut t, warden) = setup();
        t.enter(1, 70, vec![]);
        t.quest_page(1, warden);
        start_as_lord(&mut t, 1);
        t.take_writes();
        t.take();

        let boss = t.spawn(25563, 60);
        t.kill(1, boss);
        assert_eq!(t.count(1, FRAGMENT_OF_THE_DUNGEON_LEADER_MARK), 175);
        assert!(names_to(&t.take(), 1).contains(&"PLAY_SOUND".to_owned()));
        assert!(t.take_writes().is_empty());

        // The warden: the epaulettes for the marks, the marks are taken.
        assert!(t.quest_page(1, warden).contains("36403-09.html"));
        assert_eq!(
            (
                t.count(1, KNIGHTS_EPAULETTE),
                t.count(1, FRAGMENT_OF_THE_DUNGEON_LEADER_MARK)
            ),
            (175, 0)
        );
        assert!(t.quest_page(1, warden).contains("36403-12.html"));
    }

    #[test]
    fn a_party_kill_splits_the_second_chance_between_the_members() {
        let (mut t, warden) = setup();
        t.enter(1, 70, vec![]);
        t.enter(2, 70, vec![]);
        t.quest_page(1, warden);
        start_as_lord(&mut t, 1);
        start_as_lord(&mut t, 2);
        t.party(1, 2);
        t.take();

        let boss = t.spawn(25563, 60);
        t.kill(1, boss);
        // Each member of the party gets the second chance divided by the party size (2).
        assert_eq!(t.count(1, FRAGMENT_OF_THE_DUNGEON_LEADER_MARK), 1443 / 2);
        assert_eq!(t.count(2, FRAGMENT_OF_THE_DUNGEON_LEADER_MARK), 1443 / 2);
    }

    #[test]
    fn the_exit_is_linked_from_the_reward_page() {
        let (mut t, warden) = setup();
        t.enter(1, 70, vec![]);
        t.quest_page(1, warden);
        start_as_lord(&mut t, 1);
        t.take_writes();
        t.take();
        let boss = t.spawn(25563, 60);
        t.kill(1, boss);
        t.quest_page(1, warden);
        t.take_writes();
        assert!(t.event(1, NAME, "36403-11.html").contains("36403-11.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
