//! Quest 692: How to Oppose Evil (Dilios sends the character to the three Seeds; Kirklan trades
//! the items the Seed monsters drop for rewards).
// Java: dist/game/data/scripts/quests/Q00692_HowtoOpposeEvil/Q00692_HowtoOpposeEvil.java

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const DILIOS: i32 = 32549;
const KIRKLAN: i32 = 32550;
const LEKONS_CERTIFICATE: i32 = 13857;
const QUEST_ITEMS: [i32; 7] = [13863, 13864, 13865, 13866, 13867, 15535, 15536];

/// Java `QUEST_MOBS` as ranges: `(first id, last id, item, chance)`; the chance is Java's
/// `ItemHolder` count (per thousand).
const QUEST_MOB_RANGES: [(i32, i32, i32, i64); 6] = [
    // Seed of Infinity
    (22509, 22515, 13863, 500),
    // Seed of Destruction
    (22537, 22544, 13865, 250),
    (22546, 22552, 13865, 250),
    (22593, 22593, 13865, 250),
    (22596, 22597, 13865, 250),
    // Seed of Annihilation
    (22746, 22765, 15536, 125),
];

/// The `ItemHolder` of an NPC id: `(item, chance)`.
fn quest_mob(npc_id: i32) -> Option<(i32, i64)> {
    QUEST_MOB_RANGES
        .iter()
        .find(|r| (r.0..=r.1).contains(&npc_id))
        .map(|r| (r.2, r.3))
}

/// Java `giveReward(player, itemId, minCount, rewardItemId, rewardCount)`.
fn give_reward(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    item_id: i32,
    min_count: i64,
    reward_item_id: i32,
    reward_count: i64,
) -> bool {
    let mut count = ctx.get_quest_items_count(player, item_id);
    if count < min_count {
        return false;
    }

    count /= min_count;
    ctx.take_items(player, item_id, count * min_count);
    ctx.reward_items(player, reward_item_id, reward_count * count);
    true
}

/// Java `Q00692_HowtoOpposeEvil`.
pub struct Q00692HowtoOpposeEvil;

