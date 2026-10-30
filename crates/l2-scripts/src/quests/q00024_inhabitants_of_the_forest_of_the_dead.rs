//! Quest 24: Inhabitants of the Forest of the Dead (Dorian, the maid of Lidia, the wizard, a tombstone).
// Java: dist/game/data/scripts/quests/Q00024_InhabitantsOfTheForestOfTheDead/Q00024_InhabitantsOfTheForestOfTheDead.java
//
// Pattern: an event-heavy chain with the page picked by the quest variable `var`
// (`docs/design/scripts.md`, "Quest patterns"). `onEvent` echoes the event by default and Java's
// `default` branch answers null.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const DORIAN: i32 = 31389;
const MYSTERIOUS_WIZARD: i32 = 31522;
const TOMBSTONE: i32 = 31531;
const LIDIA_MAID: i32 = 31532;

// Items
const LIDIA_LETTER: i32 = 7065;
const LIDIA_HAIRPIN: i32 = 7148;
const SUSPICIOUS_TOTEM_DOLL: i32 = 7151;
const FLOWER_BOUQUET: i32 = 7152;
const SILVER_CROSS_OF_EINHASAD: i32 = 7153;
const BROKEN_SILVER_CROSS_OF_EINHASAD: i32 = 7154;
const TOTEM: i32 = 7156;

// Monsters
const MOBS: [i32; 8] = [21557, 21558, 21560, 21563, 21564, 21565, 21566, 21567];

/// Java `Q00023_LidiasHeart.class.getSimpleName()`.
const LIDIAS_HEART: &str = "Q00023_LidiasHeart";
/// Java `Q00025_HidingBehindTheTruth.class.getSimpleName()`.
const HIDING_BEHIND_THE_TRUTH: &str = "Q00025_HidingBehindTheTruth";

/// Java `Q00024_InhabitantsOfTheForestOfTheDead`.
pub struct Q00024InhabitantsOfTheForestOfTheDead;

