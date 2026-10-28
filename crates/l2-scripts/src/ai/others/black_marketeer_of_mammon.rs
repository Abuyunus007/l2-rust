//! The Black Marketeer of Mammon: 2,000,000 adena for 500,000 ancient adena, daily, 20:00-24:00.
// Java: dist/game/data/scripts/ai/others/BlackMarketeerOfMammon/BlackMarketeerOfMammon.java

use l2_util::ObjectId;
use l2_world::script::STARTED;
use l2_world::{QuestType, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const BLACK_MARKETEER: i32 = 31092;

// Misc
const MIN_LEVEL: i32 = 60;
/// Java `Inventory.ADENA_ID` and `Inventory.ANCIENT_ADENA_ID`.
const ADENA_ID: i32 = 57;
const ANCIENT_ADENA_ID: i32 = 5575;

/// Java `ai.others.BlackMarketeerOfMammon.BlackMarketeerOfMammon`.
pub struct BlackMarketeerOfMammon;

/// Java `exchangeAvailable()`: the local time is from 20:00 to 23:59:59. `local_ms` is the
/// local wall clock in milliseconds since the epoch (D-48).
fn exchange_available_at(local_ms: i64) -> bool {
    let second_of_day = (local_ms / 1000).rem_euclid(86_400);
    // `currentTime >= 20:00:00` (the milliseconds of "now") and `<= 23:59:59`.
    second_of_day >= 20 * 3600
}

fn exchange_available(ctx: &ScriptCtx<'_>) -> bool {
    #[cfg(test)]
    if let Some(local_ms) = tests::CLOCK.with(std::cell::Cell::get) {
        return exchange_available_at(local_ms);
    }
    let now = ctx.now_ms();
    exchange_available_at(now + l2_util::time::local_offset_millis(now))
}

impl Script for BlackMarketeerOfMammon {
    fn name(&self) -> &'static str {
        "BlackMarketeerOfMammon"
    }

    fn path(&self) -> &'static str {
        "ai/others/BlackMarketeerOfMammon"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BLACK_MARKETEER]);
        s.add_talk_id(&[BLACK_MARKETEER]);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _talker: ObjectId) -> Option<String> {
        Some(
            if exchange_available(ctx) {
                "31092-01.html"
            } else {
                "31092-02.html"
            }
            .to_owned(),
        )
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = event.to_owned();
        if event == "exchange" {
            let player = player?;
            if exchange_available(ctx) {
                if ctx.player_level(player) >= MIN_LEVEL {
                    let qs = ctx.get_quest_state(player, true)?;
                    if qs.get(ctx, "restartTime").is_some() && !qs.is_now_available(ctx) {
                        htmltext = "31092-03.html".to_owned();
                    } else if ctx.adena_of(player) >= 2_000_000 {
                        qs.set_state(ctx, STARTED);
                        ctx.take_items(player, ADENA_ID, 2_000_000);
                        ctx.give_items(player, ANCIENT_ADENA_ID, 500_000);
                        htmltext = "31092-04.html".to_owned();
                        qs.exit_quest_type(ctx, QuestType::Daily, false);
                    } else {
                        htmltext = "31092-05.html".to_owned();
                    }
                } else {
                    htmltext = "31092-06.html".to_owned();
                }
            } else {
                htmltext = "31092-02.html".to_owned();
            }
        }

        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item, pages_to};

    const NPCS: &str = r#"<npc id="31092" type="Folk" level="70" name="Black Marketeer"/>"#;
    const ITEMS: &str = r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item><item id="5575" type="EtcItem" name="Ancient Adena"><set name="is_stackable" val="true"/></item>"#;
    const DIR: &str = "data/scripts/ai/others/BlackMarketeerOfMammon";

    #[test]
    fn the_window_is_from_eight_to_midnight() {
        let hour = 3_600_000_i64;
        assert!(!exchange_available_at(19 * hour + 59 * 60_000 + 59_000));
        assert!(exchange_available_at(20 * hour));
        assert!(exchange_available_at(23 * hour + 59 * 60_000 + 59_000));
        assert!(!exchange_available_at(24 * hour));
        assert!(!exchange_available_at(0));
        assert!(exchange_available_at(3 * 86_400_000 + 21 * hour));
    }

    fn setup(level: i32, adena: i64) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(BlackMarketeerOfMammon);
        let mut pages = Vec::new();
        for n in 1..=6 {
            pages.push((
                format!("{DIR}/31092-0{n}.html"),
                format!("<html><body>page0{n}<a action=\"bypass -h Script BlackMarketeerOfMammon exchange\">e</a></body></html>"),
            ));
        }
        pages.push((
            // The Black Marketeer is a built-in `Folk` of the Seven Signs pages; its first page links here.
            "data/html/seven_signs/blkmrkt_1.htm".to_owned(),
            r#"<html><body>npc <a action="bypass Script BlackMarketeerOfMammon">Talk</a></body></html>"#
                .to_owned(),
        ));
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        let npc = t.spawn(BLACK_MARKETEER, 20);
        t.enter(1, level, vec![item(900, ADENA_ID, adena)]);
        // The NPC's page, its quest link (`onTalk`) and so the exchange link.
        t.talk(1, npc);
        t.bypass(1, "Script BlackMarketeerOfMammon");
        t.take();
        t
    }

    thread_local! {
        /// The local time the tests pretend it is (the world runs on the test's thread).
        pub(super) static CLOCK: std::cell::Cell<Option<i64>> = const { std::cell::Cell::new(None) };
    }

    const EVENING: i64 = 21 * 3_600_000;
    const MORNING: i64 = 9 * 3_600_000;

    #[test]
    fn the_exchange_runs_in_the_evening() {
        CLOCK.with(|c| c.set(Some(EVENING)));
        let mut t = setup(75, 2_500_000);
        t.bypass(1, "Script BlackMarketeerOfMammon exchange");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("page04"));
        assert_eq!(t.count(1, ADENA_ID), 500_000);
        assert_eq!(t.count(1, ANCIENT_ADENA_ID), 500_000);
        assert!(!t.take_writes().is_empty());
        // `exitQuest(DAILY)` left a `restartTime`: a second exchange waits for it.
        t.bypass(1, "Script BlackMarketeerOfMammon exchange");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("page03"));
        assert_eq!(t.count(1, ADENA_ID), 500_000);
        assert_eq!(t.count(1, ANCIENT_ADENA_ID), 500_000);
    }

    #[test]
    fn the_exchange_refusals_in_the_evening() {
        CLOCK.with(|c| c.set(Some(EVENING)));
        let mut t = setup(59, 2_500_000);
        t.bypass(1, "Script BlackMarketeerOfMammon exchange");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("page06"));
        let mut t = setup(75, 1_999_999);
        t.bypass(1, "Script BlackMarketeerOfMammon exchange");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("page05"));
        assert_eq!(t.count(1, ADENA_ID), 1_999_999);
    }

    #[test]
    fn nothing_is_sold_by_day() {
        CLOCK.with(|c| c.set(Some(MORNING)));
        let mut t = setup(75, 2_500_000);
        t.bypass(1, "Script BlackMarketeerOfMammon exchange");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("page02"));
        assert_eq!(t.count(1, ADENA_ID), 2_500_000);
    }
}
