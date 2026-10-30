//! Quest 367: Electrifying Recharge! (Lorain, Catherok's thunder storm charges the lamp).
// Java: dist/game/data/scripts/quests/Q00367_ElectrifyingRecharge/Q00367_ElectrifyingRecharge.java
//
// Pattern: an `on_attack` quest: the first hit on a Catherok (once per life, script value 367)
// makes it cast the thunder storm and gives a random party member a chance to upgrade the lamp.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPC
const LORAIN: i32 = 30673;

// Monster
const CATHEROK: i32 = 21035;

// Items
const TITAN_LAMP1: i32 = 5875;
const TITAN_LAMP2: i32 = 5876;
const TITAN_LAMP3: i32 = 5877;
const TITAN_LAMP4: i32 = 5878;
const TITAN_LAMP5: i32 = 5879;
const BROKEN_TITAN_LAMP: i32 = 5880;

// Misc
const MIN_LEVEL: i32 = 37;

// Skill
const NPC_THUNDER_STORM: SkillKey = SkillKey { id: 4072, level: 4 };

/// Java `Q00367_ElectrifyingRecharge`.
pub struct Q00367ElectrifyingRecharge;

impl Script for Q00367ElectrifyingRecharge {
    fn name(&self) -> &'static str {
        "Q00367_ElectrifyingRecharge"
    }

    fn path(&self) -> &'static str {
        "quests/Q00367_ElectrifyingRecharge"
    }

    fn id(&self) -> i32 {
        367
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LORAIN]);
        s.add_talk_id(&[LORAIN]);
        s.add_attack_id(&[CATHEROK]);
        s.register_quest_items(&[
            TITAN_LAMP1,
            TITAN_LAMP2,
            TITAN_LAMP3,
            TITAN_LAMP4,
            TITAN_LAMP5,
            BROKEN_TITAN_LAMP,
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
        let qs = ctx.get_quest_state(player, false)?;
        match event {
            "30673-02.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, TITAN_LAMP1, 1);
                Some(event.to_owned())
            }
            "30673-05.html" => Some(event.to_owned()),
            "30673-06.html" => {
                qs.exit_quest(ctx, true, true);
                Some(event.to_owned())
            }
            _ => None,
        }
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if ctx.is_script_value(npc, 367) {
            return;
        }
        let qs = ctx.get_quest_state(attacker, false);
        if !qs.is_some_and(|qs| qs.is_started(ctx)) {
            return;
        }

        ctx.set_script_value(npc, 367);

        // The skill must exist, the NPC must have the MP and HP and no reuse delay.
        if let Some((mp_consume, hp_consume)) = ctx.world().script_skill_mp_hp_consume(NPC_THUNDER_STORM)
            && f64::from(mp_consume) < ctx.world().script_npc_current_mp(npc.object_id)
            && f64::from(hp_consume) < ctx.npc_current_hp(npc)
            && let Some(hash) = ctx.world().script_skill_reuse_hash_code(NPC_THUNDER_STORM)
            && ctx.world().skill_remaining_reuse_time(npc.object_id, hash) <= 0
        {
            ctx.cast_skill(npc, attacker, NPC_THUNDER_STORM);
        }

        let Some(lucky) = ctx.random_party_member_for_npc(attacker, Some(npc)) else {
            return;
        };

        let qs = ctx.get_quest_state(lucky, false);
        if let Some(qs) = qs
            && qs.is_started(ctx)
            && !ctx.has_quest_items(lucky, TITAN_LAMP5)
        {
            let random = ctx.get_random(37);
            if random == 0 {
                if ctx.has_quest_items(lucky, TITAN_LAMP1) {
                    ctx.give_items(lucky, TITAN_LAMP2, 1);
                    ctx.take_items(lucky, TITAN_LAMP1, -1);
                    ctx.play_sound(lucky, QuestSound::ItemsoundQuestMiddle);
                } else if ctx.has_quest_items(lucky, TITAN_LAMP2) {
                    ctx.give_items(lucky, TITAN_LAMP3, 1);
                    ctx.take_items(lucky, TITAN_LAMP2, -1);
                    ctx.play_sound(lucky, QuestSound::ItemsoundQuestMiddle);
                } else if ctx.has_quest_items(lucky, TITAN_LAMP3) {
                    ctx.give_items(lucky, TITAN_LAMP4, 1);
                    ctx.take_items(lucky, TITAN_LAMP3, -1);
                    ctx.play_sound(lucky, QuestSound::ItemsoundQuestMiddle);
                } else if ctx.has_quest_items(lucky, TITAN_LAMP4) {
                    ctx.give_items(lucky, TITAN_LAMP5, 1);
                    ctx.take_items(lucky, TITAN_LAMP4, -1);
                    qs.set_cond(ctx, 2, true);
                }
            } else if random == 1 && !ctx.has_quest_items(lucky, BROKEN_TITAN_LAMP) {
                ctx.give_items(lucky, BROKEN_TITAN_LAMP, 1);
                ctx.take_items_many(lucky, -1, &[TITAN_LAMP1, TITAN_LAMP2, TITAN_LAMP3, TITAN_LAMP4]);
                ctx.play_sound(lucky, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30673-01.htm"
                } else {
                    "30673-03.html"
                }
                .to_owned();
            }
            STARTED => {
                if !ctx.has_at_least_one_quest_item(player, &[TITAN_LAMP5, BROKEN_TITAN_LAMP]) {
                    htmltext = "30673-04.html".to_owned();
                } else if ctx.has_quest_items(player, BROKEN_TITAN_LAMP) {
                    ctx.give_items(player, TITAN_LAMP1, 1);
                    ctx.take_items(player, BROKEN_TITAN_LAMP, -1);
                    htmltext = "30673-07.html".to_owned();
                } else if ctx.has_quest_items(player, TITAN_LAMP5) {
                    let item_id = match ctx.get_random(14) {
                        0 => 4553,  // Greater Dye of STR <Str+1 Con-1>
                        1 => 4554,  // Greater Dye of STR <Str+1 Dex-1>
                        2 => 4555,  // Greater Dye of CON <Con+1 Str-1>
                        3 => 4556,  // Greater Dye of CON <Con+1 Dex-1>
                        4 => 4557,  // Greater Dye of DEX <Dex+1 Str-1>
                        5 => 4558,  // Greater Dye of DEX <Dex+1 Con-1>
                        6 => 4559,  // Greater Dye of INT <Int+1 Men-1>
                        7 => 4560,  // Greater Dye of INT <Int+1 Wit-1>
                        8 => 4561,  // Greater Dye of MEN <Men+1 Int-1>
                        9 => 4562,  // Greater Dye of MEN <Men+1 Wit-1>
                        10 => 4563, // Greater Dye of WIT <Wit+1 Int-1>
                        11 => 4564, // Greater Dye of WIT <Wit+1 Men-1>
                        _ => 4445,  // Dye of STR <Str+1 Con-3>
                    };
                    ctx.reward_items(player, item_id, 1);
                    ctx.take_items(player, TITAN_LAMP5, -1);
                    ctx.give_items(player, TITAN_LAMP1, 1);
                    htmltext = "30673-08.html".to_owned();
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

    const NAME: &str = "Q00367_ElectrifyingRecharge";

    const NPCS: &str = r#"
        <npc id="30673" type="Folk" level="70" name="Lorain"/>
        <npc id="21035" type="Monster" level="40" name="Catherok"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5875" type="EtcItem" name="Titan Lamp 1"><set name="is_questitem" val="true"/></item>
        <item id="5876" type="EtcItem" name="Titan Lamp 2"><set name="is_questitem" val="true"/></item>
        <item id="5877" type="EtcItem" name="Titan Lamp 3"><set name="is_questitem" val="true"/></item>
        <item id="5878" type="EtcItem" name="Titan Lamp 4"><set name="is_questitem" val="true"/></item>
        <item id="5879" type="EtcItem" name="Titan Lamp 5"><set name="is_questitem" val="true"/></item>
        <item id="5880" type="EtcItem" name="Broken Titan Lamp"><set name="is_questitem" val="true"/></item>
        <item id="4445" type="EtcItem" name="Dye of STR"/>
        <item id="4553" type="EtcItem" name="Dye 4553"/>
        <item id="4554" type="EtcItem" name="Dye 4554"/>
        <item id="4555" type="EtcItem" name="Dye 4555"/>
        <item id="4556" type="EtcItem" name="Dye 4556"/>
        <item id="4557" type="EtcItem" name="Dye 4557"/>
        <item id="4558" type="EtcItem" name="Dye 4558"/>
        <item id="4559" type="EtcItem" name="Dye 4559"/>
        <item id="4560" type="EtcItem" name="Dye 4560"/>
        <item id="4561" type="EtcItem" name="Dye 4561"/>
        <item id="4562" type="EtcItem" name="Dye 4562"/>
        <item id="4563" type="EtcItem" name="Dye 4563"/>
        <item id="4564" type="EtcItem" name="Dye 4564"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00367ElectrifyingRecharge);
        let pages = Pages::of(&Q00367ElectrifyingRecharge)
            .page("30673-01.htm", &["30673-02.htm"])
            .page("30673-04.html", &["30673-05.html", "30673-06.html"])
            .plain(&[
                "30673-02.htm",
                "30673-03.html",
                "30673-05.html",
                "30673-06.html",
                "30673-07.html",
                "30673-08.html",
            ])
            .npcs(&[LORAIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let lorain = t.spawn(LORAIN, 20);
        (t, lorain)
    }

    fn hit(t: &mut ScriptTest, cath: Oid) {
        let npc = ScriptNpc {
            object_id: cath,
            id: CATHEROK,
        };
        t.world.run_as_script(NAME, |ctx| {
            Q00367ElectrifyingRecharge.on_attack(ctx, npc, Oid(1), 10, false, None);
        });
    }

    #[test]
    fn a_low_level_character_is_sent_away() {
        let (mut t, lorain) = setup();
        t.enter(1, 36, vec![]);
        assert!(t.quest_page(1, lorain).contains("30673-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_lamp_is_charged_and_swapped_for_a_dye() {
        let (mut t, lorain) = setup();
        t.enter(1, 37, vec![]);
        assert!(t.quest_page(1, lorain).contains("30673-01.htm"));
        assert!(t.event(1, NAME, "30673-02.htm").contains("30673-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(t.count(1, TITAN_LAMP1), 1);
        assert!(t.quest_page(1, lorain).contains("30673-04.html"));

        // Hits (one per Catherok life) until the lamp is charged to the fifth stage.
        let mut hits = 0;
        while t.count(1, TITAN_LAMP5) == 0 {
            hits += 1;
            assert!(hits < 3000, "the lamp never charged");
            let cath = t.spawn(CATHEROK, 60);
            hit(&mut t, cath);
            // A second hit on the same Catherok does nothing.
            hit(&mut t, cath);
            if t.count(1, BROKEN_TITAN_LAMP) > 0 {
                // A broken lamp is swapped for the first one at Lorain.
                t.take();
                assert!(t.quest_page(1, lorain).contains("30673-07.html"));
                assert_eq!((t.count(1, BROKEN_TITAN_LAMP), t.count(1, TITAN_LAMP1)), (0, 1));
            }
        }
        let writes = t.take_writes();
        assert_eq!(writes.last(), Some(&rows::update(NAME, "cond", "2")));

        t.take();
        assert!(t.quest_page(1, lorain).contains("30673-08.html"));
        assert_eq!((t.count(1, TITAN_LAMP5), t.count(1, TITAN_LAMP1)), (0, 1));
    }

    #[test]
    fn the_quest_can_be_given_up() {
        let (mut t, lorain) = setup();
        t.enter(1, 37, vec![]);
        assert!(t.quest_page(1, lorain).contains("30673-01.htm"));
        t.event(1, NAME, "30673-02.htm");
        t.take_writes();
        assert!(t.quest_page(1, lorain).contains("30673-04.html"));
        assert!(t.event(1, NAME, "30673-06.html").contains("30673-06.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