impl Script for Q00024InhabitantsOfTheForestOfTheDead {
    fn name(&self) -> &'static str {
        "Q00024_InhabitantsOfTheForestOfTheDead"
    }

    fn path(&self) -> &'static str {
        "quests/Q00024_InhabitantsOfTheForestOfTheDead"
    }

    fn id(&self) -> i32 {
        24
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DORIAN]);
        s.add_talk_id(&[DORIAN, MYSTERIOUS_WIZARD, TOMBSTONE, LIDIA_MAID]);
        s.add_kill_id(&MOBS);
        s.register_quest_items(&[
            LIDIA_LETTER,
            LIDIA_HAIRPIN,
            SUSPICIOUS_TOTEM_DOLL,
            FLOWER_BOUQUET,
            SILVER_CROSS_OF_EINHASAD,
            BROKEN_SILVER_CROSS_OF_EINHASAD,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let st = ctx.get_quest_state(player, false)?;

        let mut htmltext = Some(event.to_owned());
        match event {
            // Dorian
            "31389-02.htm" => {
                let q23_completed = ctx
                    .player_quest_state(player, LIDIAS_HEART)
                    .is_some_and(|q| q.is_completed());
                if ctx.player_level(player) >= 65 && q23_completed {
                    st.start_quest(ctx);
                    ctx.give_items(player, FLOWER_BOUQUET, 1);
                    return Some("31389-03.htm".to_owned());
                }
            }
            "31389-08.html" => st.set(ctx, "var", "1"),
            "31389-13.html" => {
                ctx.give_items(player, SILVER_CROSS_OF_EINHASAD, 1);
                st.set_cond(ctx, 3, true);
                st.unset(ctx, "var");
            }
            "31389-18.html" => ctx.play_sound(player, QuestSound::InterfacesoundCharstatOpen),
            "31389-19.html" => {
                if !ctx.has_quest_items(player, BROKEN_SILVER_CROSS_OF_EINHASAD) {
                    return Some(ctx.get_no_quest_msg(player));
                }
                ctx.take_items(player, BROKEN_SILVER_CROSS_OF_EINHASAD, -1);
                st.set_cond(ctx, 5, true);
            }
            "31389-06.html" | "31389-07.html" | "31389-10.html" | "31389-11.html" | "31389-12.html"
            | "31389-16.html" | "31389-17.html" => {}
            // Lidia Maid
            "31532-04.html" => {
                ctx.give_items(player, LIDIA_LETTER, 1);
                st.set_cond(ctx, 6, true);
            }
            "31532-07.html" => {
                if st.is_cond(ctx, 8) {
                    if !ctx.has_all_quest_items(player, &[LIDIA_HAIRPIN, LIDIA_LETTER]) {
                        return Some(ctx.get_no_quest_msg(player));
                    }
                    ctx.take_items(player, LIDIA_HAIRPIN, -1);
                    ctx.take_items(player, LIDIA_LETTER, -1);
                    st.set(ctx, "var", "1");
                    htmltext = Some("31532-06.html".to_owned());
                } else if st.is_cond(ctx, 6) {
                    st.set_cond(ctx, 7, true);
                }
            }
            "31532-10.html" => st.set(ctx, "var", "2"),
            "31532-14.html" => st.set(ctx, "var", "3"),
            "31532-19.html" => {
                st.unset(ctx, "var");
                st.set_cond(ctx, 9, true);
            }
            "31532-02.html" | "31532-03.html" | "31532-09.html" | "31532-12.html" | "31532-13.html"
            | "31532-15.html" | "31532-16.html" | "31532-17.html" | "31532-18.html" => {}
            // Mysterious Wizard
            "31522-03.html" => {
                if !ctx.has_quest_items(player, SUSPICIOUS_TOTEM_DOLL) {
                    return Some(ctx.get_no_quest_msg(player));
                }
                ctx.take_items(player, SUSPICIOUS_TOTEM_DOLL, 1);
                st.set(ctx, "var", "1");
            }
            "31522-08.html" => {
                st.unset(ctx, "var");
                st.set_cond(ctx, 11, true);
            }
            "31522-17.html" => st.set(ctx, "var", "1"),
            "31522-21.html" => {
                ctx.give_items(player, TOTEM, 1);
                ctx.add_exp_and_sp(player, 242_105, 22_529); // GoD: Harmony: 6191140 exp and 6118650 sp
                st.exit_quest(ctx, false, true);
            }
            "31522-02.html" | "31522-05.html" | "31522-06.html" | "31522-07.html" | "31522-10.html"
            | "31522-11.html" | "31522-12.html" | "31522-13.html" | "31522-14.html" | "31522-15.html"
            | "31522-16.html" | "31522-19.html" | "31522-20.html" => {}
            // Tombstone
            "31531-02.html" => {
                if !ctx.has_quest_items(player, FLOWER_BOUQUET) {
                    return Some(ctx.get_no_quest_msg(player));
                }
                ctx.take_items(player, FLOWER_BOUQUET, -1);
                st.set_cond(ctx, 2, true);
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let st = ctx.get_quest_state(player, false);
        if let Some(st) = st
            && st.is_cond(ctx, 9)
            && ctx.get_random(100) < 10
        {
            ctx.give_items(player, SUSPICIOUS_TOTEM_DOLL, 1);
            st.set_cond(ctx, 10, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let st = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            DORIAN => match st.state(ctx) {
                CREATED => htmltext = "31389-01.htm".to_owned(),
                STARTED => match st.get_cond(ctx) {
                    1 => htmltext = "31389-04.html".to_owned(),
                    2 => {
                        htmltext = if st.get_int(ctx, "var") == 0 {
                            "31389-05.html"
                        } else {
                            "31389-09.html"
                        }
                        .to_owned();
                    }
                    3 => htmltext = "31389-14.html".to_owned(),
                    4 => htmltext = "31389-15.html".to_owned(),
                    5 => htmltext = "31389-20.html".to_owned(),
                    6 | 8 => htmltext = "31389-22.html".to_owned(),
                    7 => {
                        ctx.give_items(player, LIDIA_HAIRPIN, 1);
                        st.set_cond(ctx, 8, true);
                        htmltext = "31389-21.html".to_owned();
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            MYSTERIOUS_WIZARD => {
                if st.is_started(ctx) {
                    if st.is_cond(ctx, 10) {
                        htmltext = if st.get_int(ctx, "var") == 0 {
                            "31522-01.html"
                        } else {
                            "31522-04.html"
                        }
                        .to_owned();
                    } else if st.is_cond(ctx, 11) {
                        htmltext = if st.get_int(ctx, "var") == 0 {
                            "31522-09.html"
                        } else {
                            "31522-18.html"
                        }
                        .to_owned();
                    }
                } else if st.is_completed(ctx) {
                    let q25_started = ctx
                        .player_quest_state(player, HIDING_BEHIND_THE_TRUTH)
                        .is_some_and(|q| q.is_started());
                    if !q25_started {
                        htmltext = "31522-22.html".to_owned();
                    }
                }
            }
            TOMBSTONE => {
                if st.is_started(ctx) {
                    if st.is_cond(ctx, 1) {
                        ctx.play_sound(player, QuestSound::AmdsoundWindLoot);
                        htmltext = "31531-01.html".to_owned();
                    } else if st.is_cond(ctx, 2) {
                        htmltext = "31531-03.html".to_owned();
                    }
                }
            }
            LIDIA_MAID => {
                if st.is_started(ctx) {
                    match st.get_cond(ctx) {
                        5 => htmltext = "31532-01.html".to_owned(),
                        6 => htmltext = "31532-05.html".to_owned(),
                        7 => htmltext = "31532-07a.html".to_owned(),
                        8 => match st.get_int(ctx, "var") {
                            0 => htmltext = "31532-07a.html".to_owned(),
                            1 => htmltext = "31532-08.html".to_owned(),
                            2 => htmltext = "31532-11.html".to_owned(),
                            3 => htmltext = "31532-15.html".to_owned(),
                            _ => {}
                        },
                        9 | 10 => htmltext = "31532-20.html".to_owned(),
                        _ => {}
                    }
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00024_InhabitantsOfTheForestOfTheDead";

    const NPCS: &str = r#"
        <npc id="31389" type="Folk" level="70" name="Dorian"/>
        <npc id="31522" type="Folk" level="70" name="Mysterious Wizard"/>
        <npc id="31531" type="Folk" level="70" name="Tombstone"/>
        <npc id="31532" type="Folk" level="70" name="Lidia's Maid"/>
        <npc id="21557" type="Monster" level="60" name="Mob"/>"#;

    const ITEMS: &str = r#"
        <item id="7065" type="EtcItem" name="Lidia's Letter"><set name="is_questitem" val="true"/></item>
        <item id="7148" type="EtcItem" name="Lidia's Hairpin"><set name="is_questitem" val="true"/></item>
        <item id="7151" type="EtcItem" name="Doll"><set name="is_questitem" val="true"/></item>
        <item id="7152" type="EtcItem" name="Bouquet"><set name="is_questitem" val="true"/></item>
        <item id="7153" type="EtcItem" name="Cross"><set name="is_questitem" val="true"/></item>
        <item id="7154" type="EtcItem" name="Broken Cross"><set name="is_questitem" val="true"/></item>
        <item id="7156" type="EtcItem" name="Totem"/>"#;

    /// A stand-in for a quest this one reads.
    struct Other(&'static str, i32);

    impl Script for Other {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests/Other"
        }
        fn id(&self) -> i32 {
            self.1
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Other(LIDIAS_HEART, 23));
        registry.add(Q00024InhabitantsOfTheForestOfTheDead);
        let pages = Pages::of(&Q00024InhabitantsOfTheForestOfTheDead)
            .page("31389-01.htm", &["31389-02.htm"])
            .plain(&["31389-02.htm", "31389-03.htm"])
            .npcs(&[DORIAN, MYSTERIOUS_WIZARD, TOMBSTONE, LIDIA_MAID]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let dorian = t.spawn(DORIAN, 20);
        (t, dorian)
    }

    fn finish_lidias_heart(t: &mut ScriptTest) {
        t.world.run_as_script(LIDIAS_HEART, |ctx| {
            if let Some(qs) = ctx.quest_state_of(LIDIAS_HEART, Oid(1), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
        t.take_writes();
    }

    fn ev(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00024InhabitantsOfTheForestOfTheDead.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    fn talk(t: &mut ScriptTest, npc: Oid) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(npc) {
                out = Q00024InhabitantsOfTheForestOfTheDead.on_talk(ctx, npc, Oid(1));
            }
        });
        out
    }

    fn writes(t: &ScriptTest) -> Vec<l2_world::QuestWrite> {
        t.take_writes()
            .into_iter()
            .filter(|w| !format!("{w:?}").contains("__compltdStateFlags"))
            .collect()
    }

    #[test]
    fn it_needs_level_65_and_lidias_heart() {
        let (mut t, dorian) = setup();
        t.enter(1, 65, vec![]);
        assert!(t.quest_page(1, dorian).contains("31389-01.htm"));
        // No Q23: the event is echoed and nothing starts.
        assert!(t.event(1, NAME, "31389-02.htm").contains("31389-02.htm"));
        assert!(writes(&t).is_empty());

        let (mut t, dorian) = setup();
        t.enter(1, 64, vec![]);
        finish_lidias_heart(&mut t);
        t.quest_page(1, dorian);
        assert!(t.event(1, NAME, "31389-02.htm").contains("31389-02.htm"));
        assert!(writes(&t).is_empty());
    }

    #[test]
    fn the_whole_chain_and_the_reward() {
        let (mut t, dorian) = setup();
        t.enter(1, 65, vec![]);
        finish_lidias_heart(&mut t);
        t.quest_page(1, dorian);
        assert!(t.event(1, NAME, "31389-02.htm").contains("31389-03.htm"));
        assert_eq!(t.count(1, FLOWER_BOUQUET), 1);
        assert_eq!(
            writes(&t),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );

        // The tombstone takes the bouquet: cond 2; the var pages of Dorian.
        let tomb = t.spawn(TOMBSTONE, 40);
        assert_eq!(talk(&mut t, tomb).as_deref(), Some("31531-01.html"));
        assert_eq!(ev(&mut t, "31531-02.html").as_deref(), Some("31531-02.html"));
        assert_eq!(t.count(1, FLOWER_BOUQUET), 0);
        assert_eq!(talk(&mut t, dorian).as_deref(), Some("31389-05.html"));
        assert_eq!(ev(&mut t, "31389-08.html").as_deref(), Some("31389-08.html"));
        assert_eq!(talk(&mut t, dorian).as_deref(), Some("31389-09.html"));
        assert_eq!(ev(&mut t, "31389-13.html").as_deref(), Some("31389-13.html"));
        assert_eq!(t.count(1, SILVER_CROSS_OF_EINHASAD), 1);
        assert_eq!(talk(&mut t, dorian).as_deref(), Some("31389-14.html"));
        writes(&t);

        // The broken cross: refused without it, cond 5 with it.
        assert_ne!(ev(&mut t, "31389-19.html").as_deref(), Some("31389-19.html"));
        assert_eq!(talk(&mut t, dorian).as_deref(), Some("31389-14.html"));
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), BROKEN_SILVER_CROSS_OF_EINHASAD, 1)
        });
        assert_eq!(ev(&mut t, "31389-19.html").as_deref(), Some("31389-19.html"));
        assert_eq!(talk(&mut t, dorian).as_deref(), Some("31389-20.html"));

        // The maid: the letter (cond 6), cond 7, Dorian's hairpin (cond 8), then the var pages.
        let maid = t.spawn(LIDIA_MAID, 60);
        assert_eq!(ev(&mut t, "31532-04.html").as_deref(), Some("31532-04.html"));
        assert_eq!(ev(&mut t, "31532-07.html").as_deref(), Some("31532-07.html"));
        assert_eq!(talk(&mut t, dorian).as_deref(), Some("31389-21.html"));
        assert_eq!(ev(&mut t, "31532-07.html").as_deref(), Some("31532-06.html"));
        assert_eq!(talk(&mut t, maid).as_deref(), Some("31532-08.html"));
        assert_eq!(ev(&mut t, "31532-19.html").as_deref(), Some("31532-19.html"));
        assert_eq!(talk(&mut t, maid).as_deref(), Some("31532-20.html"));
        writes(&t);

        // A mob drops the doll (10% roll): cond 10; the wizard pays.
        for _ in 0..500 {
            if t.count(1, SUSPICIOUS_TOTEM_DOLL) > 0 {
                break;
            }
            let mob = t.spawn(21557, 80);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, SUSPICIOUS_TOTEM_DOLL), 1);
        let wizard = t.spawn(MYSTERIOUS_WIZARD, 100);
        assert_eq!(talk(&mut t, wizard).as_deref(), Some("31522-01.html"));
        assert_eq!(ev(&mut t, "31522-03.html").as_deref(), Some("31522-03.html"));
        assert_eq!(talk(&mut t, wizard).as_deref(), Some("31522-04.html"));
        assert_eq!(ev(&mut t, "31522-08.html").as_deref(), Some("31522-08.html"));
        assert_eq!(talk(&mut t, wizard).as_deref(), Some("31522-09.html"));
        writes(&t);
        assert_eq!(ev(&mut t, "31522-21.html").as_deref(), Some("31522-21.html"));
        assert_eq!(t.count(1, TOTEM), 1);
        assert_eq!(
            writes(&t),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert_eq!(talk(&mut t, wizard).as_deref(), Some("31522-22.html"));
        // An unknown event answers null.
        assert_eq!(ev(&mut t, "nothing"), None);
    }
}
