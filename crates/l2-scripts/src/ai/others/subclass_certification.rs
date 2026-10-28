//! The sub-class certification at the village masters: a level 65 to 80 subclass gets the
//! certificate books (Emergent Ability, class ability, Master Ability, the transformation
//! sealbooks) that Avant-Garde turns into skills.
// Java: dist/game/data/scripts/ai/others/SubclassCertification/SubclassCertification.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use l2_world::script::STARTED;
use l2_world::subclass::CategoryType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const NPCS: [i32; 111] = [
    30026, 30031, 30037, 30066, 30070, 30109, 30115, 30120, 30154, 30174, 30175, 30176, 30187, 30191, 30195,
    30288, 30289, 30290, 30297, 30358, 30373, 30462, 30474, 30498, 30499, 30500, 30503, 30504, 30505, 30508,
    30511, 30512, 30513, 30520, 30525, 30565, 30594, 30595, 30676, 30677, 30681, 30685, 30687, 30689, 30694,
    30699, 30704, 30845, 30847, 30849, 30854, 30857, 30862, 30865, 30894, 30897, 30900, 30905, 30910, 30913,
    31269, 31272, 31276, 31279, 31285, 31288, 31314, 31317, 31321, 31324, 31326, 31328, 31331, 31334, 31336,
    31755, 31958, 31961, 31965, 31968, 31974, 31977, 31996, 32092, 32093, 32094, 32095, 32096, 32097, 32098,
    32145, 32146, 32147, 32150, 32153, 32154, 32157, 32158, 32160, 32171, 32193, 32199, 32202, 32213, 32214,
    32221, 32222, 32229, 32230, 32233, 32234,
];
const CERTIFICATE_EMERGENT_ABILITY: i32 = 10280;
const CERTIFICATE_MASTER_ABILITY: i32 = 10612;
/// `ABILITY_CERTIFICATES` by [`get_class_index`].
const ABILITY_CERTIFICATES: [i32; 7] = [
    10281, // Certificate - Warrior Ability
    10283, // Certificate - Rogue Ability
    10282, // Certificate - Knight Ability
    10286, // Certificate - Summoner Ability
    10284, // Certificate - Wizard Ability
    10285, // Certificate - Healer Ability
    10287, // Certificate - Enchanter Ability
];
/// `TRANSFORMATION_SEALBOOKS` by [`get_class_index`].
const TRANSFORMATION_SEALBOOKS: [i32; 7] = [
    10289, // Transformation Sealbook: Divine Warrior
    10290, // Transformation Sealbook: Divine Rogue
    10288, // Transformation Sealbook: Divine Knight
    10294, // Transformation Sealbook: Divine Summoner
    10292, // Transformation Sealbook: Divine Wizard
    10291, // Transformation Sealbook: Divine Healer
    10293, // Transformation Sealbook: Divine Enchanter
];

const MIN_LEVEL: i32 = 65;

/// Java `ai.others.SubclassCertification.SubclassCertification`.
pub struct SubclassCertification;

/// Java `getClassIndex(player)`: the sub group of the active class, -1 for none.
fn get_class_index(ctx: &ScriptCtx<'_>, player: ObjectId) -> i32 {
    let groups = [
        CategoryType::SubGroupWarrior,
        CategoryType::SubGroupRogue,
        CategoryType::SubGroupKnight,
        CategoryType::SubGroupSummoner,
        CategoryType::SubGroupWizard,
        CategoryType::SubGroupHealer,
        CategoryType::SubGroupEnchanter,
    ];
    groups
        .iter()
        .position(|&g| ctx.world().is_in_category(player, g))
        .map_or(-1, |i| i32::try_from(i).unwrap_or(-1))
}

/// `ABILITY_CERTIFICATES.get(index)` / `TRANSFORMATION_SEALBOOKS.get(index)`: `null` for -1.
fn certificate(table: &[i32; 7], index: i32) -> Option<i32> {
    usize::try_from(index).ok().and_then(|i| table.get(i).copied())
}

