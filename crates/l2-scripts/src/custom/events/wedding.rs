//! The wedding priest (NPC 50007): the ceremony of an engaged couple.
// Java: dist/game/data/scripts/custom/events/Wedding/Wedding.java
//
// The engagement (`.engage`), the divorce, `.gotolove` and the couple table are the world's
// `wedding.rs`; the steps the priest takes on the couple are its `wedding_*` methods.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const MANAGER_ID: i32 = 50007;

/// Java `ai`-less `custom.events.Wedding.Wedding`.
pub struct Wedding;

impl Script for Wedding {
    fn name(&self) -> &'static str {
        "Wedding"
    }

    fn path(&self) -> &'static str {
        "custom/events/Wedding"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[MANAGER_ID]);
        s.add_talk_id(&[MANAGER_ID]);
        s.add_start_npc(&[MANAGER_ID]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // The flags of the wedding system of the player (`getPartnerId()`, `isMarried()`, ...).
        let state = ctx.world().player_wedding(player).cloned().unwrap_or_default();
        if state.partner_id == 0 {
            return Some("NoPartner.html".to_owned());
        }

        let partner = ObjectId(state.partner_id);
        if ctx.world().player(partner).is_none() {
            return Some("NotFound.html".to_owned());
        }

        if state.married {
            return Some("Already.html".to_owned());
        }

        if state.marry_accepted {
            return Some("WaitForPartner.html".to_owned());
        }

        let mut htmltext = None;
        if state.marry_request {
            // NOTE(java-quirk): whatever the click was (accept or decline), the first one of the
            // player who was asked only clears the requests and shows the question again.
            if !is_wearing_formal_wear(ctx, player) || !is_wearing_formal_wear(ctx, partner) {
                htmltext = Some(send_html(ctx, partner, "NoFormal.html", None));
            } else {
                ctx.world_mut().wedding_set_marry_request(player, false);
                ctx.world_mut().wedding_set_marry_request(partner, false);
                let page = ctx.get_htm(player, "Ask.html").unwrap_or_default();
                htmltext = Some(page.replace("%player%", &ctx.player_name(partner)));
            }

            return htmltext;
        }

        match event {
            "ask" => {
                if !is_wearing_formal_wear(ctx, player) || !is_wearing_formal_wear(ctx, partner) {
                    htmltext = Some(send_html(ctx, partner, "NoFormal.html", None));
                } else {
                    ctx.world_mut().wedding_set_marry_accepted(player, true);
                    ctx.world_mut().wedding_set_marry_request(partner, true);

                    let me = ctx.player_name(player);
                    send_html(ctx, partner, "Ask.html", Some(("%player%", &me)));
                    let page = ctx.get_htm(player, "Requested.html").unwrap_or_default();
                    htmltext = Some(page.replace("%player%", &ctx.player_name(partner)));
                }
            }
            "accept" => {
                let price = i64::from(ctx.world().wedding_price());
                if !is_wearing_formal_wear(ctx, player) || !is_wearing_formal_wear(ctx, partner) {
                    htmltext = Some(send_html(ctx, partner, "NoFormal.html", None));
                } else if ctx.world().adena_of(player) < price || ctx.world().adena_of(partner) < price {
                    let fee = price.to_string();
                    htmltext = Some(send_html(ctx, partner, "Adena.html", Some(("%fee%", &fee))));
                } else {
                    ctx.world_mut().reduce_adena(player, price, true);
                    ctx.world_mut().reduce_adena(partner, price, true);

                    // Accept the wedding request.
                    ctx.world_mut().wedding_set_marry_accepted(player, true);
                    ctx.world_mut().wedding_marry_couple(state.couple_id);

                    // Messages to the couple
                    ctx.world()
                        .send_message(player, "Congratulations you are married!");
                    ctx.world_mut().wedding_set_married(player, true);
                    ctx.world_mut().wedding_set_marry_request(player, false);
                    ctx.world()
                        .send_message(partner, "Congratulations you are married!");
                    ctx.world_mut().wedding_set_married(partner, true);
                    ctx.world_mut().wedding_set_marry_request(partner, false);

                    // Wedding march
                    ctx.world_mut().wedding_march(player);
                    ctx.world_mut().wedding_march(partner);

                    // Fireworks
                    ctx.world_mut().wedding_fireworks(player);
                    ctx.world_mut().wedding_fireworks(partner);

                    let text = format!(
                        "Congratulations to {} and {}! They have been married.",
                        ctx.player_name(player),
                        ctx.player_name(partner)
                    );
                    ctx.world_mut().wedding_announce(&text);
                    htmltext = Some(send_html(ctx, partner, "Accepted.html", None));
                }
            }
            "decline" => {
                ctx.world_mut().wedding_set_marry_request(player, false);
                ctx.world_mut().wedding_set_marry_request(partner, false);
                ctx.world_mut().wedding_set_marry_accepted(player, false);
                ctx.world_mut().wedding_set_marry_accepted(partner, false);

                ctx.world()
                    .send_message(player, "You declined your partner's marriage request.");
                ctx.world()
                    .send_message(partner, "Your partner declined your marriage request.");
                htmltext = Some(send_html(ctx, partner, "Declined.html", None));
            }
            _ => {}
        }

        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let htmltext = ctx.get_htm(player, "Start.html").unwrap_or_default();
        Some(htmltext.replace("%fee%", &ctx.world().wedding_price().to_string()))
    }
}

