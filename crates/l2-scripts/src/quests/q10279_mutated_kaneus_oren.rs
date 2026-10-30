//! Quest 10279: Mutated Kaneus - Oren (Mouen sends the party after Kaim Abigore and Knight Montagnar).
// Java: dist/game/data/scripts/quests/Q10279_MutatedKaneusOren/Q10279_MutatedKaneusOren.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MOUEN: i32 = 30196;
const ROVIA: i32 = 30189;

// Monsters
const KAIM_ABIGORE: i32 = 18566;
const KNIGHT_MONTAGNAR: i32 = 18568;

// Items
const TISSUE_KA: i32 = 13836;
const TISSUE_KM: i32 = 13837;

/// Java `Q10279_MutatedKaneusOren`.
pub struct Q10279MutatedKaneusOren;

/// Java `rewardItem(npcId, player)`: one tissue of the monster, once.
fn reward_item(ctx: &mut ScriptCtx<'_>, npc_id: i32, player: ObjectId) {
    if npc_id == KAIM_ABIGORE && !ctx.has_quest_items(player, TISSUE_KA) {
        ctx.give_items(player, TISSUE_KA, 1);
        ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
    } else if npc_id == KNIGHT_MONTAGNAR && !ctx.has_quest_items(player, TISSUE_KM) {
        ctx.give_items(player, TISSUE_KM, 1);
        ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
    }
}

impl Script for Q10279MutatedKaneusOren {
    fn name(&self) -> &'static str {
        "Q10279_MutatedKaneusOren"
    }

    fn path(&self) -> &'static str {
        "quests/Q10279_MutatedKaneusOren"
    }

    fn id(&self) -> i32 {
        10279
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MOUEN]);
        s.add_talk_id(&[MOUEN, ROVIA]);
        s.add_kill_id(&[KAIM_ABIGORE, KNIGHT_MONTAGNAR]);
        s.register_quest_items(&[TISSUE_KA, TISSUE_KM]);
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
            "30196-03.htm" => qs.start_quest(ctx),
            "30189-03.htm" => {
                ctx.give_adena(player, 100_000, true);
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
                    && ((npc_id == KAIM_ABIGORE && !ctx.has_quest_items(member, TISSUE_KA))
                        || (npc_id == KNIGHT_MONTAGNAR && !ctx.has_quest_items(member, TISSUE_KM)))
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
            MOUEN => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) > 47 {
                        "30196-01.htm"
                    } else {
                        "30196-00.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    htmltext =
                        if ctx.has_quest_items(player, TISSUE_KA) && ctx.has_quest_items(player, TISSUE_KM) {
                            "30196-05.htm"
                        } else {
                            "30196-04.htm"
                        }
                        .to_owned();
                }
                // NOTE(java-quirk): Java's completed page of Mouen is Gosta's `30916-06.htm`.
                COMPLETED => htmltext = "30916-06.htm".to_owned(),
                _ => {}
            },
            ROVIA => match qs.state(ctx) {
                STARTED => {
                    htmltext =
                        if ctx.has_quest_items(player, TISSUE_KA) && ctx.has_quest_items(player, TISSUE_KM) {
                            "30189-02.htm"
                        } else {
                            "30189-01.htm"
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

    const NAME: &str = "Q10279_MutatedKaneusOren";

    const NPCS: &str = r#"
        <npc id="30196" type="Folk" level="50" name="Mouen"/>
        <npc id="30189" type="Folk" level="50" name="Rovia"/>
        <npc id="18566" type="Monster" level="40" name="Kaim Abigore"/>
        <npc id="18568" type="Monster" level="40" name="Knight Montagnar"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13836" type="EtcItem" name="Tissue of Kaim Abigore"><set name="is_questitem" val="true"/></item>
        <item id="13837" type="EtcItem" name="Tissue of Knight Montagnar"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10279MutatedKaneusOren);
        let pages = Pages::of(&Q10279MutatedKaneusOren)
            .plain(&["30196-00.htm", "30196-04.htm", "30196-05.htm"])
            .page("30196-01.htm", &["30196-02.htm"])
            .page("30196-02.htm", &["30196-03.htm"])
            .plain(&["30196-03.htm", "30189-01.htm", "30189-03.htm"])
            .page("30189-02.htm", &["30189-03.htm"])
            .plain(&["30916-06.htm"])
            .npcs(&[MOUEN, ROVIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let mouen = t.spawn(MOUEN, 20);
        let rovia = t.spawn(ROVIA, 40);
        (t, mouen, rovia)
    }

    /// Accepts the quest from Mouen (`30196-03.htm`).
    fn accept(t: &mut ScriptTest, player: i32, mouen: Oid) {
        assert!(t.quest_page(player, mouen).contains("30196-01.htm"));
        assert!(t.event(player, NAME, "30196-02.htm").contains("30196-02.htm"));
        assert!(t.event(player, NAME, "30196-03.htm").contains("30196-03.htm"));
    }

    #[test]
    fn a_character_of_level_47_is_sent_away() {
        let (mut t, mouen, _) = setup();
        t.enter(1, 47, vec![]);
        assert!(t.quest_page(1, mouen).contains("30196-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn both_tissues_then_the_reward_from_rovia() {
        let (mut t, mouen, rovia) = setup();
        t.enter(1, 48, vec![]);
        accept(&mut t, 1, mouen);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );

        let abigore = t.spawn(KAIM_ABIGORE, 60);
        t.kill(1, abigore);
        assert_eq!(t.count(1, TISSUE_KA), 1);
        let montagnar = t.spawn(KNIGHT_MONTAGNAR, 60);
        t.kill(1, montagnar);
        assert_eq!(t.count(1, TISSUE_KM), 1);
        assert!(names_to(&t.take(), 1).contains(&"PLAY_SOUND".to_owned()));

        assert!(t.quest_page(1, mouen).contains("30196-05.htm"));
        assert!(t.quest_page(1, rovia).contains("30189-02.htm"));

        // The reward: 100000 adena and `exitQuest(false, true)`.
        assert!(t.event(1, NAME, "30189-03.htm").contains("30189-03.htm"));
        assert_eq!(t.count(1, 57), 100_000);
        assert_eq!((t.count(1, TISSUE_KA), t.count(1, TISSUE_KM)), (0, 0));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        // NOTE(java-quirk): Mouen's completed page is Gosta's page, as in Java.
        assert!(t.quest_page(1, mouen).contains("30916-06.htm"));
        assert!(
            t.quest_page(1, rovia)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn a_party_kill_gives_the_tissue_to_one_started_member() {
        let (mut t, mouen, _) = setup();
        t.enter(1, 50, vec![]);
        t.enter(2, 50, vec![]);
        accept(&mut t, 1, mouen);
        accept(&mut t, 2, mouen);
        t.party(1, 2);

        let abigore = t.spawn(KAIM_ABIGORE, 60);
        t.kill(1, abigore);
        assert_eq!(t.count(1, TISSUE_KA) + t.count(2, TISSUE_KA), 1);
    }
}