fn active_class(ctx: &ScriptCtx<'_>, player: ObjectId) -> i32 {
    ctx.world().player(player).map_or(-1, |p| p.data.class_id)
}

impl SubclassCertification {
    /// Java `replaceHtml(player, htmlFile, replaceClass, levelToReplace)`.
    // NOTE(java-quirk): a missing page throws in Java (`htmltext.replace` on null).
    fn replace_html(
        ctx: &ScriptCtx<'_>,
        player: ObjectId,
        html_file: &str,
        replace_class: bool,
        level_to_replace: Option<&str>,
    ) -> Option<String> {
        let mut htmltext = ctx.get_htm(player, html_file)?;
        if replace_class {
            let name = ctx.world().class_name(active_class(ctx, player));
            htmltext = htmltext.replace("%class%", &name);
        }
        if let Some(level) = level_to_replace {
            htmltext = htmltext.replace("%level%", level);
        }
        Some(htmltext)
    }

    /// Java `doCertification(player, variable, itemId, level)`: the book once per class index and
    /// level, its object id kept in the player variable `<variable><level>-<class index>`.
    fn do_certification(
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        variable: &str,
        item_id: Option<i32>,
        level: i32,
    ) -> Option<String> {
        let item_id = item_id?;
        let class_index = ctx
            .world()
            .player(player)
            .map_or(0, |p| p.data.profile.class_index);
        let var = format!("{variable}{level}-{class_index}");
        let received = ctx
            .world()
            .player_variable_text(player, &var)
            .is_some_and(|v| v != "0");
        let htmltext = if received {
            "AlreadyReceived.html".to_owned()
        } else if ctx.player_level(player) < level {
            Self::replace_html(ctx, player, "LowLevel.html", false, Some(&level.to_string()))?
        } else {
            // Add items to player's inventory
            let item = ctx.world_mut().inventory_add_item_id(player, item_id, 1)?;
            let name = ctx.world().item_message_param(item_id);
            ctx.world().send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveEarnedS1).with(name),
            );
            ctx.world_mut()
                .set_player_variable_text(player, &var, item.0.to_string());
            "GetAbility.html".to_owned()
        };
        Some(htmltext)
    }
}

