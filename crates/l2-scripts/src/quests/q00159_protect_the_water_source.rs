//! Quest 159: Protect the Water Source (Asterios sends an Elf to collect plague dust).
// Java: dist/game/data/scripts/quests/Q00159_ProtectTheWaterSource/Q00159_ProtectTheWaterSource.java
//
// Pattern: a one-time kill quest with two charms; one dust first, then five. Elves only.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ASTERIOS: i32 = 30154;

// Monster
const PLAGUE_ZOMBIE: i32 = 27017;

// Items
const PLAGUE_DUST: i32 = 1035;
const HYACINTH_CHARM: i32 = 1071;
const HYACINTH_CHARM2: i32 = 1072;

// Misc
const MIN_LEVEL: i32 = 12;

/// Java `Q00159_ProtectTheWaterSource`.
pub struct Q00159ProtectTheWaterSource;

impl Script for Q00159ProtectTheWaterSource {
    fn name(&self) -> &'static str {
        "Q00159_ProtectTheWaterSource"
    }

    fn path(&self) -> &'static str {
        "quests/Q00159_ProtectTheWaterSource"
    }

    fn id(&self) -> i32 {
        159
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ASTERIOS]);
        s.add_talk_id(&[ASTERIOS]);
        s.add_kill_id(&[PLAGUE_ZOMBIE]);
        s.register_quest_items(&[PLAGUE_DUST, HYACINTH_CHARM, HYACINTH_CHARM2]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false);
        if let Some(qs) = qs
            && event == "30154-04.htm"
        {
            qs.start_quest(ctx);
            ctx.give_items(player, HYACINTH_CHARM, 1);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs {
            match qs.get_cond(ctx) {
                1 => {
                    if ctx.get_random(100) < 40
                        && ctx.has_quest_items(killer, HYACINTH_CHARM)
                        && !ctx.has_quest_items(killer, PLAGUE_DUST)
                    {
                        ctx.give_items(killer, PLAGUE_DUST, 1);
                        qs.set_cond(ctx, 2, true);
                    }
                }
                3 => {
                    let mut dust = ctx.get_quest_items_count(killer, PLAGUE_DUST);
                    if ctx.get_random(100) < 40 && dust < 5 && ctx.has_quest_items(killer, HYACINTH_CHARM2) {
                        ctx.give_items(killer, PLAGUE_DUST, 1);
                        dust += 1;
                        if dust >= 5 {
                            qs.set_cond(ctx, 4, true);
                        } else {
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(player) == race::ELF {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        "30154-03.htm".to_owned()
                    } else {
                        "30154-02.htm".to_owned()
                    }
                } else {
                    "30154-01.htm".to_owned()
                };
            }
            STARTED => match qs.get_cond(ctx) {
                1 => {
                    if ctx.has_quest_items(player, HYACINTH_CHARM)
                        && !ctx.has_quest_items(player, PLAGUE_DUST)
                    {
                        htmltext = "30154-05.html".to_owned();
                    }
                }
                2 => {
                    if ctx.has_all_quest_items(player, &[HYACINTH_CHARM, PLAGUE_DUST]) {
                        ctx.take_items(player, HYACINTH_CHARM, -1);
                        ctx.take_items(player, PLAGUE_DUST, -1);
                        ctx.give_items(player, HYACINTH_CHARM2, 1);
                        qs.set_cond(ctx, 3, true);
                        htmltext = "30154-06.html".to_owned();
                    }
                }
                3 => {
                    if ctx.has_quest_items(player, HYACINTH_CHARM2) {
                        htmltext = "30154-07.html".to_owned();
                    }
                }
                4 => {
                    if ctx.has_quest_items(player, HYACINTH_CHARM2)
                        && ctx.get_quest_items_count(player, PLAGUE_DUST) >= 5
                    {
                        ctx.give_adena(player, 18_250, true);
                        qs.exit_quest(ctx, false, true);
                        htmltext = "30154-08.html".to_owned();
                    }
                }
                _ => {}
            },
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
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

    const NAME: &str = "Q00159_ProtectTheWaterSource";
    const ADENA: i32 = 57;

    const NPCS: &str = r#"
        <npc id="30154" type="Folk" level="1" name="Asterios"/>
        <npc id="27017" type="Monster" level="12" name="Plague Zombie"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1035" type="EtcItem" name="Plague Dust"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1071" type="EtcItem" name="Hyacinth Charm"><set name="is_questitem" val="true"/></item>
        <item id="1072" type="EtcItem" name="Hyacinth Charm"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00159ProtectTheWaterSource);
        let pages = Pages::of(&Q00159ProtectTheWaterSource)
            .page("30154-03.htm", &["30154-04.htm"])
            .plain(&[
                "30154-01.htm",
                "30154-02.htm",
                "30154-04.htm",
                "30154-05.html",
                "30154-06.html",
                "30154-07.html",
                "30154-08.html",
            ])
            .npcs(&[ASTERIOS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let asterios = t.spawn(ASTERIOS, 20);
        (t, asterios)
    }

    #[test]
    fn only_an_elf_of_level_12_starts_the_quest() {
        let (mut t, asterios) = setup();
        t.enter(1, 12, vec![]);
        assert!(t.quest_page(1, asterios).contains("30154-01.htm"));
        t.enter_as(2, 11, race::ELF, 0, vec![]);
        assert!(t.quest_page(2, asterios).contains("30154-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn dust_charm_swap_five_dust_and_the_reward() {
        let (mut t, asterios) = setup();
        t.enter_as(1, 12, race::ELF, 0, vec![]);
        assert!(t.quest_page(1, asterios).contains("30154-03.htm"));
        assert!(t.event(1, NAME, "30154-04.htm").contains("30154-04.htm"));
        assert_eq!(t.count(1, HYACINTH_CHARM), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, asterios).contains("30154-05.html"));

        // One dust (40 %): cond 2.
        let mut kills = 0;
        while t.count(1, PLAGUE_DUST) < 1 {
            let zombie = t.spawn(PLAGUE_ZOMBIE, 60);
            t.kill(1, zombie);
            kills += 1;
            assert!(kills < 500, "no dust");
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The second charm replaces the first and the dust.
        assert!(t.quest_page(1, asterios).contains("30154-06.html"));
        assert_eq!(t.count(1, HYACINTH_CHARM), 0);
        assert_eq!(t.count(1, HYACINTH_CHARM2), 1);
        assert_eq!(t.count(1, PLAGUE_DUST), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, asterios).contains("30154-07.html"));

        // Five dust: cond 4.
        while t.count(1, PLAGUE_DUST) < 5 {
            let zombie = t.spawn(PLAGUE_ZOMBIE, 60);
            t.kill(1, zombie);
            kills += 1;
            assert!(kills < 1000, "no dust");
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        assert!(t.quest_page(1, asterios).contains("30154-08.html"));
        assert_eq!(t.count(1, ADENA), 18_250);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
