//! Quest 504: Competition for the Bandit Stronghold (Messenger, the clan hall siege of the Bandit Stronghold).
// Java: dist/game/data/scripts/quests/Q00504_CompetitionForTheBanditStronghold/Q00504_CompetitionForTheBanditStronghold.java
//
// Pattern: a kill-count quest (`on_kill` gives the amulet, cond 2 at 30) with a repeatable end and
// a start that depends on the siege of a clan hall (`CHSiegeManager`, hall 35).

use l2_util::ObjectId;
use l2_world::residence::ClanHallId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const MESSENGER: i32 = 35437;

// Items
const TARLK_AMULET: i32 = 4332;
const TROPHY_OF_ALLIANCE: i32 = 5009;

// Monsters
const MOBS: [i32; 5] = [20570, 20571, 20572, 20573, 20574];

// Misc
/// Java `CHSiegeManager.getInstance().getSiegableHall(35)`: the Bandit Stronghold.
const BANDIT_STRONGHOLD: ClanHallId = ClanHallId(35);
/// Java `getSiege().getAttackers().size() >= 5`: five clans are allowed into the siege.
const MAX_ATTACKERS: usize = 5;
const AMULETS_NEEDED: i64 = 30;

/// Java `Q00504_CompetitionForTheBanditStronghold`.
pub struct Q00504CompetitionForTheBanditStronghold;

impl Script for Q00504CompetitionForTheBanditStronghold {
    fn name(&self) -> &'static str {
        "Q00504_CompetitionForTheBanditStronghold"
    }

    fn path(&self) -> &'static str {
        "quests/Q00504_CompetitionForTheBanditStronghold"
    }

    fn id(&self) -> i32 {
        504
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MESSENGER]);
        s.add_talk_id(&[MESSENGER]);
        s.add_kill_id(&MOBS);
        s.register_quest_items(&[TARLK_AMULET]);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = ctx.get_no_quest_msg(player);
        let qs = ctx.get_quest_state(player, true)?;
        match qs.state(ctx) {
            CREATED => {
                if ctx.world().hall_siege_attacker_clans(BANDIT_STRONGHOLD).len() >= MAX_ATTACKERS {
                    htmltext = "35437-00.htm".to_owned();
                } else {
                    htmltext = "35437-01.htm".to_owned();
                    qs.start_quest(ctx);
                }
            }
            STARTED => {
                if ctx.get_quest_items_count(player, TARLK_AMULET) < AMULETS_NEEDED {
                    htmltext = "35437-02.htm".to_owned();
                } else {
                    ctx.take_items(player, TARLK_AMULET, AMULETS_NEEDED);
                    ctx.reward_items(player, TROPHY_OF_ALLIANCE, 1);
                    qs.exit_quest(ctx, true, false);
                    htmltext = "35437-03.htm".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_started(ctx) && qs.is_cond(ctx, 1) {
            ctx.give_items(killer, TARLK_AMULET, 1);
            if ctx.get_quest_items_count(killer, TARLK_AMULET) < AMULETS_NEEDED {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
            } else {
                qs.set_cond(ctx, 2, true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00504_CompetitionForTheBanditStronghold";

    const NPCS: &str = r#"
        <npc id="35437" type="Folk" level="70" name="Messenger"/>
        <npc id="20570" type="Monster" level="30" name="Tarlk Bugbear"/>
        <npc id="20574" type="Monster" level="30" name="Tarlk Basilisk"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4332" type="EtcItem" name="Tarlk Amulet"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5009" type="EtcItem" name="Trophy of Alliance"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00504CompetitionForTheBanditStronghold);
        let pages = Pages::of(&Q00504CompetitionForTheBanditStronghold)
            .plain(&["35437-00.htm", "35437-01.htm", "35437-02.htm", "35437-03.htm"])
            .npcs(&[MESSENGER]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let messenger = t.spawn(MESSENGER, 20);
        (t, messenger)
    }

    fn kill_bugbears(t: &mut ScriptTest, count: usize) {
        for _ in 0..count {
            let mob = t.spawn(20570, 60);
            t.kill(1, mob);
        }
    }

    #[test]
    fn the_messenger_starts_the_quest() {
        let (mut t, messenger) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, messenger).contains("35437-01.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn thirty_amulets_then_the_trophy_ends_the_quest_repeatable() {
        let (mut t, messenger) = setup();
        t.enter(1, 20, vec![]);
        t.quest_page(1, messenger);
        t.take_writes();
        t.take();

        // Before 30 amulets: the messenger sends the player back.
        assert!(t.quest_page(1, messenger).contains("35437-02.htm"));
        kill_bugbears(&mut t, 29);
        assert_eq!(t.count(1, TARLK_AMULET), 29);
        assert!(t.quest_page(1, messenger).contains("35437-02.htm"));
        assert!(t.take_writes().is_empty());

        // The 30th amulet sets cond 2 (`setCond(2, true)`).
        kill_bugbears(&mut t, 1);
        assert_eq!(t.count(1, TARLK_AMULET), 30);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The trophy, the amulets are taken, `exitQuest(true)` (repeatable, no sound).
        assert!(t.quest_page(1, messenger).contains("35437-03.htm"));
        assert_eq!((t.count(1, TARLK_AMULET), t.count(1, TROPHY_OF_ALLIANCE)), (0, 1));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_kill_after_thirty_does_not_give_more_amulets() {
        let (mut t, messenger) = setup();
        t.enter(1, 20, vec![]);
        t.quest_page(1, messenger);
        kill_bugbears(&mut t, 30);
        t.take();
        // The other four mobs of the list count too (`addKillId` for all five).
        let other = t.spawn(20574, 60);
        t.kill(1, other);
        assert_eq!(t.count(1, TARLK_AMULET), 30);
        let out = t.take();
        assert!(!names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
    }
}
