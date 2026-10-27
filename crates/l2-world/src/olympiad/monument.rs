//! The Monument of Heroes: a hero's circlet and weapon (`bypass -h Script MonumentOfHeroes ...`).
// Java: dist/game/data/scripts/ai/others/MonumentOfHeroes/MonumentOfHeroes.java
// Java: mechanics/script/Quest.java (notifyEvent, showResult, showHtmlFile, giveItems)
// Java: entity/actor/Npc.java (showChatWindow, `case 31690, 31769 .. 31772`: `hero_main.htm`)
//
// The pages of the monument NPCs (31690, 31769 to 31772) are `data/html/olympiad/hero_main*.htm`
// (the hero list, the certification, the reward page); this script is what the reward page's
// buttons call. D-05 puts the scripts last, but the script is eight lines of logic on the hero
// status, so it is built in like the wedding priest (`wedding.rs`).

use l2_util::ObjectId;
use tracing::debug;

use crate::dialog::INTERACTION_DISTANCE;
use crate::target::distance_2d;
use crate::world::World;

const SCRIPT_NAME: &str = "MonumentOfHeroes";
const PATH: &str = "data/scripts/ai/others/MonumentOfHeroes";
/// `WINGS_OF_DESTINY_CIRCLET`.
const WINGS_OF_DESTINY_CIRCLET: i32 = 6842;
/// `WEAPONS`: Infinity Blade .. Infinity Shooter.
const WEAPONS: [i32; 14] = [
    6611, 6612, 6613, 6614, 6615, 6616, 6617, 6618, 6619, 6620, 6621, 9388, 9389, 9390,
];

impl World {
    /// Java `Player.processScriptEvent("MonumentOfHeroes", event)`. `true` when the script name
    /// is ours.
    pub(crate) fn monument_script_event(&mut self, player: ObjectId, script_name: &str, event: &str) -> bool {
        if script_name != SCRIPT_NAME {
            return false;
        }
        if event.is_empty() {
            return true;
        }
        let Some(talk) = self.players.get(&player).map(|p| &p.talk) else {
            return true;
        };
        let (folk, quest_npc) = (talk.last_folk_npc, talk.last_quest_npc_object);
        let in_range = |world: &Self, npc: ObjectId| match (world.position_of(player), world.position_of(npc))
        {
            (Some(me), Some(pos)) => distance_2d(me, pos) < f64::from(INTERACTION_DISTANCE),
            _ => false,
        };
        let npc = match folk.filter(|n| self.npcs.contains_key(n)) {
            Some(target) if in_range(self, target) => target,
            _ if quest_npc > 0 => match self
                .find_object(ObjectId(quest_npc))
                .filter(|o| self.npcs.contains_key(o))
            {
                Some(npc) if in_range(self, npc) => npc,
                _ => return true,
            },
            _ => return true,
        };
        if let Some(result) = self.monument_on_event(player, event) {
            self.monument_show_result(player, npc, &result);
        }
        true
    }

    /// Java `MonumentOfHeroes.onEvent(event, npc, player)`.
    fn monument_on_event(&mut self, player: ObjectId, event: &str) -> Option<String> {
        match event {
            "HeroWeapon" => {
                if self.is_hero(player) {
                    let has_weapon = WEAPONS.iter().any(|id| self.script_has_items(player, *id));
                    return Some(
                        if has_weapon {
                            "already_have_weapon.htm"
                        } else {
                            "weapon_list.htm"
                        }
                        .to_owned(),
                    );
                }
                Some("no_hero_weapon.htm".to_owned())
            }
            "HeroCirclet" => {
                if self.is_hero(player) {
                    if self.script_has_items(player, WINGS_OF_DESTINY_CIRCLET) {
                        return Some("already_have_circlet.htm".to_owned());
                    }
                    self.script_give_items(player, WINGS_OF_DESTINY_CIRCLET, 1);
                    None
                } else {
                    Some("no_hero_circlet.htm".to_owned())
                }
            }
            _ => {
                // `Integer.parseInt(event)`: a bypass that is no number throws.
                if let Ok(weapon_id) = event.parse::<i32>()
                    && WEAPONS.contains(&weapon_id)
                {
                    self.script_give_items(player, weapon_id, 1);
                }
                None
            }
        }
    }

    /// Java `Quest.showResult(player, res, npc)`.
    fn monument_show_result(&mut self, player: ObjectId, npc: ObjectId, res: &str) {
        if res.is_empty() {
            return;
        }
        if res.ends_with(".htm") || res.ends_with(".html") {
            let Some(content) = self.get_htm(player, &format!("{PATH}/{res}")) else {
                debug!("{player}: page {res} of {SCRIPT_NAME} not found.");
                return;
            };
            let name = self.object_name(player).unwrap_or_default();
            let html = content
                .replace("%objectId%", &npc.0.to_string())
                .replace("%playername%", &name);
            if let Some(message) = l2_protocol_game::server::NpcHtmlMessage::new(npc.0, &html) {
                self.send_html(player, &message);
            }
            self.send(player, &l2_protocol_game::server::ActionFailed);
        } else {
            self.send_message(player, res);
        }
    }
}
