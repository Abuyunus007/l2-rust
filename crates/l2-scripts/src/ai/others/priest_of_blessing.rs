//! The Priests of Blessing: Nevit's Voice and the Nevit hourglasses for adena, once every 20 hours.
// Java: dist/game/data/scripts/ai/others/PriestOfBlessing/PriestOfBlessing.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const PRIEST: i32 = 32783;

/// Java `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

// Items
const NEVIT_VOICE: i32 = 17094;
const HOURGLASSES: [[i32; 5]; 7] = [
    [17095, 17096, 17097, 17098, 17099],
    [17100, 17101, 17102, 17103, 17104],
    [17105, 17106, 17107, 17108, 17109],
    [17110, 17111, 17112, 17113, 17114],
    [17115, 17116, 17117, 17118, 17119],
    [17120, 17121, 17122, 17123, 17124],
    [17125, 17126, 17127, 17128, 17129],
];

// Prices
const PRICE_VOICE: i64 = 100000;
const PRICE_HOURGLASS: [i64; 7] = [4000, 30000, 110000, 310000, 970000, 2160000, 5000000];

// Locations (x, y, z, heading)
const SPAWNS: [(i32, i32, i32, i32); 17] = [
    (-84144, 243148, -3713, 8473),
    (-119694, 44553, 365, 33023),
    (45412, 48358, -3060, 50020),
    (115626, -177944, -905, 38058),
    (12086, 16583, -4587, 63000),
    (-45032, -113561, -192, 32767),
    (-83123, 150921, -3132, 2280),
    (-13921, 121937, -2989, 30212),
    (87123, -141327, -1340, 49153),
    (43520, -47590, -792, 43738),
    (148060, -55314, -2728, 40961),
    (82801, 149381, -3464, 53707),
    (82431, 53281, -1496, 16383),
    (147063, 25943, -2013, 49151),
    (111169, 221052, -3549, 0),
    (15907, 142901, -2688, 14324),
    (116973, 77245, -2697, 41760),
];

/// Java `FormatUtil.formatAdena`: groups of three digits, comma separated.
fn format_adena(amount: i64) -> String {
    let digits = amount.unsigned_abs().to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    if amount < 0 {
        format!("-{grouped}")
    } else {
        grouped
    }
}

/// Java `getHGIndex(level)`.
fn hg_index(level: i32) -> usize {
    if level < 20 {
        0
    } else if level < 40 {
        1
    } else if level < 52 {
        2
    } else if level < 61 {
        3
    } else if level < 76 {
        4
    } else if level < 80 {
        5
    } else if level < 86 {
        6
    } else {
        0
    }
}

/// The "available again in" message of both purchases.
fn send_reuse_message(ctx: &mut ScriptCtx<'_>, player: ObjectId, item: SystemMessageParam, reuse_time: i64) {
    let remaining_time = (reuse_time - ctx.now_ms()) / 1000;
    let hours = (remaining_time / 3600) as i32;
    let minutes = ((remaining_time % 3600) / 60) as i32;
    let sm = SystemMessage::new(SystemMessageId::S1WillBeAvailableForReUseAfterS2HourSS3MinuteS)
        .with(item)
        .with(SystemMessageParam::IntNumber(hours))
        .with(SystemMessageParam::IntNumber(minutes));
    ctx.world().send(player, &sm);
}

