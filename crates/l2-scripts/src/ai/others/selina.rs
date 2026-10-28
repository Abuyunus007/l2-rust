//! Selina, the Golden Ram Mercenary buffer: buffs for Golden Ram Coins, paid when the cast ends.
// Java: dist/game/data/scripts/ai/others/Selina/Selina.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};
use tracing::warn;

// NPC
const SELINA: i32 = 31556;

// Items
const GOLDEN_RAM_BADGE_RECRUIT: i32 = 7246;
const GOLDEN_RAM_BADGE_SOLDIER: i32 = 7247;
const GOLDEN_RAM_COIN: i32 = 7251;

/// Java `BUFFS.get(key)`: the skill and its price in coins (Java's `BuffHolder`).
fn buff(skill_id: &str) -> Option<(SkillKey, i64)> {
    let (id, level, cost) = match skill_id {
        "4359" => (4359, 2, 2), // Focus
        "4360" => (4360, 2, 2), // Death Whisper
        "4345" => (4345, 3, 3), // Might
        "4355" => (4355, 2, 3), // Acumen
        "4352" => (4352, 1, 3), // Berserker Spirit
        "4354" => (4354, 2, 3), // Vampiric Rage
        "4356" => (4356, 1, 6), // Empower
        "4357" => (4357, 2, 6), // Haste
        _ => return None,
    };
    Some((SkillKey { id, level }, cost))
}

/// Java `ai.others.Selina.Selina`.
pub struct Selina;

impl Script for Selina {
    fn name(&self) -> &'static str {
        "Selina"
    }

    fn path(&self) -> &'static str {
        "ai/others/Selina"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SELINA]);
        s.add_talk_id(&[SELINA]);
        s.add_first_talk_id(&[SELINA]);
        s.add_spell_finished_id(&[SELINA]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let (npc, player) = (npc?, player?);
        if let Some((skill, cost)) = buff(event) {
            if ctx.get_quest_items_count(player, GOLDEN_RAM_COIN) >= cost {
                ctx.cast_skill(npc, player, skill);
                // Java `super.onEvent`.
                return None;
            }
        } else {
            warn!(
                "Selina AI: player {} sent invalid bypass: {event}",
                ctx.player_name(player)
            );
        }

        Some("31556-02.html".to_owned())
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let htmltext = if ctx.has_quest_items(player, GOLDEN_RAM_BADGE_SOLDIER) {
            "31556-08.html"
        } else if ctx.has_quest_items(player, GOLDEN_RAM_BADGE_RECRUIT) {
            "31556-01.html"
        } else {
            "31556-09.html"
        };

        Some(htmltext.to_owned())
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        _npc: ScriptNpc,
        player: Option<ObjectId>,
        skill: SkillKey,
    ) {
        let Some(player) = player else {
            return;
        };
        if let Some((_, cost)) = buff(&skill.id.to_string()) {
            ctx.take_items(player, GOLDEN_RAM_COIN, cost);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item, pages_to};

    const NPCS: &str = r#"<npc id="31556" type="Folk" level="70" name="Selina"/>"#;
    const ITEMS: &str = r#"<item id="7246" type="EtcItem" name="Recruit Badge"><set name="is_questitem" val="true"/></item>
        <item id="7247" type="EtcItem" name="Soldier Badge"><set name="is_questitem" val="true"/></item>
        <item id="7251" type="EtcItem" name="Golden Ram Coin"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;
    const DIR: &str = "data/scripts/ai/others/Selina";

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Selina);
        let pages = [
            (
                format!("{DIR}/31556-01.html"),
                "<html><body>recruit</body></html>",
            ),
            (
                format!("{DIR}/31556-02.html"),
                "<html><body>no coins</body></html>",
            ),
            (
                format!("{DIR}/31556-08.html"),
                "<html><body>soldier</body></html>",
            ),
            (format!("{DIR}/31556-09.html"), "<html><body>nobody</body></html>"),
        ];
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), *b)).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        let selina = t.spawn(SELINA, 20);
        (t, selina)
    }

    #[test]
    fn the_first_page_follows_the_badge() {
        let (mut t, selina) = setup();
        t.enter(1, 60, vec![]);
        t.enter(2, 60, vec![item(5000, GOLDEN_RAM_BADGE_RECRUIT, 1)]);
        t.enter(
            3,
            60,
            vec![
                item(5001, GOLDEN_RAM_BADGE_RECRUIT, 1),
                item(5002, GOLDEN_RAM_BADGE_SOLDIER, 1),
            ],
        );
        t.take();
        for (p, text) in [(1, "nobody"), (2, "recruit"), (3, "soldier")] {
            t.talk(p, selina);
            assert!(pages_to(&t.take(), p).pop().unwrap().contains(text), "{p}");
        }
    }

    #[test]
    fn a_buff_without_the_coins_gives_the_refusal_page_and_a_bad_bypass_too() {
        let (mut t, selina) = setup();
        t.enter(1, 60, vec![item(5000, GOLDEN_RAM_COIN, 1)]);
        let mut answers = Vec::new();
        t.world.run_as_script("Selina", |ctx| {
            let npc = ctx.script_npc(selina);
            answers.push(Selina.on_event(ctx, "4345", npc, Some(ObjectId(1))));
            answers.push(Selina.on_event(ctx, "nonsense", npc, Some(ObjectId(1))));
            // Two coins are enough for Focus: the cast starts (the engine has no skill here).
            answers.push(Selina.on_event(ctx, "4359", npc, Some(ObjectId(1))));
        });
        assert_eq!(
            answers,
            [
                Some("31556-02.html".to_owned()),
                Some("31556-02.html".to_owned()),
                Some("31556-02.html".to_owned())
            ]
        );
        assert_eq!(t.count(1, GOLDEN_RAM_COIN), 1);
    }

    #[test]
    fn the_coins_are_taken_when_the_cast_ends() {
        let (mut t, selina) = setup();
        t.enter(1, 60, vec![item(5000, GOLDEN_RAM_COIN, 5)]);
        let mut answer = Some(String::new());
        t.world.run_as_script("Selina", |ctx| {
            let npc = ctx.script_npc(selina).unwrap();
            answer = Selina.on_event(ctx, "4345", Some(npc), Some(ObjectId(1)));
            Selina.on_spell_finished(ctx, npc, Some(ObjectId(1)), SkillKey { id: 4345, level: 3 });
            Selina.on_spell_finished(ctx, npc, Some(ObjectId(1)), SkillKey { id: 1, level: 1 });
        });
        assert_eq!(answer, None);
        assert_eq!(t.count(1, GOLDEN_RAM_COIN), 2);
    }
}
