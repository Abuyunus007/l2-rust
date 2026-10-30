//! Quest 247: Possessor of a Precious Soul 4 (Caradine in Gludio's Ruins sends a sub-class
//! character to the Lady of the Lake, who makes it a noble).
// Java: dist/game/data/scripts/quests/Q00247_PossessorOfAPreciousSoul4/Q00247_PossessorOfAPreciousSoul4.java
//
// Pattern: a sub-class quest (`ctx.is_sub_class_active`, `no_sub.html`), a teleport at cond 1 and
// a one-time reward (`docs/design/scripts.md`, "Quest patterns").

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const CARADINE: i32 = 31740;
const LADY_OF_LAKE: i32 = 31745;

// Items
const CARADINE_LETTER_LAST: i32 = 7679;
const NOBLESS_TIARA: i32 = 7694;

// Location: Caradine's teleport
const CARADINE_LOC: (i32, i32, i32) = (143209, 43968, -3038);

// Skill: Mimir's Elixir (the noble cast)
const MIMIRS_ELIXIR: SkillKey = SkillKey { id: 4339, level: 1 };

/// Java `Q00247_PossessorOfAPreciousSoul4`.
pub struct Q00247PossessorOfAPreciousSoul4;

impl Script for Q00247PossessorOfAPreciousSoul4 {
    fn name(&self) -> &'static str {
        "Q00247_PossessorOfAPreciousSoul4"
    }

    fn path(&self) -> &'static str {
        "quests/Q00247_PossessorOfAPreciousSoul4"
    }

    fn id(&self) -> i32 {
        247
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[CARADINE]);
        s.add_talk_id(&[CARADINE, LADY_OF_LAKE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(st) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };
        if !ctx.is_sub_class_active(player) {
            return Some("no_sub.html".to_owned());
        }
        match event {
            "31740-3.html" => {
                st.start_quest(ctx);
                ctx.take_items(player, CARADINE_LETTER_LAST, -1);
            }
            "TELEPORT" => {
                if st.is_cond(ctx, 1) {
                    st.set_cond(ctx, 2, true);
                    let (x, y, z) = CARADINE_LOC;
                    ctx.teleport_player(player, x, y, z, 0, false);
                }
            }
            "31745-5.html" => {
                if st.is_cond(ctx, 2) {
                    ctx.set_noble(player, true);
                    ctx.add_exp_and_sp(player, 93836, 0);
                    ctx.give_items(player, NOBLESS_TIARA, 1);
                    // Java `npc.setTarget(player); npc.doCast(MIMIRS_ELIXIR)`: the bypass always
                    // has the Lady of the Lake.
                    if let Some(npc) = npc {
                        ctx.cast_skill(npc, player, MIMIRS_ELIXIR);
                    }
                    // `player.sendPacket(new SocialAction(player.getObjectId(), 3))`.
                    ctx.send_social_action(player, 3);
                    st.exit_quest(ctx, false, true);
                }
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let st = ctx.get_quest_state(player, true)?;
        if st.is_started(ctx) && !ctx.is_sub_class_active(player) {
            return Some("no_sub.html".to_owned());
        }

        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            CARADINE => match st.state(ctx) {
                CREATED => {
                    let third_done = ctx
                        .player_quest_state(player, "Q00246_PossessorOfAPreciousSoul3")
                        .is_some_and(|qs| qs.is_completed());
                    if third_done {
                        htmltext = if ctx.player_level(player) >= 75 {
                            "31740-1.htm"
                        } else {
                            "31740-2.html"
                        }
                        .to_owned();
                    }
                }
                STARTED => {
                    if st.is_cond(ctx, 1) {
                        htmltext = "31740-6.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            LADY_OF_LAKE => {
                if st.is_cond(ctx, 2) {
                    htmltext = "31745-1.html".to_owned();
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
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q00247_PossessorOfAPreciousSoul4";
    const THIRD: &str = "Q00246_PossessorOfAPreciousSoul3";

    const NPCS: &str = r#"
        <npc id="31740" type="Folk" level="70" name="Caradine"/>
        <npc id="31745" type="Folk" level="70" name="Lady of the Lake"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7679" type="EtcItem" name="Caradine's Letter - Last"><set name="is_questitem" val="true"/></item>
        <item id="7694" type="Armor" name="Nobless Tiara"><set name="bodypart" val="hair"/></item>"#;

    /// Java `Q00246_PossessorOfAPreciousSoul3`, as far as this quest reads it: a script of that
    /// name whose state a test completes.
    struct ThirdSoul;

    impl Script for ThirdSoul {
        fn name(&self) -> &'static str {
            THIRD
        }
        fn path(&self) -> &'static str {
            "quests/Q00246_PossessorOfAPreciousSoul3"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(ThirdSoul);
        registry.add(Q00247PossessorOfAPreciousSoul4);
        let pages = Pages::of(&Q00247PossessorOfAPreciousSoul4)
            .page("31740-1.htm", &["31740-3.html"])
            .page("31740-3.html", &["31740-4.html"])
            .page("31740-4.html", &["31740-5.html"])
            .page("31740-6.html", &["TELEPORT"])
            .plain(&["31740-2.html", "31740-5.html", "no_sub.html"])
            .page("31745-1.html", &["31745-2.html"])
            .page("31745-2.html", &["31745-3.html"])
            .page("31745-3.html", &["31745-4.html"])
            .page("31745-4.html", &["31745-5.html"])
            .plain(&["31745-5.html"])
            .npcs(&[CARADINE, LADY_OF_LAKE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let caradine = t.spawn(CARADINE, 20);
        (t, caradine)
    }

    /// Enters a character of `level` whose sub class is active (`class_index` > 0).
    fn enter_sub(t: &mut ScriptTest, id: i32, level: i32, items: Vec<l2_world::ItemData>) {
        let mut data = player_data(id, level, items);
        data.profile.class_index = 1;
        t.enter_with(data);
    }

    /// Completes the third soul quest of `player` (the state Java reads through `getQuestState`).
    fn complete_third(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(THIRD, |ctx| {
            if let Some(qs) = ctx.quest_state_of(THIRD, Oid(player), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
        t.take_writes();
    }

    #[test]
    fn only_a_completed_third_soul_is_offered_this_quest() {
        let (mut t, caradine) = setup();
        enter_sub(&mut t, 1, 75, vec![]);
        assert!(!t.quest_page(1, caradine).contains("31740-1.htm"));
        complete_third(&mut t, 1);
        assert!(t.quest_page(1, caradine).contains("31740-1.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_character_below_level_75_is_sent_away() {
        let (mut t, caradine) = setup();
        enter_sub(&mut t, 1, 74, vec![]);
        complete_third(&mut t, 1);
        assert!(t.quest_page(1, caradine).contains("31740-2.html"));
    }

    #[test]
    fn the_quest_needs_an_active_sub_class() {
        let (mut t, caradine) = setup();
        t.enter(1, 75, vec![]);
        complete_third(&mut t, 1);
        assert!(t.quest_page(1, caradine).contains("31740-1.htm"));
        assert!(t.event(1, NAME, "31740-3.html").contains("no_sub.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_teleport_and_the_tiara() {
        let (mut t, caradine) = setup();
        enter_sub(&mut t, 1, 75, vec![item(10, CARADINE_LETTER_LAST, 1)]);
        complete_third(&mut t, 1);
        assert!(t.quest_page(1, caradine).contains("31740-1.htm"));
        assert!(t.event(1, NAME, "31740-3.html").contains("31740-3.html"));
        assert!(t.event(1, NAME, "31740-4.html").contains("31740-4.html"));
        assert!(t.event(1, NAME, "31740-5.html").contains("31740-5.html"));
        assert_eq!(t.count(1, CARADINE_LETTER_LAST), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        assert!(t.quest_page(1, caradine).contains("31740-6.html"));
        // A bypass that is not a page is a chat message, so no page comes back.
        t.event(1, NAME, "TELEPORT");
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        // The height comes from the world (no geodata in the test), so only x and y are compared.
        assert_eq!(
            t.world.position_of(Oid(1)).map(|(x, y, _)| (x, y)),
            Some((143209, 43968))
        );
    }

    #[test]
    fn the_lady_of_the_lake_gives_the_tiara_and_ends_the_quest() {
        let (mut t, caradine) = setup();
        enter_sub(&mut t, 1, 75, vec![]);
        complete_third(&mut t, 1);
        t.quest_page(1, caradine);
        t.event(1, NAME, "31740-3.html");
        t.quest_page(1, caradine);
        t.event(1, NAME, "TELEPORT");
        t.take_writes();

        // The Lady stands where the teleport put the player.
        let lady = t
            .world
            .spawn_npc_at(LADY_OF_LAKE, 143209, 43968, -3038, 0)
            .unwrap();
        assert!(t.quest_page(1, lady).contains("31745-1.html"));
        assert!(t.event(1, NAME, "31745-2.html").contains("31745-2.html"));
        assert!(t.event(1, NAME, "31745-3.html").contains("31745-3.html"));
        assert!(t.event(1, NAME, "31745-4.html").contains("31745-4.html"));
        assert!(t.event(1, NAME, "31745-5.html").contains("31745-5.html"));
        assert_eq!(t.count(1, NOBLESS_TIARA), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
