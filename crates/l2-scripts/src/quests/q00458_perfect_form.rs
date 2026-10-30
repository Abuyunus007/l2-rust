//! Quest 458: Perfect Form (Kelleyia; feed the full grown beasts and beat them with overhits).
// Java: dist/game/data/scripts/quests/Q00458_PerfectForm/Q00458_PerfectForm.java
//
// Pattern: a kill log per monster (a variable per beast, up to 10 each), the overhit counts of the
// kills in the quest variables, and result pages whose numbers come from those variables
// (`getHtm` with `<?number?>` replaced). The reward is a daily one.

use l2_protocol_game::server::ExQuestNpcLogList;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, QuestType, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KELLEYIA: i32 = 32768;

// Monsters: level 4 (full grown) feedable beasts, the first of each pair is the young one.
const KOOKABURRAS: [i32; 2] = [18878, 18879];
const COUGARS: [i32; 2] = [18885, 18886];
const BUFFALOS: [i32; 2] = [18892, 18893];
const GRENDELS: [i32; 2] = [18899, 18900];

// Rewards: 60% Icarus weapon recipes (except kamael weapons)
const ICARUS_WEAPON_RECIPES: [i32; 9] = [10373, 10374, 10375, 10376, 10377, 10378, 10379, 10380, 10381];
const ICARUS_WEAPON_PIECES: [i32; 9] = [10397, 10398, 10399, 10400, 10401, 10402, 10403, 10404, 10405];
const GOLDEN_SPICE_CRATE: i32 = 15482;
const CRYSTAL_SPICE_CRATE: i32 = 15483;

/// Java `Q00458_PerfectForm`.
pub struct Q00458PerfectForm;

