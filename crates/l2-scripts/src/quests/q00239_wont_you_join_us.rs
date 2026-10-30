//! Quest 239: Won't You Join Us? (Athenia in the Hot Springs Village breaks the Waste Landfill
//! Machines and the Suppressors and Exterminators of the Ruins of Agony for the Support
//! Certificate's support).
// Java: dist/game/data/scripts/quests/Q00239_WontYouJoinUs/Q00239_WontYouJoinUs.java
//
// Pattern: two kill counts in a row, each drawn from a random party member at the cond of its
// step (`docs/design/scripts.md`, "Kills, drops and parties"). The items go to the member the
// draw picked, unlike Q00238 (where the killer gets the items).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ATHENIA: i32 = 32643;

// Mobs
const WASTE_LANDFILL_MACHINE: i32 = 18805;
const SUPPRESSOR: i32 = 22656;
const EXTERMINATOR: i32 = 22657;

// Items
const SUPPORT_CERTIFICATE: i32 = 14866;
const DESTROYED_MACHINE_PIECE: i32 = 14869;
const ENCHANTED_GOLEM_FRAGMENT: i32 = 14870;

// Misc
const ENCHANTED_GOLEM_FRAGMENT_NEEDED: i64 = 20;
const DESTROYED_MACHINE_PIECE_NEEDED: i64 = 10;
const CHANCE_FOR_FRAGMENT: i32 = 80;
const MIN_LEVEL: i32 = 82;

/// Java `Q00239_WontYouJoinUs`.
pub struct Q00239WontYouJoinUs;

