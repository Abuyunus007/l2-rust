//! Quest 186: Contract Execution (Lorain, Maestro Nikola and Blueprint Seller Luka; the Leto Lizardmen).
// Java: dist/game/data/scripts/quests/Q00186_ContractExecution/Q00186_ContractExecution.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MAESTRO_NIKOLA: i32 = 30621;
const RESEARCHER_LORAIN: i32 = 30673;
const BLUEPRINT_SELLER_LUKA: i32 = 31437;

// Items
const LORAINES_CERTIFICATE: i32 = 10362;
const METALLOGRAPH_RESEARCH_REPORT: i32 = 10366;
const LETO_LIZARDMAN_ACCESSORY: i32 = 10367;

// Misc
const MIN_LEVEL: i32 = 41;
const MAX_LEVEL_FOR_EXP_SP: i32 = 47;

/// Java `MONSTERS`: the Leto Lizardmen and their chance (percent) of the accessory.
const MONSTERS: [(i32, i32); 6] = [
    (20577, 40),  // Leto Lizardman
    (20578, 44),  // Leto Lizardman Archer
    (20579, 46),  // Leto Lizardman Soldier
    (20580, 88),  // Leto Lizardman Warrior
    (20581, 50),  // Leto Lizardman Shaman
    (20582, 100), // Leto Lizardman Overlord
];

/// Java `Q00184_ArtOfPersuasion.class.getSimpleName()`: the quest this one follows.
const ART_OF_PERSUASION: &str = "Q00184_ArtOfPersuasion";

/// Java `Q00186_ContractExecution`.
pub struct Q00186ContractExecution;

