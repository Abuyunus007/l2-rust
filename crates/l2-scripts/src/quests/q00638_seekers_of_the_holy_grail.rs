//! Quest 638: Seekers of the Holy Grail (Innocentin in the Town of Aden; the Triol's followers and
//! the undead of the Schuttgart/Aden ruins drop the totems, 2000 of them buy an enchant scroll or
//! 3576000 adena; some drop a key on the side).
// Java: dist/game/data/scripts/quests/Q00638_SeekersOfTheHolyGrail/Q00638_SeekersOfTheHolyGrail.java
//
// Pattern: a repeatable kill count with a drop chance per monster, an optional key roll after a
// drop (`DropInfo`), a random reward choice (an 80 % roll, then the scroll kind by a coin), and a
// quit. Java's `getRandom(100)` runs before `getRandomBoolean()`.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const INNOCENTIN: i32 = 31328;

// Items
const TOTEM: i32 = 8068;
const ANTEROOM_KEY: i32 = 8273;
const CHAPEL_KEY: i32 = 8274;
const KEY_OF_DARKNESS: i32 = 8275;

// Misc
const MIN_LEVEL: i32 = 73;
const TOTEMS_REQUIRED_COUNT: i64 = 2000;

// Rewards
const SCROLL_ENCHANT_W_S: i32 = 959;
const SCROLL_ENCHANT_A_S: i32 = 960;

/// Java `DropInfo`: the totem chance of a monster, and an optional key (`key_id` 0 for none) with
/// its own chance (per cent) and count.
#[derive(Clone, Copy)]
struct DropInfo {
    id: i32,
    chance: f64,
    key_id: i32,
    key_chance: i32,
    key_count: i64,
}

const fn drop_info(id: i32, chance: f64) -> DropInfo {
    DropInfo {
        id,
        chance,
        key_id: 0,
        key_chance: 0,
        key_count: 0,
    }
}

const fn drop_key(id: i32, chance: f64, key_id: i32, key_chance: i32, key_count: i64) -> DropInfo {
    DropInfo {
        id,
        chance,
        key_id,
        key_chance,
        key_count,
    }
}

