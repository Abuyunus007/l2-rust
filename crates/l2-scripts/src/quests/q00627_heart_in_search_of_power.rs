//! Quest 627: Heart in Search of Power (the Mysterious Necromancer and Enfeux in the Gludin area;
//! the splendor monsters drop the beads of obedience, 300 of them buy the seal of light, and the
//! gem of saints buys one of five rewards).
// Java: dist/game/data/scripts/quests/Q00627_HeartInSearchOfPower/Q00627_HeartInSearchOfPower.java
//
// Pattern: a party kill count with a drop chance per monster (per mille, the member at cond 1,
// no range check), three NPCs that move the cond (without sound), and a reward page of named
// choices (`Adena`, `Asofes`, ...) that ends the quest. The reward events are "bypass -h" links
// in the real page.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MYSTERIOUS_NECROMANCER: i32 = 31518;
const ENFEUX: i32 = 31519;

// Items
const SEAL_OF_LIGHT: i32 = 7170;
const BEAD_OF_OBEDIENCE: i32 = 7171;
const GEM_OF_SAINTS: i32 = 7172;

/// Java `MONSTERS`: the bead drop chance (per mille) of each splendor monster.
const MONSTERS: [(i32, i32); 14] = [
    (21520, 661), // Eye of Splendor
    (21523, 668), // Flash of Splendor
    (21524, 714), // Blade of Splendor
    (21525, 714), // Blade of Splendor
    (21526, 796), // Wisdom of Splendor
    (21529, 659), // Soul of Splendor
    (21530, 704), // Victory of Splendor
    (21531, 791), // Punishment of Splendor
    (21532, 820), // Shout of Splendor
    (21535, 827), // Signet of Splendor
    (21536, 798), // Crown of Splendor
    (21539, 875), // Wailing of Splendor
    (21540, 875), // Wailing of Splendor
    (21658, 791), // Punishment of Splendor
];

// Misc
const MIN_LEVEL_REQUIRED: i32 = 60;
const BEAD_OF_OBEDIENCE_COUNT_REQUIRED: i64 = 300;

// Rewards ID's
const ASOFE: i32 = 4043;
const THONS: i32 = 4044;
const ENRIA: i32 = 4042;
const MOLD_HARDENER: i32 = 4041;

/// Java `Q00627_HeartInSearchOfPower`.
pub struct Q00627HeartInSearchOfPower;

