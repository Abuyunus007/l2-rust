//! Quest 10278: Mutated Kaneus - Heine (Gosta sends the party after Blade Otis and Weird Bunei).
// Java: dist/game/data/scripts/quests/Q10278_MutatedKaneusHeine/Q10278_MutatedKaneusHeine.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const GOSTA: i32 = 30916;
const MINEVIA: i32 = 30907;

// Monsters
const BLADE_OTIS: i32 = 18562;
const WEIRD_BUNEI: i32 = 18564;

// Items
const TISSUE_BO: i32 = 13834;
const TISSUE_WB: i32 = 13835;

/// Java `Q10278_MutatedKaneusHeine`.
pub struct Q10278MutatedKaneusHeine;

/// Java `rewardItem(npcId, player)`: one tissue of the monster, once.
fn reward_item(ctx: &mut ScriptCtx<'_>, npc_id: i32, player: ObjectId) {
    if npc_id == BLADE_OTIS && !ctx.has_quest_items(player, TISSUE_BO) {
        ctx.give_items(player, TISSUE_BO, 1);
        ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
    } else if npc_id == WEIRD_BUNEI && !ctx.has_quest_items(player, TISSUE_WB) {
        ctx.give_items(player, TISSUE_WB, 1);
        ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
    }
}

impl Script for Q10278MutatedKaneusHeine {
    fn name(&self) -> &'static str {
        "Q10278_MutatedKaneusHeine"
    }

    fn path(&self) -> &'static str {
        "quests/Q10278_MutatedKaneusHeine"
    }

    fn id(&self) -> i32 {
        10278
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GOSTA]);
        s.add_talk_id(&[GOSTA, MINEVIA]);
        s.add_kill_id(&[BLADE_OTIS, WEIRD_BUNEI]);
        s.register_quest_items(&[TISSUE_BO, TISSUE_WB]);
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
            "30916-03.htm" => qs.start_quest(ctx),
            "30907-03.htm" => {
                ctx.give_adena(player, 50000, true);
                qs.exit_quest(ctx, false, true);
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };

        let npc_id = npc.id;
        if ctx.world().party_of(killer).is_some() {
            // A party member who still lacks this tissue and has the quest gets it; one of them
            // is drawn at random.
            let mut party_members = Vec::new();
            for member in ctx.world().party_members_of(killer) {
                if let Some(member_qs) = ctx.get_quest_state(member, false)
                    && member_qs.is_started(ctx)
                    && ((npc_id == BLADE_OTIS && !ctx.has_quest_items(member, TISSUE_BO))
                        || (npc_id == WEIRD_BUNEI && !ctx.has_quest_items(member, TISSUE_WB)))
                {
                    party_members.push(member);
                }
            }
            if let Some(member) = ctx.get_random_entry(&party_members) {
                reward_item(ctx, npc_id, member);
            }
        } else if qs.is_started(ctx) {
            reward_item(ctx, npc_id, killer);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            GOSTA => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) > 37 {
                        "30916-01.htm"
                    } else {
                        "30916-00.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    htmltext =
                        if ctx.has_quest_items(player, TISSUE_BO) && ctx.has_quest_items(player, TISSUE_WB) {
                            "30916-05.htm"
                        } else {
                            "30916-04.htm"
                        }
                        .to_owned();
                }
                COMPLETED => htmltext = "30916-06.htm".to_owned(),
                _ => {}
            },
            MINEVIA => match qs.state(ctx) {
                STARTED => {
                    htmltext =
                        if ctx.has_quest_items(player, TISSUE_BO) && ctx.has_quest_items(player, TISSUE_WB) {
                            "30907-02.htm"
                        } else {
                            "30907-01.htm"
                        }
                        .to_owned();
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
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

    const NAME: &str = "Q10278_MutatedKaneusHeine";

    const NPCS: &str = r#"
        <npc id="30916" type="Folk" level="50" name="Gosta"/>
        <npc id="30907" type="Folk" level="50" name="Minevia"/>
        <npc id="18562" type="Monster" level="40" name="Blade Otis"/>
        <npc id="18564" type="Monster" level="40" name="Weird Bunei"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13834" type="EtcItem" name="Tissue of Blade Otis"><set name="is_questitem" val="true"/></item>
        <item id="13835" type="EtcItem" name="Tissue of Weird Bunei"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10278MutatedKaneusHeine);
        let pages = Pages::of(&Q10278MutatedKaneusHeine)
            .plain(&["30916-00.htm", "30916-04.htm", "30916-05.htm", "30916-06.htm"])
            .page("30916-01.htm", &["30916-02.htm"])
            .page("30916-02.htm", &["30916-03.htm"])
            .plain(&["30916-03.htm", "30907-01.htm", "30907-03.htm"])
            .page("30907-02.htm", &["30907-03.htm"])
            .npcs(&[GOSTA, MINEVIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let gosta = t.spawn(GOSTA, 20);
        let minevia = t.spawn(MINEVIA, 40);
        (t, gosta, minevia)
    }

    /// Accepts the quest from Gosta (`30916-03.htm`).
    fn accept(t: &mut ScriptTest, player: i32, gosta: Oid) {
        assert!(t.quest_page(player, gosta).contains("30916-01.htm"));
        assert!(t.event(player, NAME, "30916-02.htm").contains("30916-02.htm"));
        assert!(t.event(player, NAME, "30916-03.htm").contains("30916-03.htm"));
    }

    #[test]
    fn a_character_of_level_37_is_sent_away() {
        let (mut t, gosta, _) = setup();
        t.enter(1, 37, vec![]);
        assert!(t.quest_page(1, gosta).contains("30916-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn both_tissues_then_the_reward_from_minevia() {
        let (mut t, gosta, minevia) = setup();
        t.enter(1, 38, vec![]);
        accept(&mut t, 1, gosta);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.quest_page(1, gosta).contains("30916-04.htm"));

        // Blade Otis and Weird Bunei: one tissue each.
        let otis = t.spawn(BLADE_OTIS, 60);
        t.kill(1, otis);
        assert_eq!(t.count(1, TISSUE_BO), 1);
        let bunei = t.spawn(WEIRD_BUNEI, 60);
        t.kill(1, bunei);
        assert_eq!(t.count(1, TISSUE_WB), 1);
        assert!(names_to(&t.take(), 1).contains(&"PLAY_SOUND".to_owned()));

        assert!(t.quest_page(1, gosta).contains("30916-05.htm"));
        assert!(t.quest_page(1, minevia).contains("30907-02.htm"));

        // The reward: 50000 adena and `exitQuest(false, true)`; the tissues go with the quest.
        assert!(t.event(1, NAME, "30907-03.htm").contains("30907-03.htm"));
        assert_eq!(t.count(1, 57), 50000);
        assert_eq!((t.count(1, TISSUE_BO), t.count(1, TISSUE_WB)), (0, 0));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(t.quest_page(1, gosta).contains("30916-06.htm"));
        assert!(
            t.quest_page(1, minevia)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn a_party_kill_gives_the_tissue_to_one_started_member() {
        let (mut t, gosta, _) = setup();
        t.enter(1, 40, vec![]);
        t.enter(2, 40, vec![]);
        accept(&mut t, 1, gosta);
        accept(&mut t, 2, gosta);
        t.party(1, 2);

        let otis = t.spawn(BLADE_OTIS, 60);
        t.kill(1, otis);
        assert_eq!(t.count(1, TISSUE_BO) + t.count(2, TISSUE_BO), 1);
    }
}