/// The stored reuse time of a variable: 0 when it is not set.
fn reuse_time_of(ctx: &ScriptCtx<'_>, player: ObjectId, name: &str) -> i64 {
    // NOTE(java-quirk): `Long.parseLong` of a bad value throws in Java; it counts as 0 here.
    ctx.player_variable(player, name)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Java `ai.others.PriestOfBlessing.PriestOfBlessing`.
pub struct PriestOfBlessing;

impl Script for PriestOfBlessing {
    fn name(&self) -> &'static str {
        "PriestOfBlessing"
    }

    fn path(&self) -> &'static str {
        "ai/others/PriestOfBlessing"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PRIEST]);
        s.add_first_talk_id(&[PRIEST]);
        s.add_talk_id(&[PRIEST]);
    }

    /// The constructor spawns the priests (`SPAWNED` keeps it to once).
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        for (x, y, z, heading) in SPAWNS {
            ctx.add_spawn(PRIEST, x, y, z, heading, false, 0, 0);
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
        let mut htmltext = Some(event.to_owned());
        if event.eq_ignore_ascii_case("buy_voice") {
            if ctx.adena_of(player) >= PRICE_VOICE {
                let reuse_time = reuse_time_of(ctx, player, "PriestOfBlessing_voice");
                if ctx.now_ms() > reuse_time {
                    ctx.take_items(player, ADENA_ID, PRICE_VOICE);
                    ctx.give_items(player, NEVIT_VOICE, 1);
                    let next = ctx.now_ms() + 20 * 3600000;
                    ctx.set_player_variable(player, "PriestOfBlessing_voice", &next.to_string());
                } else {
                    send_reuse_message(ctx, player, SystemMessageParam::ItemName(NEVIT_VOICE), reuse_time);
                }

                return None;
            }

            htmltext = Some("32783-adena.htm".to_owned());
        } else if event.eq_ignore_ascii_case("buy_hourglass") {
            let index = hg_index(ctx.player_level(player));
            let price_hourglass = PRICE_HOURGLASS[index];
            if ctx.adena_of(player) >= price_hourglass {
                let name = format!("PriestOfBlessing_hg_{index}");
                let reuse_time = reuse_time_of(ctx, player, &name);
                if ctx.now_ms() > reuse_time {
                    let hg = &HOURGLASSES[index];
                    let nevit_hourglass = hg[ctx.get_random_range(0, hg.len() as i32 - 1) as usize];
                    ctx.take_items(player, ADENA_ID, price_hourglass);
                    ctx.give_items(player, nevit_hourglass, 1);
                    let next = ctx.now_ms() + 20 * 3600000;
                    ctx.set_player_variable(player, &name, &next.to_string());
                } else {
                    send_reuse_message(
                        ctx,
                        player,
                        SystemMessageParam::Text("Nevit's Hourglass".to_owned()),
                        reuse_time,
                    );
                }

                return None;
            }

            htmltext = Some("32783-adena.htm".to_owned());
        }

        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        // NOTE(java-quirk): a missing page is a null `content` and `replace` throws; no page here.
        let content = ctx.get_htm(player, "32783.htm")?;
        Some(content.replace(
            "%donate%",
            &format_adena(PRICE_HOURGLASS[hg_index(ctx.player_level(player))]),
        ))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item, names_to, pages_to};

    const NPCS: &str = r#"<npc id="32783" type="Folk" level="70" name="Priest of Blessing"/>"#;
    const ITEMS: &str = r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="17094" type="EtcItem" name="Nevit's Voice"/>"#;
    const PAGE: &str = "data/scripts/ai/others/PriestOfBlessing/32783.htm";

    fn setup(adena: i64, level: i32) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(PriestOfBlessing);
        let mut t = ScriptTest::new(
            registry,
            NPCS,
            ITEMS,
            &[(PAGE, "<html><body>Price %donate%</body></html>")],
        );
        t.enter(1, level, vec![item(5000, 57, adena)]);
        // One of them next to the player.
        let priest = t.spawn(PRIEST, 20);
        (t, priest)
    }

    fn event(t: &mut ScriptTest, name: &str) -> Option<String> {
        let mut out = Some(String::new());
        t.world.run_as_script("PriestOfBlessing", |ctx| {
            out = PriestOfBlessing.on_event(ctx, name, None, Some(ObjectId(1)));
        });
        out
    }

    #[test]
    fn the_priests_are_spawned_when_the_script_loads() {
        let (t, _) = setup(0, 60);
        assert_eq!(t.npcs_of(PRIEST).len(), 17 + 1);
    }

    #[test]
    fn the_first_page_shows_the_hourglass_price_of_the_level() {
        let (mut t, priest) = setup(0, 60);
        t.take();
        t.talk(1, priest);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("Price 310,000"));
    }

    #[test]
    fn the_voice_costs_100000_once_every_twenty_hours() {
        let (mut t, _) = setup(250000, 60);
        t.take();
        assert_eq!(event(&mut t, "buy_voice"), None);
        assert_eq!(t.count(1, 17094), 1);
        assert_eq!(t.count(1, 57), 150000);
        // The second try is within the 20 hours: the player is told and pays nothing.
        assert_eq!(event(&mut t, "BUY_VOICE"), None);
        assert_eq!(t.count(1, 17094), 1);
        assert_eq!(t.count(1, 57), 150000);
        assert!(names_to(&t.take(), 1).iter().any(|n| n == "SYSTEM_MESSAGE"));
    }

    #[test]
    fn without_the_adena_the_page_says_so_and_other_events_are_echoed() {
        let (mut t, _) = setup(10, 60);
        assert_eq!(event(&mut t, "buy_voice").as_deref(), Some("32783-adena.htm"));
        assert_eq!(event(&mut t, "buy_hourglass").as_deref(), Some("32783-adena.htm"));
        assert_eq!(event(&mut t, "32783-2.htm").as_deref(), Some("32783-2.htm"));
    }

    #[test]
    fn an_hourglass_is_one_of_the_five_of_the_level_range() {
        let (mut t, _) = setup(1_000_000, 60);
        assert_eq!(event(&mut t, "buy_hourglass"), None);
        assert_eq!(t.count(1, 57), 1_000_000 - 310000);
        assert_eq!(hg_index(60), 3);
        assert_eq!(hg_index(86), 0);
        assert_eq!(format_adena(5000000), "5,000,000");
    }
}