/// Java `Wedding.sendHtml(player, fileName, regex, replacement)`: the page goes to `player` and is
/// returned.
fn send_html(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    file_name: &str,
    replace: Option<(&str, &str)>,
) -> String {
    let mut html = ctx.get_htm(player, file_name).unwrap_or_default();
    if let Some((from, to)) = replace {
        html = html.replace(from, to);
    }
    ctx.world_mut().wedding_send_page(player, &html);
    html
}

/// Java `Wedding.isWearingFormalWear(player)`.
fn is_wearing_formal_wear(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.world().wedding_is_wearing_formal_wear(player)
}

#[cfg(test)]
mod tests {
    use l2_world::wedding::{Couple, WeddingSettings};
    use l2_world::{ItemData, ScriptRegistry};

    use super::*;
    use crate::testing::{ScriptTest, pages_to, player_data};

    const PRICE: i64 = 1000;
    const ALICE: i32 = 1;
    const BOB: i32 = 2;
    const DIR: &str = "data/scripts/custom/events/Wedding";
    const NPCS_XML: &str = r#"<npc id="50007" type="Folk" level="70" name="Andromeda"/>"#;
    const ITEMS: &str = r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="6408" type="Armor" name="Formal Wear"><set name="bodypart" val="chest"/></item>"#;

    fn pages() -> Vec<(String, String)> {
        [
            (
                "Start",
                r#"<html><body>Fee %fee% <a action="bypass -h Script Wedding ask">I want to marry</a></body></html>"#,
            ),
            (
                "Ask",
                r#"<html><body>%player% asks <a action="bypass -h Script Wedding accept">Accept</a><a action="bypass -h Script Wedding decline">Decline</a></body></html>"#,
            ),
            ("Requested", "<html><body>You requested %player%</body></html>"),
            ("Accepted", "<html><body>Congratulations you are married!</body></html>"),
            ("Declined", "<html><body>Declined</body></html>"),
            ("Adena", "<html><body>You need %fee% Adena.</body></html>"),
            ("NoFormal", "<html><body>No formal wear</body></html>"),
            ("WaitForPartner", "<html><body>Wait</body></html>"),
            ("NoPartner", "<html><body>No partner</body></html>"),
            ("NotFound", "<html><body>Not found</body></html>"),
            ("Already", "<html><body>Already</body></html>"),
        ]
        .iter()
        .map(|(name, text)| (format!("{DIR}/{name}.html"), (*text).to_owned()))
        .collect()
    }

    fn adena(count: i64) -> ItemData {
        crate::testing::item(100, 57, count)
    }

    fn formal_wear(object_id: i32) -> ItemData {
        ItemData {
            loc: "PAPERDOLL".to_owned(),
            loc_data: 6,
            ..crate::testing::item(object_id, 6408, 1)
        }
    }

