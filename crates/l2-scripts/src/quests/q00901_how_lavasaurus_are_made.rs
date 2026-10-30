//! Quest 901: How Lavasaurus Are Made (Rooney collects the four lavasaurus fragments for a daily totem).
// Java: dist/game/data/scripts/quests/Q00901_HowLavasaurusesAreMade/Q00901_HowLavasaurusesAreMade.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, QuestType, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ROONEY: i32 = 32049;

// Monsters
const LAVASAURUS_NEWBORN: i32 = 18799;
const LAVASAURUS_FLEDGIING: i32 = 18800;
const LAVASAURUS_ADULT: i32 = 18801;
const LAVASAURUS_ELDERLY: i32 = 18802;

// Items
const FRAGMENT_STONE: i32 = 21909;
const FRAGMENT_HEAD: i32 = 21910;
const FRAGMENT_BODY: i32 = 21911;
const FRAGMENT_HORN: i32 = 21912;

// Rewards
const TOTEM_OF_BODY: i32 = 21899;
const TOTEM_OF_SPIRIT: i32 = 21900;
const TOTEM_OF_COURAGE: i32 = 21901;
const TOTEM_OF_FORTITUDE: i32 = 21902;

// Misc
const MIN_LEVEL: i32 = 76;

/// Java `gotAllQuestItems(player)`: ten of each fragment.
fn got_all_quest_items(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.get_quest_items_count(player, FRAGMENT_STONE) >= 10
        && ctx.get_quest_items_count(player, FRAGMENT_HEAD) >= 10
        && ctx.get_quest_items_count(player, FRAGMENT_BODY) >= 10
        && ctx.get_quest_items_count(player, FRAGMENT_HORN) >= 10
}

/// Java `giveQuestItems(qs, itemId)`: one fragment up to ten; the last one sets cond 2.
fn give_quest_items(ctx: &mut ScriptCtx<'_>, qs: l2_world::script::Qs, item_id: i32) {
    let player = qs.player();
    if ctx.get_quest_items_count(player, item_id) < 10 {
        ctx.give_items(player, item_id, 1);
        ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
    } else if got_all_quest_items(ctx, player) {
        qs.set_cond(ctx, 2, true);
    }
}

/// Java `Q00901_HowLavasaurusesAreMade`.
pub struct Q00901HowLavasaurusesAreMade;