impl Script for SubclassCertification {
    fn name(&self) -> &'static str {
        "SubclassCertification"
    }

    fn path(&self) -> &'static str {
        "ai/others/SubclassCertification"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&NPCS);
        s.add_talk_id(&NPCS);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        qs.set_state(ctx, STARTED);
        Some("Main.html".to_owned())
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        ctx.get_quest_state(player, false)?;
        let emergent = |ctx: &ScriptCtx<'_>, level: &str, skill_type: &str, next: &str| {
            Self::replace_html(ctx, player, "EmergentAbility.html", true, None).map(|h| {
                h.replace("%level%", level)
                    .replace("%skilltype%", skill_type)
                    .replace("%event%", next)
            })
        };
        match event {
            "GetCertified" => {
                if !ctx.world().is_subclass_active(player) {
                    Some("NotSubclass.html".to_owned())
                } else if ctx.player_level(player) < MIN_LEVEL {
                    Some("NotMinLevel.html".to_owned())
                } else {
                    // NOTE(java-quirk): `(VillageMaster) npc` throws for an NPC that is no
                    // village master (or no NPC).
                    let class = active_class(ctx, player);
                    if ctx.world().check_village_master(npc?.object_id, class)? {
                        Some("CertificationList.html".to_owned())
                    } else {
                        Some("WrongVillageMaster.html".to_owned())
                    }
                }
            }
            "Obtain65" => emergent(ctx, "65", "common skill", "lvl65Emergent"),
            "Obtain70" => emergent(ctx, "70", "common skill", "lvl70Emergent"),
            "Obtain75" => Self::replace_html(ctx, player, "ClassAbility.html", true, None),
            "Obtain80" => emergent(ctx, "80", "transformation skill", "lvl80Class"),
            "lvl65Emergent" => Self::do_certification(
                ctx,
                player,
                "EmergentAbility",
                Some(CERTIFICATE_EMERGENT_ABILITY),
                65,
            ),
            "lvl70Emergent" => Self::do_certification(
                ctx,
                player,
                "EmergentAbility",
                Some(CERTIFICATE_EMERGENT_ABILITY),
                70,
            ),
            "lvl75Master" => {
                Self::do_certification(ctx, player, "ClassAbility", Some(CERTIFICATE_MASTER_ABILITY), 75)
            }
            "lvl75Class" => {
                let item = certificate(&ABILITY_CERTIFICATES, get_class_index(ctx, player));
                Self::do_certification(ctx, player, "ClassAbility", item, 75)
            }
            "lvl80Class" => {
                let item = certificate(&TRANSFORMATION_SEALBOOKS, get_class_index(ctx, player));
                Self::do_certification(ctx, player, "ClassAbility", item, 80)
            }
            "Main.html" | "Explanation.html" | "NotObtain.html" => Some(event.to_owned()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestWrite, ScriptRegistry, SubClassHolder};

    use super::*;
    use crate::testing::{ScriptTest, names_to, pages_to, player_data};

    const DIR: &str = "data/scripts/ai/others/SubclassCertification";
    const NPCS_XML: &str = r#"<npc id="30109" type="VillageMasterFighter" level="70" name="Hannavalt"/>
        <npc id="30115" type="VillageMasterMystic" level="70" name="Jurek"/>"#;
    const ITEMS: &str = r#"
        <item id="10280" type="EtcItem" name="Certificate - Emergent Ability"/>
        <item id="10281" type="EtcItem" name="Certificate - Warrior Ability"/>
        <item id="10612" type="EtcItem" name="Certificate - Master Ability"/>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(SubclassCertification);
        let link = |e: &str| format!(r#"<a action="bypass -h Script SubclassCertification {e}">{e}</a>"#);
        let mut pages: Vec<(String, String)> = vec![
            (
                format!("{DIR}/Main.html"),
                format!("<html><body>Main {}</body></html>", link("GetCertified")),
            ),
            (
                format!("{DIR}/CertificationList.html"),
                format!(
                    "<html><body>CertificationList {}{}</body></html>",
                    link("Obtain65"),
                    link("Obtain75")
                ),
            ),
            (
                format!("{DIR}/EmergentAbility.html"),
                format!(
                    "<html><body>EmergentAbility %class% %level% %skilltype% {}</body></html>",
                    link("%event%")
                ),
            ),
            (
                format!("{DIR}/ClassAbility.html"),
                format!(
                    "<html><body>ClassAbility %class% {}{}</body></html>",
                    link("lvl75Class"),
                    link("lvl75Master")
                ),
            ),
            (
                format!("{DIR}/LowLevel.html"),
                "<html><body>LowLevel %level%</body></html>".to_owned(),
            ),
        ];
        for page in [
            "NotSubclass.html",
            "NotMinLevel.html",
            "WrongVillageMaster.html",
            "GetAbility.html",
            "AlreadyReceived.html",
        ] {
            pages.push((
                format!("{DIR}/{page}"),
                format!("<html><body>{page}</body></html>"),
            ));
        }
        for npc in [30109, 30115] {
            pages.push((
                format!("data/html/villagemaster/{npc}.htm"),
                r#"<html><body><a action="bypass -h Script SubclassCertification">cert</a></body></html>"#
                    .to_owned(),
            ));
        }
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS_XML, ITEMS, &pages);
        let fighter_master = t.spawn(30109, 20);
        let mystic_master = t.spawn(30115, 40);
        (t, fighter_master, mystic_master)
    }

    /// A human with a Gladiator subclass (class index 1) active at `level`.
    fn enter(t: &mut ScriptTest, id: i32, level: i32, subclass: bool) {
        let mut data = player_data(id, level, vec![]);
        data.base_class = 12;
        data.class_id = if subclass { 2 } else { 12 };
        if subclass {
            data.profile.class_index = 1;
            data.profile.subclasses.subs.insert(
                1,
                SubClassHolder {
                    class_id: 2,
                    exp: 0,
                    sp: 0,
                    level,
                    class_index: 1,
                },
            );
        }
        t.enter_with(data);
        t.take();
    }

    fn open(t: &mut ScriptTest, player: i32, npc: Oid) -> String {
        t.talk(player, npc);
        t.take();
        t.bypass(player, "Script SubclassCertification");
        pages_to(&t.take(), player).pop().unwrap_or_default()
    }

    fn click(t: &mut ScriptTest, player: i32, event: &str) -> String {
        t.bypass(player, &format!("Script SubclassCertification {event}"));
        pages_to(&t.take(), player).pop().unwrap_or_default()
    }

    #[test]
    fn a_level_75_gladiator_subclass_gets_the_warrior_certificate_once() {
        let (mut t, fighter_master, _) = setup();
        enter(&mut t, 1, 75, true);
        assert!(open(&mut t, 1, fighter_master).contains("Main"));
        // `setState(STARTED)` writes the state row of the script.
        assert_eq!(
            t.take_writes(),
            [QuestWrite::InsertVar {
                char_id: 1,
                name: "SubclassCertification".to_owned(),
                var: "<state>".to_owned(),
                value: "Started".to_owned(),
            }]
        );
        assert!(click(&mut t, 1, "GetCertified").contains("CertificationList"));
        assert!(click(&mut t, 1, "Obtain75").contains("ClassAbility"));
        t.bypass(1, "Script SubclassCertification lvl75Class");
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("GetAbility.html"));
        assert!(names_to(&out, 1).contains(&"SYSTEM_MESSAGE".to_owned()));
        assert_eq!(t.count(1, 10281), 1);
        let book = t
            .world
            .player(Oid(1))
            .unwrap()
            .data
            .items
            .iter()
            .find(|i| i.item_id == 10281)
            .unwrap()
            .object_id;
        assert_eq!(
            t.world.player_variable_text(Oid(1), "ClassAbility75-1"),
            Some(book.0.to_string().as_str())
        );
        // A second time: already received.
        open(&mut t, 1, fighter_master);
        click(&mut t, 1, "GetCertified");
        click(&mut t, 1, "Obtain75");
        assert!(click(&mut t, 1, "lvl75Class").contains("AlreadyReceived.html"));
        assert_eq!(t.count(1, 10281), 1);
    }

    #[test]
    fn the_emergent_ability_page_and_the_refusals() {
        let (mut t, fighter_master, mystic_master) = setup();
        enter(&mut t, 1, 66, true);
        open(&mut t, 1, fighter_master);
        click(&mut t, 1, "GetCertified");
        let page = click(&mut t, 1, "Obtain65");
        assert!(page.contains("EmergentAbility"));
        assert!(page.contains("65 common skill"));
        assert!(page.contains("lvl65Emergent"));
        assert!(click(&mut t, 1, "lvl65Emergent").contains("GetAbility.html"));
        assert_eq!(t.count(1, 10280), 1);
        // A mystic master does not teach a Gladiator.
        assert!(open(&mut t, 1, mystic_master).contains("Main"));
        assert!(click(&mut t, 1, "GetCertified").contains("WrongVillageMaster.html"));
        // The base class, and a subclass below 65.
        enter(&mut t, 2, 80, false);
        open(&mut t, 2, fighter_master);
        assert!(click(&mut t, 2, "GetCertified").contains("NotSubclass.html"));
        enter(&mut t, 3, 64, true);
        open(&mut t, 3, fighter_master);
        assert!(click(&mut t, 3, "GetCertified").contains("NotMinLevel.html"));
    }
}