/// Java `MOBS_DROP_CHANCES` (the duplicate `22194` put is one entry, with the same values).
const MOBS_DROP_CHANCES: [(i32, DropInfo); 48] = [
    (22136, drop_info(TOTEM, 0.55)),                        // Gatekeeper Zombie
    (22137, drop_info(TOTEM, 0.06)),                        // Penance Guard
    (22138, drop_info(TOTEM, 0.06)),                        // Chapel Guard
    (22139, drop_info(TOTEM, 0.54)),                        // Old Aristocrat's Soldier
    (22140, drop_info(TOTEM, 0.54)),                        // Zombie Worker
    (22141, drop_info(TOTEM, 0.55)),                        // Forgotten Victim
    (22142, drop_info(TOTEM, 0.54)),                        // Triol's Layperson
    (22143, drop_key(TOTEM, 0.62, CHAPEL_KEY, 100, 1)),     // Triol's Believer
    (22144, drop_info(TOTEM, 0.54)),                        // Resurrected Temple Knight
    (22145, drop_info(TOTEM, 0.53)),                        // Ritual Sacrifice
    (22146, drop_key(TOTEM, 0.54, KEY_OF_DARKNESS, 10, 1)), // Triol's Priest
    (22147, drop_info(TOTEM, 0.55)),                        // Ritual Offering
    (22148, drop_info(TOTEM, 0.45)),                        // Triol's Believer
    (22149, drop_key(TOTEM, 0.54, ANTEROOM_KEY, 100, 6)),   // Ritual Offering
    (22150, drop_info(TOTEM, 0.46)),                        // Triol's Believer
    (22151, drop_key(TOTEM, 0.62, KEY_OF_DARKNESS, 10, 1)), // Triol's Priest
    (22152, drop_info(TOTEM, 0.55)),                        // Temple Guard
    (22153, drop_info(TOTEM, 0.54)),                        // Temple Guard Captain
    (22154, drop_info(TOTEM, 0.53)),                        // Ritual Sacrifice
    (22155, drop_info(TOTEM, 0.75)),                        // Triol's High Priest
    (22156, drop_info(TOTEM, 0.67)),                        // Triol's Priest
    (22157, drop_info(TOTEM, 0.66)),                        // Triol's Priest
    (22158, drop_info(TOTEM, 0.67)),                        // Triol's Believer
    (22159, drop_info(TOTEM, 0.75)),                        // Triol's High Priest
    (22160, drop_info(TOTEM, 0.67)),                        // Triol's Priest
    (22161, drop_info(TOTEM, 0.78)),                        // Ritual Sacrifice
    (22162, drop_info(TOTEM, 0.67)),                        // Triol's Believer
    (22163, drop_info(TOTEM, 0.87)),                        // Triol's High Priest
    (22164, drop_info(TOTEM, 0.67)),                        // Triol's Believer
    (22165, drop_info(TOTEM, 0.66)),                        // Triol's Priest
    (22166, drop_info(TOTEM, 0.66)),                        // Triol's Believer
    (22167, drop_info(TOTEM, 0.75)),                        // Triol's High Priest
    (22168, drop_info(TOTEM, 0.66)),                        // Triol's Priest
    (22169, drop_info(TOTEM, 0.78)),                        // Ritual Sacrifice
    (22170, drop_info(TOTEM, 0.67)),                        // Triol's Believer
    (22171, drop_info(TOTEM, 0.87)),                        // Triol's High Priest
    (22172, drop_info(TOTEM, 0.78)),                        // Ritual Sacrifice
    (22173, drop_info(TOTEM, 0.66)),                        // Triol's Priest
    (22174, drop_info(TOTEM, 0.67)),                        // Triol's Priest
    (22175, drop_info(TOTEM, 0.03)),                        // Andreas' Captain of the Royal Guard
    (22176, drop_info(TOTEM, 0.03)),                        // Andreas' Royal Guards
    (22188, drop_info(TOTEM, 0.03)),                        // Andreas' Captain of the Royal Guard
    (22189, drop_info(TOTEM, 0.03)),                        // Andreas' Royal Guards
    (22190, drop_info(TOTEM, 0.03)),                        // Ritual Sacrifice
    (22191, drop_info(TOTEM, 0.03)),                        // Andreas' Captain of the Royal Guard
    (22192, drop_info(TOTEM, 0.03)),                        // Andreas' Royal Guards
    (22193, drop_info(TOTEM, 0.03)),                        // Andreas' Royal Guards
    (22194, drop_info(TOTEM, 0.03)),                        // Penance Guard, Ritual Sacrifice
];

/// Java `Q00638_SeekersOfTheHolyGrail`.
pub struct Q00638SeekersOfTheHolyGrail;

