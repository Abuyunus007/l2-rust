//! Quest 603: Daimon the White-Eyed, Part 1 (Eye of Argos, the five tablets, the Spirit of Darkness).
// Java: dist/game/data/scripts/quests/Q00603_DaimonTheWhiteEyedPart1/Q00603_DaimonTheWhiteEyedPart1.java
//
// Pattern: a tablet chain. Each tablet gives a broken crystal once (its variable says so), the
// eye sends the player through five of them, then the monsters drop spirits of darkness for a
// random party member at cond 7 (`getRandomPartyMemberState`).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const EYE_OF_ARGOS: i32 = 31683;
const TABLET_1: i32 = 31548;
const TABLET_2: i32 = 31549;
const TABLET_3: i32 = 31550;
const TABLET_4: i32 = 31551;
const TABLET_5: i32 = 31552;

// Items
const SPIRIT_OF_DARKNESS: i32 = 7190;
const BROKEN_CRYSTAL: i32 = 7191;

// Monsters: (id, drop chance)
const MONSTER_CHANCES: [(i32, f64); 3] = [(21297, 0.5), (21299, 0.519), (21304, 0.673)];

// Reward
const UNFINISHED_CRYSTAL: i32 = 7192;

// Misc
const MIN_LEVEL: i32 = 73;

/// Java `Q00603_DaimonTheWhiteEyedPart1`.
pub struct Q00603DaimonTheWhiteEyedPart1;

