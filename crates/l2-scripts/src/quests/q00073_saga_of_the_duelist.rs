//! Quest 73: Saga of the Duelist.
// Java: dist/game/data/scripts/quests/Q00073_SagaOfTheDuelist/Q00073_SagaOfTheDuelist.java
//
// Pattern: a saga (`docs/design/scripts.md`, "Quest patterns"): the Java class only fills the
// fields of `AbstractSagaQuest`; here that is a `SagaData` constant and one `saga_quest!` line.
// The hooks are the shared ones of `abstract_saga_quest.rs`.

use super::abstract_saga_quest::{
    SagaData, on_attack, on_event, on_first_talk, on_kill, on_skill_see, on_talk, register_npcs,
};

/// Java `Q00073SagaOfTheDuelist`.
pub struct Q00073SagaOfTheDuelist;

const DATA: SagaData = SagaData {
    quest_id: 73,
    npc: [
        30849, 31624, 31226, 31331, 31639, 31646, 31647, 31653, 31654, 31655, 31656, 31277,
    ],
    items: [
        7080, 7537, 7081, 7488, 7271, 7302, 7333, 7364, 7395, 7426, 7096, 7546,
    ],
    mob: [27289, 27222, 27281],
    class_id: &[88],
    prev_class: &[0x02],
    npc_spawn_locations: [
        (164650, -74121, -2871),
        (47429, -56923, -2383),
        (47391, -56929, -2370),
    ],
    text: [
        "PLAYERNAME! Pursued to here! However, I jumped out of the Banshouren boundaries! You look at the giant as the sign of power!",
        "... Oh ... good! So it was ... let's begin!",
        "I do not have the patience ..! I have been a giant force ...! Cough chatter ah ah ah!",
        "Paying homage to those who disrupt the orderly will be PLAYERNAME's death!",
        "Now, my soul freed from the shackles of the millennium, Halixia, to the back side I come ...",
        "Why do you interfere others' battles?",
        "This is a waste of time.. Say goodbye...!",
        "...That is the enemy",
        "...Goodness! PLAYERNAME you are still looking?",
        "PLAYERNAME ... Not just to whom the victory. Only personnel involved in the fighting are eligible to share in the victory.",
        "Your sword is not an ornament. Don't you think, PLAYERNAME?",
        "Goodness! I no longer sense a battle there now.",
        "let...",
        "Only engaged in the battle to bar their choice. Perhaps you should regret.",
        "The human nation was foolish to try and fight a giant's strength.",
        "Must...Retreat... Too...Strong.",
        "PLAYERNAME. Defeat...by...retaining...and...Mo...Hacker",
        "....! Fight...Defeat...It...Fight...Defeat...It...",
    ],
};

/// Java `TUNATUN`: the NPC that gives "Top-grade Meat" (`TOPQUALITYMEAT`) at cond 3. It is an
/// extra talk NPC of this saga only, so the saga macro is not used: the same hooks, plus this one.
const TUNATUN: i32 = 31537;
const TOPQUALITYMEAT: i32 = 7546;