impl Script for Q00901HowLavasaurusesAreMade {
    fn name(&self) -> &'static str {
        "Q00901_HowLavasaurusesAreMade"
    }

    fn path(&self) -> &'static str {
        "quests/Q00901_HowLavasaurusesAreMade"
    }

    fn id(&self) -> i32 {
        901
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ROONEY]);
        s.add_talk_id(&[ROONEY]);
        s.add_kill_id(&[
            LAVASAURUS_NEWBORN,
            LAVASAURUS_FLEDGIING,
            LAVASAURUS_ADULT,
            LAVASAURUS_ELDERLY,
        ]);
        s.register_quest_items(&[FRAGMENT_STONE, FRAGMENT_HORN, FRAGMENT_HEAD, FRAGMENT_BODY]);
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
            "32049-03.htm" | "32049-08.html" | "32049-09.html" | "32049-10.html" | "32049-11.html" => {}
            "32049-04.htm" => qs.start_quest(ctx),
            "32049-12.html" => {
                ctx.give_items(player, TOTEM_OF_BODY, 1);
                qs.exit_quest_type(ctx, QuestType::Daily, true);
            }
            "32049-13.html" => {
                ctx.give_items(player, TOTEM_OF_SPIRIT, 1);
                qs.exit_quest_type(ctx, QuestType::Daily, true);
            }
            "32049-14.html" => {
                ctx.give_items(player, TOTEM_OF_FORTITUDE, 1);
                qs.exit_quest_type(ctx, QuestType::Daily, true);
            }
            "32049-15.html" => {
                ctx.give_items(player, TOTEM_OF_COURAGE, 1);
                qs.exit_quest_type(ctx, QuestType::Daily, true);
            }
            _ => return None,
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_cond(ctx, 1) {
            match npc.id {
                LAVASAURUS_NEWBORN => give_quest_items(ctx, qs, FRAGMENT_STONE),
                LAVASAURUS_FLEDGIING => give_quest_items(ctx, qs, FRAGMENT_HEAD),
                LAVASAURUS_ADULT => give_quest_items(ctx, qs, FRAGMENT_BODY),
                LAVASAURUS_ELDERLY => give_quest_items(ctx, qs, FRAGMENT_HORN),
                _ => {}
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);

        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "32049-01.htm"
                } else {
                    "32049-02.htm"
                }
                .to_owned();
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    htmltext = "32049-05.html".to_owned();
                } else if qs.is_cond(ctx, 2) {
                    if got_all_quest_items(ctx, player) {
                        ctx.take_items(player, FRAGMENT_STONE, -1);
                        ctx.take_items(player, FRAGMENT_HEAD, -1);
                        ctx.take_items(player, FRAGMENT_BODY, -1);
                        ctx.take_items(player, FRAGMENT_HORN, -1);
                        htmltext = "32049-06.html".to_owned();
                    } else {
                        htmltext = "32049-07.html".to_owned();
                    }
                }
            }
            COMPLETED => {
                if qs.is_now_available(ctx) {
                    qs.set_state(ctx, CREATED);
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "32049-01.htm"
                    } else {
                        "32049-02.html"
                    }
                    .to_owned();
                } else {
                    htmltext = "32049-16.html".to_owned();
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
    use l2_world::{QuestWrite, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00901_HowLavasaurusesAreMade";

    const NPCS: &str = r#"
        <npc id="32049" type="Folk" level="80" name="Rooney"/>
        <npc id="18799" type="Monster" level="70" name="Lavasaurus Newborn"/>
        <npc id="18800" type="Monster" level="70" name="Lavasaurus Fledgling"/>
        <npc id="18801" type="Monster" level="70" name="Lavasaurus Adult"/>
        <npc id="18802" type="Monster" level="70" name="Lavasaurus Elderly"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="21909" type="EtcItem" name="Fragment Stone"><set name="is_questitem" val="true"/></item>
        <item id="21910" type="EtcItem" name="Fragment Head"><set name="is_questitem" val="true"/></item>
        <item id="21911" type="EtcItem" name="Fragment Body"><set name="is_questitem" val="true"/></item>
        <item id="21912" type="EtcItem" name="Fragment Horn"><set name="is_questitem" val="true"/></item>
        <item id="21899" type="EtcItem" name="Totem of Body"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00901HowLavasaurusesAreMade);
        let pages = Pages::of(&Q00901HowLavasaurusesAreMade)
            .page("32049-01.htm", &["32049-03.htm"])
            .page("32049-03.htm", &["32049-04.htm"])
            .plain(&["32049-02.htm", "32049-04.htm", "32049-05.html", "32049-06.html"])
            .page(
                "32049-07.html",
                &["32049-08.html", "32049-09.html", "32049-10.html", "32049-11.html"],
            )
            .page("32049-08.html", &["32049-12.html"])
            .plain(&[
                "32049-09.html",
                "32049-10.html",
                "32049-11.html",
                "32049-12.html",
                "32049-16.html",
            ])
            .npcs(&[ROONEY]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rooney = t.spawn(ROONEY, 20);
        (t, rooney)
    }

    /// Kills the four lavasaurus kinds in turn until cond 2 (all fragments collected).
    fn hunt_fragments(t: &mut ScriptTest) {
        let kinds = [18799, 18800, 18801, 18802];
        for kill in 0..400 {
            let mob = t.spawn(kinds[kill % 4], 60);
            t.kill(1, mob);
            if t.take_writes().contains(&rows::update(NAME, "cond", "2")) {
                return;
            }
        }
        panic!("no cond 2 after 400 kills");
    }

    #[test]
    fn a_character_below_level_76_is_sent_away() {
        let (mut t, rooney) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, rooney).contains("32049-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn four_fragments_then_one_totem_once_a_day() {
        let (mut t, rooney) = setup();
        t.enter(1, 76, vec![]);
        assert!(t.quest_page(1, rooney).contains("32049-01.htm"));
        assert!(t.event(1, NAME, "32049-03.htm").contains("32049-03.htm"));
        assert!(t.event(1, NAME, "32049-04.htm").contains("32049-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.quest_page(1, rooney).contains("32049-05.html"));

        hunt_fragments(&mut t);
        assert_eq!(t.count(1, FRAGMENT_STONE), 10);
        assert_eq!(t.count(1, FRAGMENT_HORN), 10);

        // Rooney takes the fragments (06), then offers the totems (07).
        assert!(t.quest_page(1, rooney).contains("32049-06.html"));
        assert_eq!(t.count(1, FRAGMENT_BODY), 0);
        assert!(t.quest_page(1, rooney).contains("32049-07.html"));
        assert!(t.event(1, NAME, "32049-08.html").contains("32049-08.html"));
        assert!(t.event(1, NAME, "32049-12.html").contains("32049-12.html"));
        assert_eq!(t.count(1, TOTEM_OF_BODY), 1);

        // `exitQuest(DAILY, true)`: the rows go, the state is completed, `restartTime` is set.
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(matches!(&writes[2], QuestWrite::InsertVar { var, .. } if var == "restartTime"));
        assert!(t.quest_page(1, rooney).contains("32049-16.html"));
    }
}
