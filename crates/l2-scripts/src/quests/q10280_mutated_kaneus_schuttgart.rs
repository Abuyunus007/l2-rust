//! Quest 10280: Mutated Kaneus - Schuttgart (Vishotsky sends the party after Venomous Storace and Kel Bilette).
// Java: dist/game/data/scripts/quests/Q10280_MutatedKaneusSchuttgart/Q10280_MutatedKaneusSchuttgart.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const VISHOTSKY: i32 = 31981;
const ATRAXIA: i32 = 31972;

// Monsters
const VENOMOUS_STORACE: i32 = 18571;
const KEL_BILETTE: i32 = 18573;

// Items
const TISSUE_VS: i32 = 13838;
const TISSUE_KB: i32 = 13839;

/// Java `Q10280_MutatedKaneusSchuttgart`.
pub struct Q10280MutatedKaneusSchuttgart;

/// Java `rewardItem(npcId, player)`: one tissue of the monster, once.
fn reward_item(ctx: &mut ScriptCtx<'_>, npc_id: i32, player: ObjectId) {
    if npc_id == VENOMOUS_STORACE && !ctx.has_quest_items(player, TISSUE_VS) {
        ctx.give_items(player, TISSUE_VS, 1);
        ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
    } else if npc_id == KEL_BILETTE && !ctx.has_quest_items(player, TISSUE_KB) {
        ctx.give_items(player, TISSUE_KB, 1);
        ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
    }
}

impl Script for Q10280MutatedKaneusSchuttgart {
    fn name(&self) -> &'static str {
        "Q10280_MutatedKaneusSchuttgart"
    }

    fn path(&self) -> &'static str {
        "quests/Q10280_MutatedKaneusSchuttgart"
    }

    fn id(&self) -> i32 {
        10280
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[VISHOTSKY]);
        s.add_talk_id(&[VISHOTSKY, ATRAXIA]);
        s.add_kill_id(&[VENOMOUS_STORACE, KEL_BILETTE]);
        s.register_quest_items(&[TISSUE_VS, TISSUE_KB]);
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
            "31981-03.htm" => qs.start_quest(ctx),
            "31972-03.htm" => {
                ctx.give_adena(player, 210_000, true);
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
                    && ((npc_id == VENOMOUS_STORACE && !ctx.has_quest_items(member, TISSUE_VS))
                        || (npc_id == KEL_BILETTE && !ctx.has_quest_items(member, TISSUE_KB)))
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
            VISHOTSKY => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) > 57 {
                        "31981-01.htm"
                    } else {
                        "31981-00.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    htmltext = if ctx.has_quest_items(player, TISSUE_VS)
                        && ctx.has_quest_items(player, TISSUE_KB)
                    {
                        "31981-05.htm"
                    } else {
                        "31981-04.htm"
                    }
                    .to_owned();
                }
                COMPLETED => htmltext = "31981-06.htm".to_owned(),
                _ => {}
            },
            ATRAXIA => match qs.state(ctx) {
                STARTED => {
                    htmltext = if ctx.has_quest_items(player, TISSUE_VS)
                        && ctx.has_quest_items(player, TISSUE_KB)
                    {
                        "31972-02.htm"
                    } else {
                        "31972-01.htm"
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

    const NAME: &str = "Q10280_MutatedKaneusSchuttgart";

    const NPCS: &str = r#"
        <npc id="31981" type="Folk" level="50" name="Vishotsky"/>
        <npc id="31972" type="Folk" level="50" name="Atraxia"/>
        <npc id="18571" type="Monster" level="40" name="Venomous Storace"/>
        <npc id="18573" type="Monster" level="40" name="Kel Bilette"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13838" type="EtcItem" name="Tissue of Venomous Storace"><set name="is_questitem" val="true"/></item>
        <item id="13839" type="EtcItem" name="Tissue of Kel Bilette"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10280MutatedKaneusSchuttgart);
        let pages = Pages::of(&Q10280MutatedKaneusSchuttgart)
            .plain(&["31981-00.htm", "31981-04.htm", "31981-05.htm", "31981-06.htm"])
            .page("31981-01.htm", &["31981-02.htm"])
            .page("31981-02.htm", &["31981-03.htm"])
            .plain(&["31981-03.htm", "31972-01.htm", "31972-03.htm"])
            .page("31972-02.htm", &["31972-03.htm"])
            .npcs(&[VISHOTSKY, ATRAXIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let vishotsky = t.spawn(VISHOTSKY, 20);
        let atraxia = t.spawn(ATRAXIA, 40);
        (t, vishotsky, atraxia)
    }

    /// Accepts the quest from Vishotsky (`31981-03.htm`).
    fn accept(t: &mut ScriptTest, player: i32, vishotsky: Oid) {
        assert!(t.quest_page(player, vishotsky).contains("31981-01.htm"));
        assert!(t.event(player, NAME, "31981-02.htm").contains("31981-02.htm"));
        assert!(t.event(player, NAME, "31981-03.htm").contains("31981-03.htm"));
    }

    #[test]
    fn a_character_of_level_57_is_sent_away() {
        let (mut t, vishotsky, _) = setup();
        t.enter(1, 57, vec![]);
        assert!(t.quest_page(1, vishotsky).contains("31981-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn both_tissues_then_the_reward_from_atraxia() {
        let (mut t, vishotsky, atraxia) = setup();
        t.enter(1, 58, vec![]);
        accept(&mut t, 1, vishotsky);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );

        let storace = t.spawn(VENOMOUS_STORACE, 60);
        t.kill(1, storace);
        assert_eq!(t.count(1, TISSUE_VS), 1);
        let bilette = t.spawn(KEL_BILETTE, 60);
        t.kill(1, bilette);
        assert_eq!(t.count(1, TISSUE_KB), 1);
        assert!(names_to(&t.take(), 1).contains(&"PLAY_SOUND".to_owned()));

        assert!(t.quest_page(1, vishotsky).contains("31981-05.htm"));
        assert!(t.quest_page(1, atraxia).contains("31972-02.htm"));

        // The reward: 210000 adena and `exitQuest(false, true)`.
        assert!(t.event(1, NAME, "31972-03.htm").contains("31972-03.htm"));
        assert_eq!(t.count(1, 57), 210_000);
        assert_eq!((t.count(1, TISSUE_VS), t.count(1, TISSUE_KB)), (0, 0));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(t.quest_page(1, vishotsky).contains("31981-06.htm"));
        assert!(t.quest_page(1, atraxia).contains("This quest has already been completed."));
    }

    #[test]
    fn a_party_kill_gives_the_tissue_to_one_started_member() {
        let (mut t, vishotsky, _) = setup();
        t.enter(1, 60, vec![]);
        t.enter(2, 60, vec![]);
        accept(&mut t, 1, vishotsky);
        accept(&mut t, 2, vishotsky);
        t.party(1, 2);

        let storace = t.spawn(VENOMOUS_STORACE, 60);
        t.kill(1, storace);
        assert_eq!(t.count(1, TISSUE_VS) + t.count(2, TISSUE_VS), 1);
    }
}