impl l2_world::Script for Q00073SagaOfTheDuelist {
    fn name(&self) -> &'static str {
        "Q00073_SagaOfTheDuelist"
    }

    fn path(&self) -> &'static str {
        concat!("quests/", "Q00073_SagaOfTheDuelist")
    }

    fn id(&self) -> i32 {
        DATA.quest_id
    }

    fn setup(&self, s: &mut l2_world::ScriptSetup) {
        register_npcs(&DATA, s);
        // Java: addTalkId(TUNATUN) after registerNPCs()
        s.add_talk_id(&[TUNATUN]);
    }

    fn on_event(
        &self,
        ctx: &mut l2_world::ScriptCtx<'_>,
        event: &str,
        npc: Option<l2_world::ScriptNpc>,
        player: Option<l2_util::ObjectId>,
    ) -> Option<String> {
        on_event(&DATA, ctx, event, npc, player)
    }

    fn on_attack(
        &self,
        ctx: &mut l2_world::ScriptCtx<'_>,
        npc: l2_world::ScriptNpc,
        attacker: l2_util::ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<l2_world::SkillKey>,
    ) {
        on_attack(&DATA, ctx, npc, attacker);
    }

    fn on_first_talk(
        &self,
        ctx: &mut l2_world::ScriptCtx<'_>,
        npc: l2_world::ScriptNpc,
        player: l2_util::ObjectId,
    ) -> Option<String> {
        on_first_talk(&DATA, ctx, npc, player)
    }

    fn on_kill(
        &self,
        ctx: &mut l2_world::ScriptCtx<'_>,
        npc: l2_world::ScriptNpc,
        killer: l2_util::ObjectId,
        _is_summon: bool,
    ) {
        on_kill(&DATA, ctx, npc, killer);
    }

    fn on_skill_see(
        &self,
        ctx: &mut l2_world::ScriptCtx<'_>,
        npc: l2_world::ScriptNpc,
        caster: l2_util::ObjectId,
        _skill: l2_world::SkillKey,
        targets: &[l2_util::ObjectId],
        _is_summon: bool,
    ) {
        on_skill_see(&DATA, ctx, npc, caster, targets);
    }

    fn on_talk(
        &self,
        ctx: &mut l2_world::ScriptCtx<'_>,
        npc: l2_world::ScriptNpc,
        player: l2_util::ObjectId,
    ) -> Option<String> {
        // Java onTalk: TUNATUN first, everything else is super.onTalk
        if npc.id == TUNATUN {
            let htmltext = ctx.get_no_quest_msg(player);
            let qs = ctx.get_quest_state(player, false);
            if let Some(qs) = qs
                && qs.is_cond(ctx, 3)
            {
                if !ctx.has_quest_items(player, TOPQUALITYMEAT) {
                    ctx.give_items(player, TOPQUALITYMEAT, 1);
                    return Some("tunatun_01.htm".to_owned());
                }
                return Some("tunatun_02.htm".to_owned());
            }
            return Some(htmltext);
        }
        on_talk(&DATA, ctx, npc, player)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId;
    use l2_world::ScriptRegistry;
    use l2_world::script::race;

    use super::{DATA, Q00073SagaOfTheDuelist as Saga, TOPQUALITYMEAT, TUNATUN};
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00073_SagaOfTheDuelist";

    fn npcs() -> String {
        format!(
            r#"<npc id="{}" type="Folk" level="70" name="Start"/>
            <npc id="{}" type="Folk" level="70" name="Step 1"/>"#,
            DATA.npc[0], DATA.npc[2]
        )
    }

    fn items() -> String {
        format!(
            r#"<item id="{}" type="EtcItem" name="Saga item"><set name="is_questitem" val="true"/></item>"#,
            DATA.items[10]
        )
    }

    fn setup() -> (ScriptTest, ObjectId, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Saga);
        let pages = Pages::of(&Saga)
            .page("0-01.htm", &["0-1"])
            .page("0-05.htm", &["accept"])
            .page("2-01.htm", &["2-1"])
            .plain(&["0-02.htm", "0-03.htm", "2-05.htm"])
            .npcs(&[DATA.npc[0], DATA.npc[2]]);
        let mut t = ScriptTest::with_pages(registry, &npcs(), &items(), pages);
        let start = t.spawn(DATA.npc[0], 20);
        let step1 = t.spawn(DATA.npc[2], 40);
        (t, start, step1)
    }

    #[test]
    fn a_lower_level_or_another_class_gets_no_start() {
        let (mut t, start, _) = setup();
        t.enter_as(1, 75, race::HUMAN, DATA.prev_class[0], vec![]);
        assert!(t.quest_page(1, start).contains("0-01.htm"));
        assert!(t.event(1, NAME, "0-1").contains("0-02.htm"));
        assert!(t.take_writes().is_empty());

        t.enter_as(2, 76, race::HUMAN, 0, vec![]);
        assert!(!t.quest_page(2, start).contains("0-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_start_writes_the_cond_and_gives_the_item_then_step_1() {
        let (mut t, start, step1) = setup();
        t.enter_as(1, 76, race::HUMAN, DATA.prev_class[0], vec![]);
        assert!(t.quest_page(1, start).contains("0-01.htm"));
        assert!(t.event(1, NAME, "0-1").contains("0-05.htm"));
        assert!(t.event(1, NAME, "accept").contains("0-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(t.count(1, DATA.items[10]), 1);

        assert!(t.quest_page(1, step1).contains("2-01.htm"));
        assert!(t.event(1, NAME, "2-1").contains("2-05.htm"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
    }

    #[test]
    fn tunatun_gives_the_top_quality_meat_once_at_cond_3() {
        let mut registry = ScriptRegistry::new();
        registry.add(Saga);
        let pages = Pages::of(&Saga)
            .plain(&["tunatun_01.htm", "tunatun_02.htm"])
            .npcs(&[TUNATUN]);
        let npcs = format!(r#"<npc id="{TUNATUN}" type="Folk" level="70" name="Tunatun"/>"#);
        let items = r#"<item id="7546" type="EtcItem" name="Top-Quality Meat"><set name="is_questitem" val="true"/></item>"#;
        let mut t = ScriptTest::with_pages(registry, &npcs, items, pages);
        let tuna = t.spawn(TUNATUN, 20);
        t.enter_as(1, 76, race::HUMAN, DATA.prev_class[0], vec![]);
        // Cond 3 is reached through the steps, which the other test covers: set it here.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(ObjectId(1), true).unwrap();
            qs.start_quest(ctx);
            qs.set_cond(ctx, 3, false);
        });
        assert!(t.quest_page(1, tuna).contains("tunatun_01.htm"));
        assert_eq!(t.count(1, TOPQUALITYMEAT), 1);
        assert!(t.quest_page(1, tuna).contains("tunatun_02.htm"));
        assert_eq!(t.count(1, TOPQUALITYMEAT), 1);
    }
}