    /// Alice and Bob are an engaged couple (a row of `mods_wedding`), both online with the formal
    /// wear unless `bob_dressed` is false, next to the priest.
    fn scene(bob_dressed: bool, bob_adena: i64) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Wedding);
        let pages = pages();
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS_XML, ITEMS, &pages);
        t.world.set_wedding_settings(WeddingSettings {
            allow_wedding: true,
            wedding_price: i32::try_from(PRICE).unwrap(),
            ..WeddingSettings::default()
        });
        t.world.load_couples(vec![Couple {
            id: 1,
            player1_id: ALICE,
            player2_id: BOB,
            married: false,
            affiance_date: 0,
            wedding_date: 0,
        }]);
        t.enter(ALICE, 20, vec![adena(10_000), formal_wear(201)]);
        let mut bob = vec![ItemData {
            object_id: ObjectId(101),
            ..adena(bob_adena)
        }];
        if bob_dressed {
            bob.push(formal_wear(202));
        }
        t.enter(BOB, 20, bob);
        let priest = t.spawn(50007, 20);
        (t, priest)
    }

    fn texts_to(t: &ScriptTest, out: &crate::testing::Sent, to: i32) -> Vec<String> {
        let _ = t;
        crate::testing::names_to(out, to)
    }

    #[test]
    fn the_whole_wedding_at_the_priest() {
        let (mut t, priest) = scene(true, 10_000);
        t.take();
        t.talk(ALICE, priest);
        t.talk(BOB, priest);
        let out = t.take();
        // The first talk: Start.html with the fee.
        assert!(
            pages_to(&out, ALICE)[0].contains("Fee 1000"),
            "{:?}",
            pages_to(&out, ALICE)
        );

        t.bypass(ALICE, "Script Wedding ask");
        let out = t.take();
        assert!(
            pages_to(&out, BOB).iter().any(|p| p.contains("P1 asks")),
            "{:?}",
            pages_to(&out, BOB)
        );
        assert!(
            pages_to(&out, ALICE)
                .iter()
                .any(|p| p.contains("You requested P2"))
        );
        let wedding = |id: i32| t.world.player_wedding(ObjectId(id)).cloned().unwrap();
        assert!(wedding(ALICE).marry_accepted);
        assert!(wedding(BOB).marry_request);

        // Asking again waits for the partner (the page with the link is shown again first).
        t.talk(ALICE, priest);
        t.take();
        t.bypass(ALICE, "Script Wedding ask");
        assert!(pages_to(&t.take(), ALICE).iter().any(|p| p.contains("Wait")));

        // Bob's first click only clears the requests and shows the page again.
        t.bypass(BOB, "Script Wedding accept");
        let out = t.take();
        assert!(pages_to(&out, BOB).iter().any(|p| p.contains("asks")));
        let bob = t.world.player_wedding(ObjectId(BOB)).cloned().unwrap();
        assert!(!bob.married && !bob.marry_request);
        // The second one marries.
        t.bypass(BOB, "Script Wedding accept");
        let out = t.take();
        assert!(
            pages_to(&out, BOB)
                .iter()
                .any(|p| p.contains("Congratulations you are married!"))
        );
        let (alice, bob) = (
            t.world.player_wedding(ObjectId(ALICE)).cloned().unwrap(),
            t.world.player_wedding(ObjectId(BOB)).cloned().unwrap(),
        );
        assert!(alice.married && bob.married);
        assert_eq!(t.world.adena_of(ObjectId(ALICE)), 10_000 - PRICE);
        assert_eq!(t.world.adena_of(ObjectId(BOB)), 10_000 - PRICE);
        assert!(t.world.couples()[0].married);
        // The march (MagicSkillUse) and the announcement (Say2) reach both.
        for id in [ALICE, BOB] {
            let names = texts_to(&t, &out, id);
            assert!(names.contains(&"MAGIC_SKILL_USE".to_owned()), "{names:?}");
            assert!(names.contains(&"SAY2".to_owned()), "{names:?}");
        }

        // A married player is sent away.
        t.talk(ALICE, priest);
        t.take();
        t.bypass(ALICE, "Script Wedding ask");
        assert!(pages_to(&t.take(), ALICE).iter().any(|p| p.contains("Already")));
    }

    #[test]
    fn no_formal_wear_no_wedding() {
        let (mut t, priest) = scene(false, 10_000);
        t.talk(ALICE, priest);
        t.take();
        t.bypass(ALICE, "Script Wedding ask");
        let out = t.take();
        // Both get the page, the partner through the packet, the player as the result.
        assert!(pages_to(&out, ALICE).iter().any(|p| p.contains("No formal wear")));
        assert!(pages_to(&out, BOB).iter().any(|p| p.contains("No formal wear")));
        assert!(!t.world.player_wedding(ObjectId(ALICE)).unwrap().marry_accepted);
        assert_eq!(t.world.adena_of(ObjectId(ALICE)), 10_000);
    }

    #[test]
    fn a_partner_without_the_fee_gets_the_adena_page() {
        let (mut t, priest) = scene(true, 10);
        t.talk(ALICE, priest);
        t.talk(BOB, priest);
        t.bypass(ALICE, "Script Wedding ask");
        t.take();
        // The first click clears the request, the second is the real one.
        t.bypass(BOB, "Script Wedding accept");
        t.take();
        t.bypass(BOB, "Script Wedding accept");
        let out = t.take();
        assert!(
            pages_to(&out, BOB)
                .iter()
                .any(|p| p.contains("You need 1000 Adena."))
        );
        assert_eq!(t.world.adena_of(ObjectId(ALICE)), 10_000);
        assert!(!t.world.couples()[0].married);
    }

    #[test]
    fn a_decline_ends_the_request() {
        let (mut t, priest) = scene(true, 10_000);
        t.talk(ALICE, priest);
        t.talk(BOB, priest);
        t.bypass(ALICE, "Script Wedding ask");
        t.take();
        // NOTE(java-quirk): the first click only clears the request and shows the page again.
        t.bypass(BOB, "Script Wedding decline");
        t.take();
        t.bypass(BOB, "Script Wedding decline");
        let out = t.take();
        assert!(pages_to(&out, BOB).iter().any(|p| p.contains("Declined")));
        let (alice, bob) = (
            t.world.player_wedding(ObjectId(ALICE)).cloned().unwrap(),
            t.world.player_wedding(ObjectId(BOB)).cloned().unwrap(),
        );
        assert!(!alice.marry_accepted && !bob.marry_request);
    }

    #[test]
    fn a_player_without_a_partner_is_told_so() {
        let (mut t, priest) = scene(true, 10_000);
        t.enter_with(player_data(3, 20, vec![]));
        t.talk(3, priest);
        t.take();
        t.bypass(3, "Script Wedding ask");
        assert!(pages_to(&t.take(), 3).iter().any(|p| p.contains("No partner")));
    }
}