impl Script for Q00186ContractExecution {
    fn name(&self) -> &'static str {
        "Q00186_ContractExecution"
    }

    fn path(&self) -> &'static str {
        "quests/Q00186_ContractExecution"
    }

    fn id(&self) -> i32 {
        186
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RESEARCHER_LORAIN]);
        s.add_talk_id(&[RESEARCHER_LORAIN, BLUEPRINT_SELLER_LUKA, MAESTRO_NIKOLA]);
        let kill_ids: Vec<i32> = MONSTERS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&kill_ids);
        s.register_quest_items(&[METALLOGRAPH_RESEARCH_REPORT, LETO_LIZARDMAN_ACCESSORY]);
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

        let mut htmltext = None;
        match event {
            "30673-03.htm" => {
                if ctx.player_level(player) >= MIN_LEVEL && ctx.has_quest_items(player, LORAINES_CERTIFICATE)
                {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    ctx.give_items(player, METALLOGRAPH_RESEARCH_REPORT, 1);
                    ctx.take_items(player, LORAINES_CERTIFICATE, -1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30621-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30621-03.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31437-03.html" => {
                if qs.is_memo_state(ctx, 2) && ctx.has_quest_items(player, LETO_LIZARDMAN_ACCESSORY) {
                    htmltext = Some(event.to_owned());
                }
            }
            "31437-04.html" => {
                if qs.is_memo_state(ctx, 2) && ctx.has_quest_items(player, LETO_LIZARDMAN_ACCESSORY) {
                    qs.set_memo_state(ctx, 3);
                    htmltext = Some(event.to_owned());
                }
            }
            "31437-06.html" => {
                if qs.is_memo_state(ctx, 3) {
                    ctx.give_adena(player, 105_083, true);
                    if ctx.player_level(player) < MAX_LEVEL_FOR_EXP_SP {
                        ctx.add_exp_and_sp(player, 285_935, 18_711);
                    }
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if let Some(qs) = ctx.get_quest_state(killer, false)
            && qs.is_memo_state(ctx, 2)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, false)
        {
            let chance = MONSTERS
                .iter()
                .find(|(id, _)| *id == npc.id)
                .map_or(0, |(_, chance)| *chance);
            if ctx.get_random(100) < chance && !ctx.has_quest_items(killer, LETO_LIZARDMAN_ACCESSORY) {
                ctx.give_items(killer, LETO_LIZARDMAN_ACCESSORY, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                if npc.id == RESEARCHER_LORAIN {
                    // `player.getQuestState(Q00184_ArtOfPersuasion)`: read only.
                    let art_done = ctx
                        .player_quest_state(player, ART_OF_PERSUASION)
                        .is_some_and(|q184| q184.is_completed());
                    if art_done && ctx.has_quest_items(player, LORAINES_CERTIFICATE) {
                        htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                            "30673-01.htm"
                        } else {
                            "30673-02.htm"
                        }
                        .to_owned();
                    }
                }
            }
            STARTED => match npc.id {
                RESEARCHER_LORAIN => {
                    if memo_state >= 1 {
                        htmltext = "30673-04.html".to_owned();
                    }
                }
                MAESTRO_NIKOLA => {
                    if memo_state == 1 {
                        htmltext = "30621-01.html".to_owned();
                    } else if memo_state == 2 {
                        htmltext = "30621-04.html".to_owned();
                    }
                }
                BLUEPRINT_SELLER_LUKA => {
                    if memo_state == 2 {
                        htmltext = if ctx.has_quest_items(player, LETO_LIZARDMAN_ACCESSORY) {
                            "31437-02.html"
                        } else {
                            "31437-01.html"
                        }
                        .to_owned();
                    } else if memo_state == 3 {
                        htmltext = "31437-05.html".to_owned();
                    }
                }
                _ => {}
            },
            COMPLETED => {
                if npc.id == RESEARCHER_LORAIN {
                    htmltext = ctx.get_already_completed_msg(player);
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
    use l2_world::script::{COMPLETED, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00186_ContractExecution";

    const NPCS: &str = r#"
        <npc id="30621" type="Folk" level="70" name="Maestro Nikola"/>
        <npc id="30673" type="Folk" level="70" name="Researcher Lorain"/>
        <npc id="31437" type="Folk" level="70" name="Blueprint Seller Luka"/>
        <npc id="20577" type="Monster" level="40" name="Leto Lizardman"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10362" type="EtcItem" name="Loraine's Certificate"><set name="is_questitem" val="true"/></item>
        <item id="10366" type="EtcItem" name="Metallograph Research Report"><set name="is_questitem" val="true"/></item>
        <item id="10367" type="EtcItem" name="Leto Lizardman Accessory"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00184_ArtOfPersuasion` as far as this quest reads it: a script of that name.
    struct ArtOfPersuasion;

    impl Script for ArtOfPersuasion {
        fn name(&self) -> &'static str {
            ART_OF_PERSUASION
        }
        fn path(&self) -> &'static str {
            "quests/Q00184_ArtOfPersuasion"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        nikola: Oid,
        lorain: Oid,
        luka: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(ArtOfPersuasion);
        registry.add(Q00186ContractExecution);
        let pages = Pages::of(&Q00186ContractExecution)
            .page("30673-01.htm", &["30673-03.htm"])
            .plain(&["30673-02.htm", "30673-03.htm", "30673-04.html"])
            .page("30621-01.html", &["30621-02.html"])
            .page("30621-02.html", &["30621-03.html"])
            .plain(&["30621-03.html", "30621-04.html"])
            .page("31437-02.html", &["31437-03.html"])
            .page("31437-03.html", &["31437-04.html"])
            .page("31437-04.html", &["31437-06.html"])
            .page("31437-05.html", &["31437-06.html"])
            .plain(&["31437-01.html", "31437-06.html"])
            .npcs(&[MAESTRO_NIKOLA, RESEARCHER_LORAIN, BLUEPRINT_SELLER_LUKA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            nikola: t.spawn(MAESTRO_NIKOLA, 20),
            lorain: t.spawn(RESEARCHER_LORAIN, 40),
            luka: t.spawn(BLUEPRINT_SELLER_LUKA, 60),
        };
        (t, npcs)
    }

    fn finish_art_of_persuasion(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(ART_OF_PERSUASION, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
    }

    /// Kills the Leto Lizardmen near the killer until the accessory is in; returns the kills.
    fn hunt(t: &mut ScriptTest, npcs_of: i32) -> usize {
        for kills in 1..400 {
            let mob = t.spawn(npcs_of, 60);
            t.kill(1, mob);
            if t.count(1, LETO_LIZARDMAN_ACCESSORY) >= 1 {
                return kills;
            }
        }
        panic!("no accessory after 400 kills");
    }

    #[test]
    fn only_a_level_41_with_the_certificate_and_art_of_persuasion_is_offered_the_quest() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 41, race::HUMAN, 0, vec![item(100, LORAINES_CERTIFICATE, 1)]);
        t.enter_as(2, 40, race::HUMAN, 0, vec![item(101, LORAINES_CERTIFICATE, 1)]);
        t.enter_as(3, 41, race::HUMAN, 0, vec![item(102, LORAINES_CERTIFICATE, 1)]);
        finish_art_of_persuasion(&mut t, 1);
        finish_art_of_persuasion(&mut t, 2);
        assert!(t.quest_page(1, npcs.lorain).contains("30673-01.htm"));
        assert!(t.quest_page(2, npcs.lorain).contains("30673-02.htm"));
        // Player 3 has no Art of Persuasion: the no-quest message.
        assert!(!t.quest_page(3, npcs.lorain).contains("30673-"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_accessory_then_luka_pays() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 41, race::HUMAN, 0, vec![item(100, LORAINES_CERTIFICATE, 1)]);
        finish_art_of_persuasion(&mut t, 1);

        assert!(t.quest_page(1, npcs.lorain).contains("30673-01.htm"));
        assert!(t.event(1, NAME, "30673-03.htm").contains("30673-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert_eq!(
            (
                t.count(1, LORAINES_CERTIFICATE),
                t.count(1, METALLOGRAPH_RESEARCH_REPORT)
            ),
            (0, 1)
        );

        // Nikola: memo state 2 and cond 2.
        assert!(t.quest_page(1, npcs.nikola).contains("30621-01.html"));
        assert!(t.event(1, NAME, "30621-02.html").contains("30621-02.html"));
        assert!(t.event(1, NAME, "30621-03.html").contains("30621-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2")
            ]
        );
        assert!(t.quest_page(1, npcs.nikola).contains("30621-04.html"));

        // Luka without the accessory: the page that sends the player out.
        assert!(t.quest_page(1, npcs.luka).contains("31437-01.html"));

        // The Leto Lizardmen: the accessory (40% for 20577), only near the killer, at memo 2.
        hunt(&mut t, 20577);
        t.take_writes();

        // Luka with the accessory: memo state 3.
        assert!(t.quest_page(1, npcs.luka).contains("31437-02.html"));
        assert!(t.event(1, NAME, "31437-03.html").contains("31437-03.html"));
        assert!(t.event(1, NAME, "31437-04.html").contains("31437-04.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "3")]);
        assert!(t.quest_page(1, npcs.luka).contains("31437-05.html"));

        // The reward: adena, exp and sp (level 41 < 47), `exitQuest(false, true)`.
        assert!(t.event(1, NAME, "31437-06.html").contains("31437-06.html"));
        assert_eq!(t.count(1, 57), 105_083);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, npcs.lorain).contains("already been completed"));
    }
}