impl Script for Q00458PerfectForm {
    fn name(&self) -> &'static str {
        "Q00458_PerfectForm"
    }

    fn path(&self) -> &'static str {
        "quests/Q00458_PerfectForm"
    }

    fn id(&self) -> i32 {
        458
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KELLEYIA]);
        s.add_talk_id(&[KELLEYIA]);
        s.add_kill_id(&KOOKABURRAS);
        s.add_kill_id(&COUGARS);
        s.add_kill_id(&BUFFALOS);
        s.add_kill_id(&GRENDELS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let no_quest = ctx.get_no_quest_msg(player);
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(no_quest);
        };

        let mut htmltext = event.to_owned();
        let mut over_hits = 0;
        let mut over_hit_html = false;

        match event {
            "32768-10.htm" => qs.start_quest(ctx),
            "results1" => {
                if qs.is_cond(ctx, 2) {
                    let overhits_total = qs.get_int(ctx, "overhitsTotal");
                    htmltext = if overhits_total >= 35 {
                        "32768-14a.html"
                    } else if overhits_total >= 10 {
                        "32768-14b.html"
                    } else {
                        "32768-14c.html"
                    }
                    .to_owned();
                    over_hits = overhits_total;
                    over_hit_html = true;
                } else {
                    htmltext = no_quest.clone();
                }
            }
            "results2" => {
                if qs.is_cond(ctx, 2) {
                    let overhits_critical = qs.get_int(ctx, "overhitsCritical");
                    htmltext = if overhits_critical >= 30 {
                        "32768-15a.html"
                    } else if overhits_critical >= 5 {
                        "32768-15b.html"
                    } else {
                        "32768-15c.html"
                    }
                    .to_owned();
                    over_hits = overhits_critical;
                    over_hit_html = true;
                } else {
                    htmltext = no_quest.clone();
                }
            }
            "results3" => {
                if qs.is_cond(ctx, 2) {
                    let overhits_consecutive = qs.get_int(ctx, "overhitsConsecutive");
                    htmltext = if overhits_consecutive >= 20 {
                        "32768-16a.html"
                    } else if overhits_consecutive >= 7 {
                        "32768-16b.html"
                    } else {
                        "32768-16c.html"
                    }
                    .to_owned();
                    over_hits = overhits_consecutive;
                    over_hit_html = true;
                } else {
                    htmltext = no_quest.clone();
                }
            }
            "32768-17.html" => {
                if qs.is_cond(ctx, 2) {
                    let overhits_consecutive = qs.get_int(ctx, "overhitsConsecutive");
                    if overhits_consecutive >= 20 {
                        if let Some(item) = ctx.get_random_entry(&ICARUS_WEAPON_RECIPES) {
                            ctx.reward_items(player, item, 1);
                        }
                    } else if overhits_consecutive >= 7 {
                        if let Some(item) = ctx.get_random_entry(&ICARUS_WEAPON_PIECES) {
                            ctx.reward_items(player, item, 5);
                        }
                    } else {
                        let rnd = ctx.get_random(ICARUS_WEAPON_PIECES.len() as i32);
                        ctx.reward_items(player, ICARUS_WEAPON_PIECES[rnd as usize], 2);

                        // NOTE(java-quirk): Java gives these two crates with `giveItems`, not
                        // `rewardItems` ("not sure if this should use rewardItems"): no quest rates.
                        ctx.give_items(player, GOLDEN_SPICE_CRATE, 10);
                        ctx.give_items(player, CRYSTAL_SPICE_CRATE, 10);
                    }

                    qs.exit_quest_type(ctx, QuestType::Daily, true);
                } else {
                    htmltext = no_quest.clone();
                }
            }
            _ => {}
        }

        if over_hit_html {
            // Java `getHtm(player, htmltext)` then `replace("<?number?>", overHits)`. A page that is
            // missing is no page here (Java would fail on the null).
            return ctx
                .get_htm(player, &htmltext)
                .map(|page| page.replace("<?number?>", &over_hits.to_string()));
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !qs.is_cond(ctx, 1) {
            return;
        }

        // The young beast and the full grown one count as the same monster (the second id).
        let mut npc_id = npc.id;
        if [KOOKABURRAS[0], COUGARS[0], BUFFALOS[0], GRENDELS[0]].contains(&npc_id) {
            npc_id += 1;
        }

        let variable = npc_id.to_string();
        let current_value = qs.get_int(ctx, &variable);
        if current_value < 10 {
            qs.set(ctx, &variable, &(current_value + 1).to_string());

            if ctx.world().is_overhit(npc.object_id) {
                let total = qs.get_int(ctx, "overhitsTotal");
                qs.set(ctx, "overhitsTotal", &(total + 1).to_string());
                let max_hp = f64::from(ctx.npc_max_hp(npc));
                // Java `(maxHp + mob.getOverhitDamage()) / maxHp`: a double division.
                let overhit_percentage = (max_hp + ctx.world().overhit_damage(npc.object_id)) / max_hp;
                if overhit_percentage >= 1.2 {
                    let critical = qs.get_int(ctx, "overhitsCritical");
                    qs.set(ctx, "overhitsCritical", &(critical + 1).to_string());
                }
                let consecutive = qs.get_int(ctx, "overhitsConsecutive");
                qs.set(ctx, "overhitsConsecutive", &(consecutive + 1).to_string());
                // NOTE(java-quirk): the retail rule that trims `overhitsConsecutive` is commented
                // out in Java: the count keeps growing.
            } else if qs.get_int(ctx, "overhitsConsecutive") > 0 {
                // Java avoids a database write when the variable is already zero.
                qs.set(ctx, "overhitsConsecutive", "0");
            }

            let full_grown = [18879, 18886, 18893, 18900];
            if full_grown.iter().all(|id| qs.get_int(ctx, &id.to_string()) == 10) {
                qs.set_cond(ctx, 2, true);
            } else {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }

            let mut log = ExQuestNpcLogList::new(ctx.quest_id());
            for id in full_grown {
                log.add_npc(id, qs.get_int(ctx, &id.to_string()));
            }
            ctx.world().send(killer, &log);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);

        let mut state = qs.state(ctx);
        // Java: `case COMPLETED` falls through to `CREATED` once the quest is available again.
        if state == COMPLETED {
            if !qs.is_now_available(ctx) {
                return Some("32768-18.htm".to_owned());
            }
            qs.set_state(ctx, CREATED);
            state = CREATED;
        }
        match state {
            CREATED => {
                htmltext = if ctx.player_level(player) > 81 {
                    "32768-01.htm"
                } else {
                    "32768-00.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => {
                    let none_killed = [18879, 18886, 18893, 18900]
                        .iter()
                        .all(|id| qs.get_int(ctx, &id.to_string()) == 0);
                    htmltext = if none_killed {
                        "32768-11.html"
                    } else {
                        "32768-12.html"
                    }
                    .to_owned();
                }
                2 => htmltext = "32768-13.html".to_owned(),
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
    use l2_world::{QuestWrite, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00458_PerfectForm";

    const NPCS: &str = r#"
        <npc id="32768" type="Folk" level="70" name="Kelleyia"/>
        <npc id="18879" type="Monster" level="4" name="Full Grown Kookaburra"/>
        <npc id="18886" type="Monster" level="4" name="Full Grown Cougar"/>
        <npc id="18893" type="Monster" level="4" name="Full Grown Buffalo"/>
        <npc id="18900" type="Monster" level="4" name="Full Grown Grendel"/>"#;

    fn items() -> String {
        let mut xml = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15482" type="EtcItem" name="Golden Spice Crate"><set name="is_stackable" val="true"/></item>
        <item id="15483" type="EtcItem" name="Crystal Spice Crate"><set name="is_stackable" val="true"/></item>"#,
        );
        for id in ICARUS_WEAPON_RECIPES.iter().chain(ICARUS_WEAPON_PIECES.iter()) {
            xml.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Icarus {id}"><set name="is_stackable" val="true"/></item>"#
            ));
        }
        xml
    }

    /// The pages of the quest. The result pages carry the `<?number?>` the port replaces.
    fn pages() -> Pages {
        let mut pages = Pages::of(&Q00458PerfectForm)
            .page("32768-01.htm", &["32768-02.html", "32768-04.html"])
            .page("32768-02.html", &["32768-03.html", "32768-04.html"])
            .page("32768-03.html", &["32768-04.html"])
            .page("32768-04.html", &["32768-05.html"])
            .page("32768-05.html", &["32768-06.htm"])
            .page("32768-06.htm", &["32768-10.htm", "32768-07.htm"])
            .page("32768-07.htm", &["32768-10.htm", "32768-08.htm"])
            .page("32768-08.htm", &["32768-10.htm", "32768-09.htm"])
            .page("32768-09.htm", &["32768-10.htm", "32768-08.htm"])
            .plain(&[
                "32768-00.htm",
                "32768-10.htm",
                "32768-11.html",
                "32768-12.html",
                "32768-18.htm",
            ])
            .page("32768-13.html", &["results1"])
            .plain(&["32768-17.html"]);
        // The result pages: their number and the link of the next result (the real pages use
        // `bypass -h Script Q00458_PerfectForm <event>`, cached as `Script ... <event>`).
        for (file, next) in [
            ("32768-14a.html", "results2"),
            ("32768-14b.html", "results2"),
            ("32768-14c.html", "results2"),
            ("32768-15a.html", "results3"),
            ("32768-15b.html", "results3"),
            ("32768-15c.html", "results3"),
            ("32768-16a.html", "32768-17.html"),
            ("32768-16b.html", "32768-17.html"),
            ("32768-16c.html", "32768-17.html"),
        ] {
            pages = pages.file(
                &format!("data/scripts/quests/Q00458_PerfectForm/{file}"),
                &format!(
                    r#"<html><body>{file} <?number?><a action="bypass Script Q00458_PerfectForm {next}">next</a></body></html>"#
                ),
            );
        }
        pages.npcs(&[KELLEYIA])
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00458PerfectForm);
        let items = items();
        let mut t = ScriptTest::with_pages(registry, NPCS, &items, pages());
        let kelleyia = t.spawn(KELLEYIA, 20);
        (t, kelleyia)
    }

    /// Kelleyia's page for the player (talk, then the quest window).
    fn quest_page(t: &mut ScriptTest, npc: Oid) -> String {
        t.quest_page(1, npc)
    }

    /// Sets the quest variables the kills and the overhits would have set.
    fn set_var(t: &mut ScriptTest, var: &'static str, value: i32) {
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_int(ctx, var, value);
        });
    }

    /// Accepts the quest from the first page.
    fn accept(t: &mut ScriptTest, kelleyia: Oid) {
        assert!(quest_page(t, kelleyia).contains("32768-01.htm"));
        assert!(t.event(1, NAME, "32768-04.html").contains("32768-04.html"));
        assert!(t.event(1, NAME, "32768-05.html").contains("32768-05.html"));
        assert!(t.event(1, NAME, "32768-06.htm").contains("32768-06.htm"));
        assert!(t.event(1, NAME, "32768-10.htm").contains("32768-10.htm"));
    }

    #[test]
    fn a_level_81_character_is_sent_away() {
        let (mut t, kelleyia) = setup();
        t.enter(1, 81, vec![]);
        assert!(quest_page(&mut t, kelleyia).contains("32768-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_start_writes_the_cond_and_the_state() {
        let (mut t, kelleyia) = setup();
        t.enter(1, 82, vec![]);
        accept(&mut t, kelleyia);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(quest_page(&mut t, kelleyia).contains("32768-11.html"));
    }

    #[test]
    fn ten_of_each_beast_then_the_second_stage() {
        let (mut t, kelleyia) = setup();
        t.enter(1, 82, vec![]);
        accept(&mut t, kelleyia);
        t.take_writes();

        // The log goes to the player at each kill.
        let first = t.spawn(18879, 60);
        t.kill(1, first);
        assert!(names_to(&t.take(), 1).contains(&"EX_QUEST_NPC_LOG_LIST".to_owned()));
        assert_eq!(t.take_writes(), [rows::insert(NAME, "18879", "1")]);

        // Ten of each full grown beast (the first kill above counts); the last kill sets cond 2.
        for id in [18879, 18886, 18893, 18900] {
            let kills = if id == 18879 { 9 } else { 10 };
            for _ in 0..kills {
                let beast = t.spawn(id, 60);
                t.kill(1, beast);
            }
        }
        let writes = t.take_writes();
        assert_eq!(writes.last(), Some(&rows::update(NAME, "cond", "2")));
        assert!(quest_page(&mut t, kelleyia).contains("32768-13.html"));
    }

    #[test]
    fn the_results_show_the_number_and_the_reward_is_a_recipe() {
        let (mut t, kelleyia) = setup();
        t.enter(1, 82, vec![]);
        accept(&mut t, kelleyia);
        // The overhits of a finished hunt (the test world has no HP to overhit with).
        set_var(&mut t, "overhitsTotal", 36);
        set_var(&mut t, "overhitsCritical", 30);
        set_var(&mut t, "overhitsConsecutive", 20);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 2, true);
        });
        t.take_writes();

        assert!(quest_page(&mut t, kelleyia).contains("32768-13.html"));
        let results1 = t.event(1, NAME, "results1");
        assert!(
            results1.contains("32768-14a.html") && results1.contains("36"),
            "{results1}"
        );
        assert!(!results1.contains("<?number?>"));
        assert!(t.event(1, NAME, "results2").contains("30"));
        assert!(t.event(1, NAME, "results3").contains("20"));
        assert!(t.event(1, NAME, "32768-17.html").contains("32768-17.html"));

        // At 20 consecutive overhits: one Icarus weapon recipe, then the daily quest is done.
        let recipes: i64 = ICARUS_WEAPON_RECIPES.iter().map(|id| t.count(1, *id)).sum();
        assert_eq!(recipes, 1);
        assert_eq!(t.count(1, GOLDEN_SPICE_CRATE), 0);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                QuestWrite::DeleteQuest {
                    char_id: 1,
                    name: NAME.to_owned(),
                    repeatable: false,
                },
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
        assert!(quest_page(&mut t, kelleyia).contains("32768-18.htm"));
    }

    #[test]
    fn a_few_overhits_give_the_crates_and_pieces() {
        let (mut t, kelleyia) = setup();
        t.enter(1, 82, vec![]);
        accept(&mut t, kelleyia);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 2, true);
        });
        set_var(&mut t, "overhitsConsecutive", 3);
        t.take_writes();
        assert!(quest_page(&mut t, kelleyia).contains("32768-13.html"));
        t.event(1, NAME, "results1");
        t.event(1, NAME, "results2");
        t.event(1, NAME, "results3");
        assert!(t.event(1, NAME, "32768-17.html").contains("32768-17.html"));

        // Fewer than 7: two pieces and ten of each crate.
        let pieces: i64 = ICARUS_WEAPON_PIECES.iter().map(|id| t.count(1, *id)).sum();
        assert_eq!(pieces, 2);
        assert_eq!(t.count(1, GOLDEN_SPICE_CRATE), 10);
        assert_eq!(t.count(1, CRYSTAL_SPICE_CRATE), 10);
    }
}