impl Script for Q00638SeekersOfTheHolyGrail {
    fn name(&self) -> &'static str {
        "Q00638_SeekersOfTheHolyGrail"
    }

    fn path(&self) -> &'static str {
        "quests/Q00638_SeekersOfTheHolyGrail"
    }

    fn id(&self) -> i32 {
        638
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[INNOCENTIN]);
        s.add_talk_id(&[INNOCENTIN]);
        let mobs: Vec<i32> = MOBS_DROP_CHANCES.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[TOTEM]);
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
        // Java has no default branch: an event not listed gives null.
        let mut htmltext = None;
        match event {
            "31328-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "31328-06.html" => {
                if qs.is_started(ctx) {
                    htmltext = Some(event.to_owned());
                }
            }
            "reward" => {
                if qs.is_started(ctx) && ctx.get_quest_items_count(player, TOTEM) >= TOTEMS_REQUIRED_COUNT {
                    if ctx.get_random(100) < 80 {
                        if ctx.get_random_boolean() {
                            ctx.give_items(player, SCROLL_ENCHANT_A_S, 1);
                        } else {
                            ctx.give_items(player, SCROLL_ENCHANT_W_S, 1);
                        }
                        htmltext = Some("31328-07.html".to_owned());
                    } else {
                        ctx.give_adena(player, 3_576_000, true);
                        htmltext = Some("31328-08.html".to_owned());
                    }
                    ctx.take_items(player, TOTEM, TOTEMS_REQUIRED_COUNT);
                }
            }
            "31328-09.html" => {
                if qs.is_started(ctx) {
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some("31328-09.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMemberState(killer, -1, 3, npc)`.
        let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) else {
            return;
        };
        let Some(&(_, info)) = MOBS_DROP_CHANCES.iter().find(|(id, _)| *id == npc.id) else {
            return;
        };
        let member = qs.player();
        if ctx.give_item_randomly(member, Some(npc), info.id, 1, 1, 0, info.chance, true)
            && info.key_id > 0
            && ctx.get_random(100) < info.key_chance
        {
            ctx.give_items(member, info.key_id, info.key_count);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "31328-01.htm".to_owned()
            } else {
                "31328-02.htm".to_owned()
            };
        } else if state == STARTED {
            htmltext = if ctx.get_quest_items_count(player, TOTEM) >= TOTEMS_REQUIRED_COUNT {
                "31328-04.html".to_owned()
            } else {
                "31328-05.html".to_owned()
            };
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

    const NAME: &str = "Q00638_SeekersOfTheHolyGrail";

    const NPCS: &str = r#"
        <npc id="31328" type="Folk" level="70" name="Innocentin"/>
        <npc id="22136" type="Monster" level="70" name="Gatekeeper Zombie"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8068" type="EtcItem" name="Totem"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="959" type="EtcItem" name="Scroll W/S"><set name="is_stackable" val="true"/></item>
        <item id="960" type="EtcItem" name="Scroll A/S"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00638SeekersOfTheHolyGrail);
        let pages = Pages::of(&Q00638SeekersOfTheHolyGrail)
            .page("31328-01.htm", &["31328-03.htm"])
            .plain(&["31328-02.htm", "31328-03.htm", "31328-07.html", "31328-08.html"])
            .page("31328-04.html", &["reward", "31328-06.html"])
            .page("31328-05.html", &["31328-06.html", "31328-09.html"])
            .plain(&["31328-06.html", "31328-09.html"])
            .npcs(&[INNOCENTIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let innocentin = t.spawn(INNOCENTIN, 20);
        (t, innocentin)
    }

    #[test]
    fn a_character_below_level_73_is_sent_away() {
        let (mut t, innocentin) = setup();
        t.enter(1, 72, vec![]);
        assert!(t.quest_page(1, innocentin).contains("31328-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn two_thousand_totems_buy_a_reward_and_the_quest_ends() {
        let (mut t, innocentin) = setup();
        t.enter(1, 73, vec![]);
        assert!(t.quest_page(1, innocentin).contains("31328-01.htm"));
        assert!(t.event(1, NAME, "31328-03.htm").contains("31328-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, innocentin).contains("31328-05.html"));

        // Totems to 2000 by the kills (the gatekeeper zombie drops 55 %).
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), TOTEM, 1999);
        });
        t.take();
        let mut kills = 0;
        while t.count(1, TOTEM) < TOTEMS_REQUIRED_COUNT {
            let mob = t.spawn(22136, 40);
            t.kill(1, mob);
            kills += 1;
            assert!(kills < 100, "no totem dropped");
        }
        assert!(t.take_writes().is_empty());

        // The reward: a scroll (80 %) or the adena; the totems go either way.
        assert!(t.quest_page(1, innocentin).contains("31328-04.html"));
        let reward = t.event(1, NAME, "reward");
        assert!(
            reward.contains("31328-07.html") || reward.contains("31328-08.html"),
            "{reward}"
        );
        assert_eq!(t.count(1, TOTEM), 0);
        let scrolls = t.count(1, SCROLL_ENCHANT_A_S) + t.count(1, SCROLL_ENCHANT_W_S);
        let adena = t.count(1, 57);
        assert!(
            (scrolls == 1 && adena == 0) || (scrolls == 0 && adena == 3_576_000),
            "{scrolls} {adena}"
        );

        // The quest goes on until the quit page.
        assert!(t.quest_page(1, innocentin).contains("31328-05.html"));
        assert!(t.event(1, NAME, "31328-09.html").contains("31328-09.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