impl Script for Q00603DaimonTheWhiteEyedPart1 {
    fn name(&self) -> &'static str {
        "Q00603_DaimonTheWhiteEyedPart1"
    }

    fn path(&self) -> &'static str {
        "quests/Q00603_DaimonTheWhiteEyedPart1"
    }

    fn id(&self) -> i32 {
        603
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[EYE_OF_ARGOS]);
        s.add_talk_id(&[EYE_OF_ARGOS, TABLET_1, TABLET_2, TABLET_3, TABLET_4, TABLET_5]);
        let mobs: Vec<i32> = MONSTER_CHANCES.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[SPIRIT_OF_DARKNESS, BROKEN_CRYSTAL]);
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
            "31683-03.htm" => {
                if qs.is_created(ctx) {
                    // NOTE(java-quirk): the tablet variables are written in lower case here and
                    // read/set in upper case below (`"TABLET_" + id`), so the start's zeros are
                    // not the ones the tablets read. Both spellings are kept as Java has them.
                    qs.set_int(ctx, "tablet_31548", 0);
                    qs.set_int(ctx, "tablet_31549", 0);
                    qs.set_int(ctx, "tablet_31550", 0);
                    qs.set_int(ctx, "tablet_31551", 0);
                    qs.set_int(ctx, "tablet_31552", 0);
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "31548-02.html" | "31549-02.html" | "31550-02.html" | "31551-02.html" | "31552-02.html" => {
                if qs.get_cond(ctx) < 6 {
                    ctx.give_items(player, BROKEN_CRYSTAL, 1);
                    if let Some(npc) = npc {
                        qs.set_int(ctx, &format!("TABLET_{}", npc.id), 1);
                    }
                    let cond = qs.get_cond(ctx);
                    qs.set_cond(ctx, cond + 1, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31683-06.html" => {
                if qs.is_cond(ctx, 6) && ctx.get_quest_items_count(player, BROKEN_CRYSTAL) >= 5 {
                    ctx.take_items(player, BROKEN_CRYSTAL, -1);
                    qs.set_cond(ctx, 7, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31683-10.html" => {
                if qs.is_cond(ctx, 8) {
                    if ctx.get_quest_items_count(player, SPIRIT_OF_DARKNESS) >= 200 {
                        ctx.take_items(player, SPIRIT_OF_DARKNESS, -1);
                        ctx.give_items(player, UNFINISHED_CRYSTAL, 1);
                        qs.exit_quest(ctx, true, true);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("31683-11.html".to_owned());
                    }
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if let Some(qs) = ctx.random_party_member_state(killer, 7, 3, Some(npc)) {
            let chance = MONSTER_CHANCES
                .iter()
                .find(|(id, _)| *id == npc.id)
                .map_or(0.0, |(_, chance)| *chance);
            if ctx.give_item_randomly(
                qs.player(),
                Some(npc),
                SPIRIT_OF_DARKNESS,
                1,
                1,
                200,
                chance,
                true,
            ) {
                qs.set_cond(ctx, 8, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match qs.state(ctx) {
            CREATED => {
                if npc.id == EYE_OF_ARGOS {
                    htmltext = if ctx.player_level(talker) < MIN_LEVEL {
                        "31683-02.html"
                    } else {
                        "31683-01.htm"
                    }
                    .to_owned();
                }
            }
            STARTED => {
                if npc.id == EYE_OF_ARGOS {
                    match qs.get_cond(ctx) {
                        1..=5 => htmltext = "31683-04.html".to_owned(),
                        6 => htmltext = "31683-05.html".to_owned(),
                        7 => htmltext = "31683-07.html".to_owned(),
                        8 => htmltext = "31683-08.html".to_owned(),
                        _ => {}
                    }
                } else if qs.get_int(ctx, &format!("TABLET_{}", npc.id)) == 0 {
                    htmltext = format!("{}-01.html", npc.id);
                } else {
                    htmltext = format!("{}-03.html", npc.id);
                }
            }
            COMPLETED => htmltext = ctx.get_already_completed_msg(talker),
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00603_DaimonTheWhiteEyedPart1";

    const NPCS: &str = r#"
        <npc id="31683" type="Folk" level="70" name="Eye of Argos"/>
        <npc id="31548" type="Folk" level="70" name="Tablet 1"/>
        <npc id="31549" type="Folk" level="70" name="Tablet 2"/>
        <npc id="31550" type="Folk" level="70" name="Tablet 3"/>
        <npc id="31551" type="Folk" level="70" name="Tablet 4"/>
        <npc id="31552" type="Folk" level="70" name="Tablet 5"/>
        <npc id="21304" type="Monster" level="70" name="Grendel Slave"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7190" type="EtcItem" name="Spirit of Darkness"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7191" type="EtcItem" name="Broken Crystal"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7192" type="EtcItem" name="Unfinished Crystal"/>"#;

    fn setup() -> (ScriptTest, Oid, Vec<Oid>) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00603DaimonTheWhiteEyedPart1);
        let mut pages = Pages::of(&Q00603DaimonTheWhiteEyedPart1)
            .page("31683-01.htm", &["31683-03.htm"])
            .plain(&[
                "31683-02.html",
                "31683-03.htm",
                "31683-04.html",
                "31683-05.html",
                "31683-07.html",
            ])
            .plain(&["31683-08.html", "31683-10.html", "31683-11.html"])
            .page("31683-05.html", &["31683-06.html"])
            .page("31683-06.html", &[])
            .npcs(&[EYE_OF_ARGOS]);
        for tablet in [31548, 31549, 31550, 31551, 31552] {
            let (one, two, three) = (
                format!("{tablet}-01.html"),
                format!("{tablet}-02.html"),
                format!("{tablet}-03.html"),
            );
            pages = pages
                .page(&one, &[two.as_str()])
                .plain(&[two.as_str(), three.as_str()]);
        }
        let pages = pages.npcs(&[31548, 31549, 31550, 31551, 31552]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let eye = t.spawn(EYE_OF_ARGOS, 20);
        let tablets = [31548, 31549, 31550, 31551, 31552]
            .iter()
            .enumerate()
            .map(|(i, id)| t.spawn(*id, 30 + 10 * i as i32))
            .collect();
        (t, eye, tablets)
    }

    #[test]
    fn a_character_below_level_73_is_sent_away() {
        let (mut t, eye, _tablets) = setup();
        t.enter(1, 72, vec![]);
        assert!(t.quest_page(1, eye).contains("31683-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_five_tablets_then_the_spirits_and_the_crystal() {
        let (mut t, eye, tablets) = setup();
        t.enter(1, 73, vec![]);
        assert!(t.quest_page(1, eye).contains("31683-01.htm"));
        assert!(t.event(1, NAME, "31683-03.htm").contains("31683-03.htm"));
        // The five zeros of the tablet variables come before `startQuest()`.
        let writes = t.take_writes();
        assert_eq!(writes.len(), 5 + 2);
        assert_eq!(writes[0], rows::insert(NAME, "tablet_31548", "0"));
        assert_eq!(
            writes[5..],
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, eye).contains("31683-04.html"));

        // Each tablet: the crystal, the variable, the next cond; the talk afterwards says `-03`.
        for (i, tablet) in tablets.iter().enumerate() {
            let id = 31548 + i as i32;
            assert!(t.quest_page(1, *tablet).contains(&format!("{id}-01.html")));
            assert!(
                t.event(1, NAME, &format!("{id}-02.html"))
                    .contains(&format!("{id}-02.html"))
            );
            assert_eq!(t.count(1, BROKEN_CRYSTAL), i as i64 + 1);
            assert!(t.take_writes().len() >= 2);
            assert!(t.quest_page(1, *tablet).contains(&format!("{id}-03.html")));
        }
        t.take();
        assert_eq!(t.count(1, BROKEN_CRYSTAL), 5);

        // The eye at cond 6: the five crystals go, cond 7.
        assert!(t.quest_page(1, eye).contains("31683-05.html"));
        assert!(t.event(1, NAME, "31683-06.html").contains("31683-06.html"));
        assert_eq!(t.count(1, BROKEN_CRYSTAL), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);
    }

    #[test]
    fn the_eye_refuses_the_collection_until_two_hundred_spirits() {
        let (mut t, eye, _tablets) = setup();
        t.enter(1, 73, vec![]);
        t.quest_page(1, eye);
        t.event(1, NAME, "31683-03.htm");
        t.take_writes();
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 8, true);
        });
        t.take_writes();
        assert!(t.quest_page(1, eye).contains("31683-08.html"));
        // Not enough spirits: `31683-11.html`, nothing is taken.
        t.world.run_as_script(NAME, |ctx| {
            let got = Q00603DaimonTheWhiteEyedPart1.on_event(ctx, "31683-10.html", None, Some(Oid(1)));
            assert_eq!(got.as_deref(), Some("31683-11.html"));
        });
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_spirits_come_from_the_monsters_of_the_party_member_at_cond_seven() {
        let (mut t, eye, _tablets) = setup();
        t.enter(1, 73, vec![]);
        t.quest_page(1, eye);
        t.event(1, NAME, "31683-03.htm");
        t.take_writes();
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 7, true);
        });
        t.take_writes();

        // The monsters drop spirits (67%); the 8th cond comes with the 200th spirit.
        for _ in 0..400 {
            if t.count(1, SPIRIT_OF_DARKNESS) >= 200 {
                break;
            }
            let mob = t.spawn(21304, 60);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, SPIRIT_OF_DARKNESS), 200);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "8")));
        assert!(t.quest_page(1, eye).contains("31683-08.html"));
    }
}