impl Script for Q00627HeartInSearchOfPower {
    fn name(&self) -> &'static str {
        "Q00627_HeartInSearchOfPower"
    }

    fn path(&self) -> &'static str {
        "quests/Q00627_HeartInSearchOfPower"
    }

    fn id(&self) -> i32 {
        627
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MYSTERIOUS_NECROMANCER]);
        s.add_talk_id(&[MYSTERIOUS_NECROMANCER, ENFEUX]);
        let mobs: Vec<i32> = MONSTERS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[SEAL_OF_LIGHT, BEAD_OF_OBEDIENCE, GEM_OF_SAINTS]);
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
        // Java: `htmltext = event` by default; `null` for the events not listed below.
        let mut htmltext = Some(event.to_owned());
        match event {
            "31518-02.htm" => qs.start_quest(ctx),
            "31518-06.html" => {
                if ctx.get_quest_items_count(player, BEAD_OF_OBEDIENCE) < BEAD_OF_OBEDIENCE_COUNT_REQUIRED {
                    return Some("31518-05.html".to_owned());
                }
                ctx.give_items(player, SEAL_OF_LIGHT, 1);
                ctx.take_items(player, BEAD_OF_OBEDIENCE, -1);
                qs.set_cond(ctx, 3, false);
            }
            "Adena" | "Asofes" | "Thons" | "Enrias" | "Mold_Hardener" => {
                if !ctx.has_quest_items(player, GEM_OF_SAINTS) {
                    return Some("31518-11.html".to_owned());
                }
                match event {
                    "Adena" => ctx.give_adena(player, 100_000, true),
                    "Asofes" => {
                        ctx.reward_items(player, ASOFE, 13);
                        ctx.give_adena(player, 6_400, true);
                    }
                    "Thons" => {
                        ctx.reward_items(player, THONS, 13);
                        ctx.give_adena(player, 6_400, true);
                    }
                    "Enrias" => {
                        ctx.reward_items(player, ENRIA, 6);
                        ctx.give_adena(player, 13_600, true);
                    }
                    "Mold_Hardener" => {
                        ctx.reward_items(player, MOLD_HARDENER, 3);
                        ctx.give_adena(player, 17_200, true);
                    }
                    _ => {}
                }
                htmltext = Some("31518-10.html".to_owned());
                // Java `exitQuest(true)`: no sound.
                qs.exit_quest(ctx, true, false);
            }
            "31519-02.html" => {
                if ctx.has_quest_items(player, SEAL_OF_LIGHT) && qs.is_cond(ctx, 3) {
                    ctx.give_items(player, GEM_OF_SAINTS, 1);
                    ctx.take_items(player, SEAL_OF_LIGHT, -1);
                    qs.set_cond(ctx, 4, false);
                } else {
                    htmltext = Some(ctx.get_no_quest_msg(player));
                }
            }
            "31518-09.html" => {}
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMember(killer, 1)`: no range check here.
        let Some(member) = ctx.get_random_party_member_cond(killer, 1) else {
            return;
        };
        let Some(&(_, chance)) = MONSTERS.iter().find(|(id, _)| *id == npc.id) else {
            return;
        };
        if ctx.get_random(1000) < chance {
            ctx.give_items(member, BEAD_OF_OBEDIENCE, 1);
            if ctx.get_quest_items_count(member, BEAD_OF_OBEDIENCE) < BEAD_OF_OBEDIENCE_COUNT_REQUIRED {
                ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
            } else if let Some(qs) = ctx.get_quest_state(member, false) {
                qs.set_cond(ctx, 2, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            if npc.id == MYSTERIOUS_NECROMANCER {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL_REQUIRED {
                    "31518-01.htm".to_owned()
                } else {
                    "31518-00.htm".to_owned()
                };
            }
        } else if state == STARTED {
            if npc.id == MYSTERIOUS_NECROMANCER {
                match qs.get_cond(ctx) {
                    1 => htmltext = "31518-03.html".to_owned(),
                    2 => htmltext = "31518-04.html".to_owned(),
                    3 => htmltext = "31518-07.html".to_owned(),
                    4 => htmltext = "31518-08.html".to_owned(),
                    _ => {}
                }
            } else if npc.id == ENFEUX {
                match qs.get_cond(ctx) {
                    3 => htmltext = "31519-01.html".to_owned(),
                    4 => htmltext = "31519-03.html".to_owned(),
                    _ => {}
                }
            }
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

    const NAME: &str = "Q00627_HeartInSearchOfPower";

    const NPCS: &str = r#"
        <npc id="31518" type="Folk" level="70" name="Mysterious Necromancer"/>
        <npc id="31519" type="Folk" level="70" name="Enfeux"/>
        <npc id="21524" type="Monster" level="70" name="Blade of Splendor"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7170" type="EtcItem" name="Seal of Light"><set name="is_questitem" val="true"/></item>
        <item id="7171" type="EtcItem" name="Bead of Obedience"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7172" type="EtcItem" name="Gem of Saints"><set name="is_questitem" val="true"/></item>
        <item id="4041" type="EtcItem" name="Mold Hardener"><set name="is_stackable" val="true"/></item>
        <item id="4042" type="EtcItem" name="Enria"><set name="is_stackable" val="true"/></item>
        <item id="4043" type="EtcItem" name="Asofe"><set name="is_stackable" val="true"/></item>
        <item id="4044" type="EtcItem" name="Thons"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00627HeartInSearchOfPower);
        let pages = Pages::of(&Q00627HeartInSearchOfPower)
            .page("31518-01.htm", &["31518-02.htm"])
            .plain(&[
                "31518-00.htm",
                "31518-02.htm",
                "31518-03.html",
                "31518-05.html",
                "31518-07.html",
                "31518-10.html",
                "31518-11.html",
            ])
            .page("31518-04.html", &["31518-06.html"])
            .plain(&["31518-06.html"])
            .page("31518-08.html", &["31518-09.html"])
            .page(
                "31518-09.html",
                &["Adena", "Asofes", "Enrias", "Mold_Hardener", "Thons"],
            )
            .page("31519-01.html", &["31519-02.html"])
            .plain(&["31519-02.html", "31519-03.html"])
            .npcs(&[MYSTERIOUS_NECROMANCER, ENFEUX]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let necromancer = t.spawn(MYSTERIOUS_NECROMANCER, 20);
        let enfeux = t.spawn(ENFEUX, 40);
        (t, necromancer, enfeux)
    }

    #[test]
    fn a_character_below_level_60_is_sent_away() {
        let (mut t, necromancer, _) = setup();
        t.enter(1, 59, vec![]);
        assert!(t.quest_page(1, necromancer).contains("31518-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn three_hundred_beads_lead_to_the_seal_then_the_gem_and_the_adena_ends_the_quest() {
        let (mut t, necromancer, enfeux) = setup();
        t.enter(1, 60, vec![]);
        assert!(t.quest_page(1, necromancer).contains("31518-01.htm"));
        assert!(t.event(1, NAME, "31518-02.htm").contains("31518-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Beads to 300: the last kill sets cond 2.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), BEAD_OF_OBEDIENCE, 299);
        });
        t.take();
        let mut kills = 0;
        while t.count(1, BEAD_OF_OBEDIENCE) < BEAD_OF_OBEDIENCE_COUNT_REQUIRED {
            let mob = t.spawn(21524, 40);
            t.kill(1, mob);
            kills += 1;
            assert!(kills < 100, "no bead dropped");
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The seal: the beads go, cond 3 (no sound).
        assert!(t.quest_page(1, necromancer).contains("31518-04.html"));
        assert!(t.event(1, NAME, "31518-06.html").contains("31518-06.html"));
        assert_eq!(t.count(1, SEAL_OF_LIGHT), 1);
        assert_eq!(t.count(1, BEAD_OF_OBEDIENCE), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Enfeux trades the seal for the gem: cond 4.
        assert!(t.quest_page(1, enfeux).contains("31519-01.html"));
        assert!(t.event(1, NAME, "31519-02.html").contains("31519-02.html"));
        assert_eq!(t.count(1, SEAL_OF_LIGHT), 0);
        assert_eq!(t.count(1, GEM_OF_SAINTS), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // The reward from the necromancer's page: 100000 adena, the quest ends.
        assert!(t.quest_page(1, necromancer).contains("31518-08.html"));
        assert!(t.event(1, NAME, "31518-09.html").contains("31518-09.html"));
        assert!(t.event(1, NAME, "Adena").contains("31518-10.html"));
        assert_eq!(t.count(1, 57), 100_000);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
