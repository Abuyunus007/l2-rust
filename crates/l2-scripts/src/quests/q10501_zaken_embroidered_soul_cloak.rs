//! Quest 10501: Zaken Embroidered Soul Cloak (Olf Adams in the Gludin Harbor area, Zaken).
// Java: dist/game/data/scripts/quests/Q10501_ZakenEmbroideredSoulCloak/Q10501_ZakenEmbroideredSoulCloak.java
//
// Pattern: a kill-count quest with a party (`docs/design/scripts.md`, "Kills, drops and
// parties"): `on_kill` runs `actionForEachPlayer` for the killer's party and command channel;
// each member gets 1..3 fragments (the last ones up to 20) and the NPC gives the cloak.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const OLF_ADAMS: i32 = 32612;

// Monster
const ZAKEN: i32 = 29181;

// Items
const ZAKENS_SOUL_FRAGMENT: i32 = 21722;
const SOUL_CLOAK_OF_ZAKEN: i32 = 21719;

// Misc
const MIN_LEVEL: i32 = 78;
const FRAGMENT_COUNT: i64 = 20;

/// Java `Q10501_ZakenEmbroideredSoulCloak`.
pub struct Q10501ZakenEmbroideredSoulCloak;

impl Script for Q10501ZakenEmbroideredSoulCloak {
    fn name(&self) -> &'static str {
        "Q10501_ZakenEmbroideredSoulCloak"
    }

    fn path(&self) -> &'static str {
        "quests/Q10501_ZakenEmbroideredSoulCloak"
    }

    fn id(&self) -> i32 {
        10501
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[OLF_ADAMS]);
        s.add_talk_id(&[OLF_ADAMS]);
        s.add_kill_id(&[ZAKEN]);
        s.register_quest_items(&[ZAKENS_SOUL_FRAGMENT]);
    }

    fn action_for_each_player(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        _is_summon: bool,
    ) {
        let qs = ctx.get_quest_state(player, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false)
        {
            let current_count = ctx.get_quest_items_count(player, ZAKENS_SOUL_FRAGMENT);
            let count = i64::from(ctx.get_random_range(1, 3));
            if count >= (FRAGMENT_COUNT - current_count) {
                ctx.give_items(player, ZAKENS_SOUL_FRAGMENT, FRAGMENT_COUNT - current_count);
                qs.set_cond(ctx, 2, true);
            } else {
                ctx.give_items(player, ZAKENS_SOUL_FRAGMENT, count);
                ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
            }
        }
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
        if ctx.player_level(player) >= MIN_LEVEL && event == "32612-04.html" {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {
        ctx.execute_for_each_player(killer, npc, is_summon, true, true);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "32612-02.html"
                } else {
                    "32612-01.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "32612-05.html".to_owned(),
                2 => {
                    if ctx.get_quest_items_count(player, ZAKENS_SOUL_FRAGMENT) >= FRAGMENT_COUNT {
                        ctx.give_items(player, SOUL_CLOAK_OF_ZAKEN, 1);
                        ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                        qs.exit_quest(ctx, false, true);
                        htmltext = "32612-06.html".to_owned();
                    }
                }
                _ => {}
            },
            COMPLETED => htmltext = "32612-03.html".to_owned(),
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
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q10501_ZakenEmbroideredSoulCloak";

    const NPCS: &str = r#"
        <npc id="32612" type="Folk" level="70" name="Olf Adams"/>
        <npc id="29181" type="RaidBoss" level="78" name="Zaken"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="21722" type="EtcItem" name="Zaken's Soul Fragment"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="21719" type="Armor" name="Soul Cloak of Zaken"><set name="bodypart" val="back"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10501ZakenEmbroideredSoulCloak);
        let pages = Pages::of(&Q10501ZakenEmbroideredSoulCloak)
            .page("32612-01.htm", &["32612-04.html"])
            .plain(&[
                "32612-02.html",
                "32612-03.html",
                "32612-04.html",
                "32612-05.html",
                "32612-06.html",
            ])
            .npcs(&[OLF_ADAMS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let olf = t.spawn(OLF_ADAMS, 20);
        (t, olf)
    }

    /// Kills Zaken next to the killer until the fragments are full; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..200 {
            let zaken = t.spawn(ZAKEN, 60);
            t.kill(1, zaken);
            if t.count(1, ZAKENS_SOUL_FRAGMENT) >= FRAGMENT_COUNT {
                return kills;
            }
        }
        panic!("no 20 fragments after 200 kills");
    }

    #[test]
    fn a_character_under_78_is_sent_away() {
        let (mut t, olf) = setup();
        t.enter(1, 77, vec![]);
        assert!(t.quest_page(1, olf).contains("32612-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn start_then_twenty_fragments_then_the_cloak() {
        let (mut t, olf) = setup();
        t.enter(1, 78, vec![]);
        assert!(t.quest_page(1, olf).contains("32612-01.htm"));
        assert!(t.event(1, NAME, "32612-04.html").contains("32612-04.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, olf).contains("32612-05.html"));

        // A kill out of range of the killer gives nothing.
        let far = t
            .world
            .spawn_npc_at(
                ZAKEN,
                crate::testing::X + 3000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(t.count(1, ZAKENS_SOUL_FRAGMENT), 0);

        // One to three fragments a kill; the last kill sets cond 2 (no sound of its own).
        let kills = hunt(&mut t);
        assert!(kills >= 5, "{kills}");
        assert_eq!(t.count(1, ZAKENS_SOUL_FRAGMENT), 20);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        t.take();

        // The cloak; the fragments go with the quest, the quest ends.
        t.talk(1, olf);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert!(
            pages_to(&out, 1)
                .last()
                .is_some_and(|p| p.contains("32612-06.html"))
        );
        assert_eq!(t.count(1, SOUL_CLOAK_OF_ZAKEN), 1);
        assert_eq!(t.count(1, ZAKENS_SOUL_FRAGMENT), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );

        // Done: the NPC says so.
        assert!(t.quest_page(1, olf).contains("32612-03.html"));
    }
}