impl Script for Q00692HowtoOpposeEvil {
    fn name(&self) -> &'static str {
        "Q00692_HowtoOpposeEvil"
    }

    fn path(&self) -> &'static str {
        "quests/Q00692_HowtoOpposeEvil"
    }

    fn id(&self) -> i32 {
        692
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DILIOS]);
        s.add_talk_id(&[DILIOS, KIRKLAN]);
        let mobs: Vec<i32> = QUEST_MOB_RANGES.iter().flat_map(|r| r.0..=r.1).collect();
        s.add_kill_id(&mobs);
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
            "32549-03.htm" => qs.start_quest(ctx),
            "32550-04.htm" => qs.set_cond(ctx, 3, false),
            "32550-07.htm" => {
                if !give_reward(ctx, player, 13863, 5, 13796, 1) {
                    return Some("32550-08.htm".to_owned());
                }
            }
            "32550-09.htm" => {
                if !give_reward(ctx, player, 13798, 1, 57, 5000) {
                    return Some("32550-10.htm".to_owned());
                }
            }
            "32550-12.htm" => {
                if !give_reward(ctx, player, 13865, 5, 13841, 1) {
                    return Some("32550-13.htm".to_owned());
                }
            }
            "32550-14.htm" => {
                if !give_reward(ctx, player, 13867, 1, 57, 5000) {
                    return Some("32550-15.htm".to_owned());
                }
            }
            "32550-17.htm" => {
                if !give_reward(ctx, player, 15536, 5, 15486, 1) {
                    return Some("32550-18.htm".to_owned());
                }
            }
            "32550-19.htm" => {
                if !give_reward(ctx, player, 15535, 1, 57, 5000) {
                    return Some("32550-20.htm".to_owned());
                }
            }
            _ => {}
        }

        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(party_member) = ctx.get_random_party_member_cond(player, 3) else {
            return;
        };

        let qs = ctx.get_quest_state(party_member, false);
        if let (Some(_), Some((item, count))) = (qs, quest_mob(npc.id)) {
            let mut chance = count as i32;
            let mut num_items = chance / 1000;
            chance %= 1000;
            if ctx.get_random(1000) < chance {
                num_items += 1;
            }

            if num_items > 0 {
                // NOTE(java-quirk): the item goes to the killer, not to the party member drawn.
                ctx.give_items(player, item, i64::from(num_items));
                ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= 75 {
                "32549-01.htm"
            } else {
                "32549-00.htm"
            }
            .to_owned();
        } else if npc.id == DILIOS {
            if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, LEKONS_CERTIFICATE) {
                htmltext = "32549-04.htm".to_owned();
                ctx.take_items(player, LEKONS_CERTIFICATE, -1);
                qs.set_cond(ctx, 2, false);
            } else if qs.is_cond(ctx, 2) {
                htmltext = "32549-05.htm".to_owned();
            }
        } else if qs.is_cond(ctx, 2) {
            htmltext = "32550-01.htm".to_owned();
        } else if qs.is_cond(ctx, 3) {
            for i in QUEST_ITEMS {
                if ctx.get_quest_items_count(player, i) > 0 {
                    return Some("32550-05.htm".to_owned());
                }
            }
            htmltext = "32550-04.htm".to_owned();
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00692_HowtoOpposeEvil";

    const NPCS: &str = r#"
        <npc id="32549" type="Folk" level="70" name="Dilios"/>
        <npc id="32550" type="Folk" level="70" name="Kirklan"/>
        <npc id="22509" type="Monster" level="80" name="Seed of Infinity monster"/>
        <npc id="22746" type="Monster" level="80" name="Seed of Annihilation monster"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13857" type="EtcItem" name="Lekon's Certificate"><set name="is_questitem" val="true"/></item>
        <item id="13863" type="EtcItem" name="Infinity item"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="13865" type="EtcItem" name="Destruction item"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="13867" type="EtcItem" name="Destruction trophy"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="15536" type="EtcItem" name="Annihilation item"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="13796" type="EtcItem" name="Reward 1"><set name="is_stackable" val="true"/></item>
        <item id="15486" type="EtcItem" name="Reward 3"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00692HowtoOpposeEvil);
        let pages = Pages::of(&Q00692HowtoOpposeEvil)
            .page("32549-01.htm", &["32549-02.htm"])
            .page("32549-02.htm", &["32549-03.htm"])
            .plain(&["32549-00.htm", "32549-03.htm", "32549-04.htm", "32549-05.htm"])
            .page("32550-01.htm", &["32550-02.htm"])
            .page("32550-02.htm", &["32550-03.htm"])
            .page("32550-03.htm", &["32550-01.htm", "32550-04.htm"])
            .page("32550-04.htm", &[])
            .page(
                "32550-05.htm",
                &["32550-06.htm", "32550-11.htm", "32550-16.htm", "32550-21.htm"],
            )
            .page("32550-06.htm", &["32550-07.htm", "32550-09.htm", "32550-05.htm"])
            .page("32550-16.htm", &["32550-17.htm", "32550-19.htm", "32550-05.htm"])
            .page("32550-07.htm", &["32550-05.htm", "32550-21.htm"])
            .plain(&["32550-08.htm", "32550-17.htm", "32550-18.htm", "32550-21.htm"])
            .npcs(&[DILIOS, KIRKLAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let dilios = t.spawn(DILIOS, 20);
        let kirklan = t.spawn(KIRKLAN, 30);
        (t, dilios, kirklan)
    }

    #[test]
    fn level_75_is_needed() {
        let (mut t, dilios, _) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, dilios).contains("32549-00.htm"));
        t.enter(2, 75, vec![]);
        assert!(t.quest_page(2, dilios).contains("32549-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_seeds_drops_are_traded_at_kirklan() {
        let (mut t, dilios, kirklan) = setup();
        t.enter(1, 75, vec![item(100, 13863, 12)]);
        t.quest_page(1, dilios);
        t.event(1, NAME, "32549-02.htm");
        assert!(t.event(1, NAME, "32549-03.htm").contains("32549-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        // Dilios takes Lekon's certificate and moves the quest to cond 2.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), LEKONS_CERTIFICATE, 1);
        });
        assert!(t.quest_page(1, dilios).contains("32549-04.htm"));
        assert_eq!(t.count(1, LEKONS_CERTIFICATE), 0);
        assert_eq!(t.take_writes()[0], rows::update(NAME, "cond", "2"));

        // Kirklan: cond 3, then 5 items per reward (12 items = 2 rewards).
        assert!(t.quest_page(1, kirklan).contains("32550-01.htm"));
        t.event(1, NAME, "32550-02.htm");
        assert!(t.event(1, NAME, "32550-03.htm").contains("32550-03.htm"));
        assert!(t.event(1, NAME, "32550-04.htm").contains("32550-04.htm"));
        assert_eq!(t.take_writes()[0], rows::update(NAME, "cond", "3"));
        assert!(t.quest_page(1, kirklan).contains("32550-05.htm"));
        t.event(1, NAME, "32550-06.htm");
        assert!(t.event(1, NAME, "32550-07.htm").contains("32550-07.htm"));
        assert_eq!((t.count(1, 13863), t.count(1, 13796)), (2, 2));
        // Not enough left for another one.
        t.event(1, NAME, "32550-05.htm");
        t.event(1, NAME, "32550-06.htm");
        let page = t.event(1, NAME, "32550-07.htm");
        assert!(page.contains("32550-08.htm"), "{page}");
    }

    #[test]
    fn the_seed_monsters_drop_for_the_killer() {
        let (mut t, dilios, kirklan) = setup();
        t.enter(1, 75, vec![]);
        t.quest_page(1, dilios);
        t.event(1, NAME, "32549-02.htm");
        t.event(1, NAME, "32549-03.htm");
        t.world.run_as_script(NAME, |ctx| {
            ctx.get_quest_state(Oid(1), false)
                .unwrap()
                .set_cond(ctx, 3, false);
        });
        let _ = kirklan;
        let mut kills = 0;
        while t.count(1, 13863) < 3 {
            let mob = t.spawn(22509, 40);
            t.kill(1, mob);
            kills += 1;
            assert!(kills < 100);
        }
        // The Seed of Annihilation monsters drop the other item.
        let mut kills = 0;
        while t.count(1, 15536) < 1 {
            let mob = t.spawn(22746, 40);
            t.kill(1, mob);
            kills += 1;
            assert!(kills < 200);
        }
    }
}
