//! The Monument of Heroes: a hero's circlet and weapon (`bypass -h Script MonumentOfHeroes ...`).
// Java: dist/game/data/scripts/ai/others/MonumentOfHeroes/MonumentOfHeroes.java
//
// The pages of the monument NPCs (31690, 31769 to 31772) are `data/html/olympiad/hero_main*.htm`
// (the hero list, the certification, the reward page); this script is what the reward page's
// buttons call.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MONUMENTS: [i32; 5] = [31690, 31769, 31770, 31771, 31772];

// Items
const WINGS_OF_DESTINY_CIRCLET: i32 = 6842;
const WEAPONS: [i32; 14] = [
    6611, // Infinity Blade
    6612, // Infinity Cleaver
    6613, // Infinity Axe
    6614, // Infinity Rod
    6615, // Infinity Crusher
    6616, // Infinity Scepter
    6617, // Infinity Stinger
    6618, // Infinity Fang
    6619, // Infinity Bow
    6620, // Infinity Wing
    6621, // Infinity Spear
    9388, // Infinity Rapier
    9389, // Infinity Sword
    9390, // Infinity Shooter
];

/// Java `ai.others.MonumentOfHeroes.MonumentOfHeroes`.
pub struct MonumentOfHeroes;

impl Script for MonumentOfHeroes {
    fn name(&self) -> &'static str {
        "MonumentOfHeroes"
    }

    fn path(&self) -> &'static str {
        "ai/others/MonumentOfHeroes"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        // NOTE(java-quirk): Java registers the NPCs only `if (OlympiadConfig.OLYMPIAD_ENABLED)`.
        // The script has no `onTalk` and its id is -1, so the registration changes nothing a
        // player can see; `setup` cannot read the config, so it always registers.
        s.add_start_npc(&MONUMENTS);
        s.add_talk_id(&MONUMENTS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        match event {
            "HeroWeapon" => {
                if ctx.world().is_hero(player) {
                    return Some(
                        if ctx.has_at_least_one_quest_item(player, &WEAPONS) {
                            "already_have_weapon.htm"
                        } else {
                            "weapon_list.htm"
                        }
                        .to_owned(),
                    );
                }
                return Some("no_hero_weapon.htm".to_owned());
            }
            "HeroCirclet" => {
                if ctx.world().is_hero(player) {
                    if !ctx.has_quest_items(player, WINGS_OF_DESTINY_CIRCLET) {
                        ctx.give_items(player, WINGS_OF_DESTINY_CIRCLET, 1);
                    } else {
                        return Some("already_have_circlet.htm".to_owned());
                    }
                } else {
                    return Some("no_hero_circlet.htm".to_owned());
                }
            }
            _ => {
                // NOTE(java-quirk): `Integer.parseInt(event)` throws for any other event.
                if let Ok(weapon_id) = event.parse::<i32>() {
                    if WEAPONS.contains(&weapon_id) {
                        ctx.give_items(player, weapon_id, 1);
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, pages_to};

    const MONUMENT: i32 = 31690;
    const NPCS_XML: &str = r#"<npc id="31690" type="Folk" level="70" name="Monument"/>"#;
    const ITEMS: &str = r#"<item id="6842" type="EtcItem" name="Wings of Destiny Circlet"/>
        <item id="6611" type="EtcItem" name="Infinity Blade"/>"#;
    const DIR: &str = "data/scripts/ai/others/MonumentOfHeroes";

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(MonumentOfHeroes);
        let mut pages: Vec<(String, String)> = [
            "weapon_list.htm",
            "already_have_weapon.htm",
            "no_hero_weapon.htm",
            "already_have_circlet.htm",
            "no_hero_circlet.htm",
        ]
        .iter()
        .map(|p| (format!("{DIR}/{p}"), format!("<html><body>{p}</body></html>")))
        .collect();
        // A hero (or a noble) is shown `olympiad/hero_main.htm`, anybody else the default page.
        let links = r#"<html><body><a action="bypass -h Script MonumentOfHeroes HeroWeapon">w</a><a action="bypass -h Script MonumentOfHeroes HeroCirclet">c</a><a action="bypass -h Script MonumentOfHeroes 6611">g</a></body></html>"#;
        for path in ["data/html/default/31690.htm", "data/html/olympiad/hero_main.htm"] {
            pages.push((path.to_owned(), links.to_owned()));
        }
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS_XML, ITEMS, &pages);
        let npc = t.spawn(MONUMENT, 20);
        (t, npc)
    }

    fn page(name: &str) -> String {
        format!("<html><body>{name}</body></html>")
    }

    fn click(t: &mut ScriptTest, npc: ObjectId, event: &str) -> Vec<String> {
        t.talk(1, npc);
        t.take();
        t.bypass(1, &format!("Script MonumentOfHeroes {event}"));
        pages_to(&t.take(), 1)
    }

    #[test]
    fn a_character_that_is_no_hero_gets_the_no_hero_pages() {
        let (mut t, npc) = setup();
        t.enter(1, 80, vec![]);
        assert_eq!(click(&mut t, npc, "HeroWeapon"), [page("no_hero_weapon.htm")]);
        assert_eq!(click(&mut t, npc, "HeroCirclet"), [page("no_hero_circlet.htm")]);
        assert_eq!(t.count(1, WINGS_OF_DESTINY_CIRCLET), 0);
    }

    #[test]
    fn a_hero_gets_the_circlet_once_and_a_weapon() {
        let (mut t, npc) = setup();
        t.enter(1, 80, vec![]);
        t.world.set_hero(ObjectId(1), true);
        // The circlet comes without a page, the second request says so.
        assert!(click(&mut t, npc, "HeroCirclet").is_empty());
        assert_eq!(t.count(1, WINGS_OF_DESTINY_CIRCLET), 1);
        assert_eq!(
            click(&mut t, npc, "HeroCirclet"),
            [page("already_have_circlet.htm")]
        );
        assert_eq!(t.count(1, WINGS_OF_DESTINY_CIRCLET), 1);

        assert_eq!(click(&mut t, npc, "HeroWeapon"), [page("weapon_list.htm")]);
        assert!(click(&mut t, npc, "6611").is_empty());
        assert_eq!(t.count(1, 6611), 1);
        assert_eq!(
            click(&mut t, npc, "HeroWeapon"),
            [page("already_have_weapon.htm")]
        );
    }
}