impl Script for Q00239WontYouJoinUs {
    fn name(&self) -> &'static str {
        "Q00239_WontYouJoinUs"
    }

    fn path(&self) -> &'static str {
        "quests/Q00239_WontYouJoinUs"
    }

    fn id(&self) -> i32 {
        239
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ATHENIA]);
        s.add_talk_id(&[ATHENIA]);
        s.add_kill_id(&[WASTE_LANDFILL_MACHINE, SUPPRESSOR, EXTERMINATOR]);
        s.register_quest_items(&[DESTROYED_MACHINE_PIECE, ENCHANTED_GOLEM_FRAGMENT]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // Java returns null (no page) when there is no quest state.
        let qs = ctx.get_quest_state(player, false)?;
        match event {
            "32643-02.htm" => Some(event.to_owned()),
            "32643-03.html" => {
                qs.start_quest(ctx);
                Some(event.to_owned())
            }
            "32643-07.html" => {
                if qs.is_cond(ctx, 2) {
                    qs.set_cond(ctx, 3, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if npc.id == WASTE_LANDFILL_MACHINE {
            if let Some(member) = ctx.get_random_party_member_cond(killer, 1) {
                let Some(qs) = ctx.get_quest_state(member, false) else {
                    return;
                };
                if ctx.get_quest_items_count(member, DESTROYED_MACHINE_PIECE) < DESTROYED_MACHINE_PIECE_NEEDED
                {
                    ctx.give_items(member, DESTROYED_MACHINE_PIECE, 1);
                }
                if ctx.get_quest_items_count(member, DESTROYED_MACHINE_PIECE)
                    == DESTROYED_MACHINE_PIECE_NEEDED
                {
                    qs.set_cond(ctx, 2, true);
                } else {
                    ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
                }
            }
        } else if let Some(member) = ctx.get_random_party_member_cond(killer, 3)
            && ctx.get_random(100) < CHANCE_FOR_FRAGMENT
        {
            let Some(qs) = ctx.get_quest_state(member, false) else {
                return;
            };
            if ctx.get_quest_items_count(member, ENCHANTED_GOLEM_FRAGMENT) < ENCHANTED_GOLEM_FRAGMENT_NEEDED {
                ctx.give_items(member, ENCHANTED_GOLEM_FRAGMENT, 1);
            }
            if ctx.get_quest_items_count(member, ENCHANTED_GOLEM_FRAGMENT) == ENCHANTED_GOLEM_FRAGMENT_NEEDED
            {
                qs.set_cond(ctx, 4, true);
            } else {
                ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            COMPLETED => htmltext = "32643-11.html".to_owned(),
            CREATED => {
                let q237_done = ctx
                    .player_quest_state(player, "Q00237_WindsOfChange")
                    .is_some_and(|q| q.is_completed());
                let q238_done = ctx
                    .player_quest_state(player, "Q00238_SuccessFailureOfBusiness")
                    .is_some_and(|q| q.is_completed());
                if q238_done {
                    htmltext = "32643-12.html".to_owned();
                } else if q237_done
                    && ctx.player_level(player) >= MIN_LEVEL
                    && ctx.has_quest_items(player, SUPPORT_CERTIFICATE)
                {
                    htmltext = "32643-01.htm".to_owned();
                } else {
                    htmltext = "32643-00.html".to_owned();
                }
            }
            STARTED => match qs.get_cond(ctx) {
                1 => {
                    htmltext = if ctx.has_quest_items(player, DESTROYED_MACHINE_PIECE) {
                        "32643-05.html"
                    } else {
                        "32643-04.html"
                    }
                    .to_owned();
                }
                2 => {
                    if ctx.get_quest_items_count(player, DESTROYED_MACHINE_PIECE)
                        == DESTROYED_MACHINE_PIECE_NEEDED
                    {
                        htmltext = "32643-06.html".to_owned();
                        ctx.take_items(player, DESTROYED_MACHINE_PIECE, -1);
                    }
                }
                3 => {
                    htmltext = if ctx.has_quest_items(player, ENCHANTED_GOLEM_FRAGMENT) {
                        "32643-08.html"
                    } else {
                        "32643-09.html"
                    }
                    .to_owned();
                }
                4 => {
                    if ctx.get_quest_items_count(player, ENCHANTED_GOLEM_FRAGMENT)
                        == ENCHANTED_GOLEM_FRAGMENT_NEEDED
                    {
                        htmltext = "32643-10.html".to_owned();
                        ctx.give_adena(player, 283346, true);
                        ctx.take_items(player, SUPPORT_CERTIFICATE, 1);
                        ctx.add_exp_and_sp(player, 1319736, 103553);
                        qs.exit_quest(ctx, false, true);
                    }
                }
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
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q00239_WontYouJoinUs";
    const Q237: &str = "Q00237_WindsOfChange";

    const NPCS: &str = r#"
        <npc id="32643" type="Folk" level="70" name="Athenia"/>
        <npc id="18805" type="Monster" level="80" name="Waste Landfill Machine"/>
        <npc id="22656" type="Monster" level="80" name="Suppressor"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14866" type="EtcItem" name="Support Certificate"/>
        <item id="14869" type="EtcItem" name="Destroyed Machine Piece"><set name="is_questitem" val="true"/></item>
        <item id="14870" type="EtcItem" name="Enchanted Golem Fragment"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00237_WindsOfChange`: a stand-in whose state a test completes.
    struct Stand237;

    impl Script for Stand237 {
        fn name(&self) -> &'static str {
            Q237
        }
        fn path(&self) -> &'static str {
            "quests/Q00237_WindsOfChange"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Stand237);
        registry.add(Q00239WontYouJoinUs);
        let pages = Pages::of(&Q00239WontYouJoinUs)
            .page("32643-01.htm", &["32643-02.htm"])
            .page("32643-02.htm", &["32643-03.html"])
            .plain(&[
                "32643-00.html",
                "32643-03.html",
                "32643-04.html",
                "32643-05.html",
                "32643-08.html",
                "32643-09.html",
                "32643-10.html",
                "32643-11.html",
                "32643-12.html",
            ])
            .page("32643-06.html", &["32643-07.html"])
            .plain(&["32643-07.html"])
            .npcs(&[ATHENIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let athenia = t.spawn(ATHENIA, 20);
        (t, athenia)
    }

    /// Completes the quest `name` of `player` (the stand-in's state).
    fn complete(t: &mut ScriptTest, name: &'static str, player: i32) {
        t.world.run_as_script(name, |ctx| {
            if let Some(qs) = ctx.quest_state_of(name, Oid(player), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
        t.take_writes();
    }

    /// A level-82 character with the support certificate who finished Winds of Change.
    fn ready(t: &mut ScriptTest) {
        t.enter_with(player_data(1, 82, vec![item(10, SUPPORT_CERTIFICATE, 1)]));
        complete(t, Q237, 1);
    }

    #[test]
    fn the_quest_is_offered_with_the_certificate_after_winds_of_change() {
        let (mut t, athenia) = setup();
        t.enter_with(player_data(1, 82, vec![]));
        complete(&mut t, Q237, 1);
        // No certificate yet.
        assert!(t.quest_page(1, athenia).contains("32643-00.html"));
        t.enter_with(player_data(2, 81, vec![item(10, SUPPORT_CERTIFICATE, 1)]));
        complete(&mut t, Q237, 2);
        assert!(t.quest_page(2, athenia).contains("32643-00.html"));
        t.enter_with(player_data(3, 82, vec![item(20, SUPPORT_CERTIFICATE, 1)]));
        complete(&mut t, Q237, 3);
        assert!(t.quest_page(3, athenia).contains("32643-01.htm"));
    }

    #[test]
    fn the_machines_and_the_golems_bring_the_reward() {
        let (mut t, athenia) = setup();
        ready(&mut t);
        assert!(t.quest_page(1, athenia).contains("32643-01.htm"));
        assert!(t.event(1, NAME, "32643-02.htm").contains("32643-02.htm"));
        assert!(t.event(1, NAME, "32643-03.html").contains("32643-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, athenia).contains("32643-04.html"));

        // Ten machine pieces; the tenth sets cond 2.
        for _ in 0..10 {
            let machine = t.spawn(WASTE_LANDFILL_MACHINE, 20);
            t.kill(1, machine);
        }
        assert_eq!(t.count(1, DESTROYED_MACHINE_PIECE), 10);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, athenia).contains("32643-06.html"));
        assert_eq!(t.count(1, DESTROYED_MACHINE_PIECE), 0);
        assert!(t.event(1, NAME, "32643-07.html").contains("32643-07.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Twenty golem fragments, at 80% a roll; the twentieth sets cond 4.
        assert!(t.quest_page(1, athenia).contains("32643-09.html"));
        for _ in 0..200 {
            if t.count(1, ENCHANTED_GOLEM_FRAGMENT) >= ENCHANTED_GOLEM_FRAGMENT_NEEDED {
                break;
            }
            let suppressor = t.spawn(SUPPRESSOR, 20);
            t.kill(1, suppressor);
        }
        assert_eq!(
            t.count(1, ENCHANTED_GOLEM_FRAGMENT),
            ENCHANTED_GOLEM_FRAGMENT_NEEDED
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Athenia pays; the certificate goes.
        assert!(t.quest_page(1, athenia).contains("32643-10.html"));
        assert_eq!(t.count(1, 57), 283346);
        assert_eq!(
            (
                t.count(1, SUPPORT_CERTIFICATE),
                t.count(1, ENCHANTED_GOLEM_FRAGMENT)
            ),
            (0, 0)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, athenia).contains("32643-11.html"));
    }
}
