//! The Territory Managers (36490 to 36498): the badge products, the noblesse for badges, the
//! rewards of the last war; and the `TerritoryStatus` bypass of the town NPCs.
// Java: dist/game/data/scripts/ai/others/TerritoryManagers/TerritoryManagers.java
// Java: dist/game/data/scripts/handlers/bypass/npc/TerritoryStatus.java

use l2_protocol_game::server::{ActionFailed, NpcHtmlMessage};
use l2_util::ObjectId;
use tracing::{info, warn};

use super::{TERRITORY_ID_BASE, territory_badge};
use crate::residence::CastleId;
use crate::residence::npc::{CastleScript, Reply, ScriptFailed};
use crate::world::World;

const SCRIPT: CastleScript = CastleScript::TerritoryManagers;

/// The first Territory Manager (Gludio's).
pub(crate) const FIRST_MANAGER: i32 = 36490;

impl World {
    /// `onFirstTalk`.
    pub(crate) fn territory_manager_first_talk(&self, npc: ObjectId, player: ObjectId) -> String {
        if self.tw_class_level_of(player) < 2 || self.tw_level_of(player) < 40 {
            // If the player does not have the second class transfer or is under level 40, it
            // cannot continue.
            return "36490-08.html".to_owned();
        }
        format!("{}.html", self.castle_npc_id(npc))
    }

    /// `onEvent` with a player.
    pub(crate) fn territory_manager_on_event(
        &mut self,
        event: &str,
        npc: ObjectId,
        player: ObjectId,
    ) -> Reply {
        let npc_id = self.castle_npc_id(npc);
        let item_id = 13757 + (npc_id - FIRST_MANAGER);
        let territory_id = 81 + (npc_id - FIRST_MANAGER);
        let min_nobless = self.territory.settings.min_tw_badge_for_nobless;
        match event {
            "36490-04.html" => {
                // L2J Custom for minimum badges required.
                let replace = [("%badge%", min_nobless.to_string())];
                self.castle_script_send_packet(SCRIPT, player, npc, "36490-04.html", &replace);
                Ok(None)
            }
            "BuyProducts" => {
                if self.inventory_item_by_item_id(player, item_id).is_some() {
                    // If the player has at least one Territory Badges then show the multisell.
                    let list = 364_900_001 + (npc_id - FIRST_MANAGER) * 10_000;
                    self.multisell_separate_and_send(list, player, Some(npc), false, 1.0, 1.0);
                    Ok(None)
                } else {
                    // If the player does not have Territory Badges, it cannot continue.
                    Ok(Some("36490-02.html".to_owned()))
                }
            }
            "MakeMeNoble" => {
                if self.inventory_item_count(player, item_id) < i64::from(min_nobless) {
                    // If the player does not have enough Territory Badges, it cannot continue.
                    Ok(Some("36490-02.html".to_owned()))
                } else if self.players.get(&player).is_some_and(|p| p.data.profile.noble) {
                    // If the player is already Noblesse, it cannot continue.
                    Ok(Some("36490-05.html".to_owned()))
                } else if self.tw_level_of(player) < 75 {
                    // If the player is not level 75 or greater, it cannot continue.
                    Ok(Some("36490-06.html".to_owned()))
                } else {
                    // D-05: the rest completes the noblesse quests (241, 242, 246, 247 and 235 or
                    // 236) and stores the noblesse, which needs the quests and the `nobless`
                    // column of the character store; until then nothing is taken or given.
                    info!("TerritoryManagers: MakeMeNoble for {player} needs the noblesse quests (D-05).");
                    Ok(None)
                }
            }
            "CalcRewards" => {
                let reward = self.territory_calc_reward(player);
                let (file, castle) = self.territory_reward_page(reward, territory_id, "reward-1.html")?;
                let mut replace = Vec::new();
                if let Some(castle) = castle {
                    replace.push(("%castle%", castle));
                }
                if file == "reward-1.html" {
                    replace.push(("%badge%", reward[1].to_string()));
                    replace.push(("%adena%", (reward[1] * 5000).to_string()));
                }
                replace.push(("%territoryId%", territory_id.to_string()));
                replace.push(("%objectId%", npc.0.to_string()));
                self.castle_script_send_packet(SCRIPT, player, npc, file, &replace);
                self.send(player, &ActionFailed);
                Ok(None)
            }
            "ReceiveRewards" => {
                let badge_id = territory_badge(territory_id).unwrap_or(l2_data::item::ADENA_ID);
                let reward = self.territory_calc_reward(player);
                let (file, castle) = self.territory_reward_page(reward, territory_id, "reward-2.html")?;
                let mut replace = Vec::new();
                if let Some(castle) = castle {
                    replace.push(("%castle%", castle));
                }
                if file == "reward-2.html" {
                    self.add_item(player, badge_id, i64::from(reward[1]), true);
                    self.add_adena(player, i64::from(reward[1]) * 5000, true);
                    self.territory_reset_reward(player);
                }
                replace.push(("%objectId%", npc.0.to_string()));
                self.castle_script_send_packet(SCRIPT, player, npc, file, &replace);
                self.send(player, &ActionFailed);
                Ok(None)
            }
            _ => Ok(Some(event.to_owned())),
        }
    }

    /// The page of `CalcRewards` / `ReceiveRewards` and the castle name it names: nothing during
    /// the war or without points, another territory's, no badges, or `found`.
    fn territory_reward_page(
        &self,
        reward: [i32; 2],
        territory_id: i32,
        found: &'static str,
    ) -> Result<(&'static str, Option<String>), ScriptFailed> {
        let castle_name = |id: i32| {
            self.castle(CastleId(id - TERRITORY_ID_BASE))
                .map(|c| c.name.clone())
                .ok_or_else(|| {
                    warn!("TerritoryManagers: NullPointerException: no castle for territory {id}");
                    ScriptFailed
                })
        };
        if self.territory.in_progress || reward[0] == 0 {
            Ok(("reward-0a.html", None))
        } else if reward[0] != territory_id {
            Ok(("reward-0b.html", Some(castle_name(reward[0])?)))
        } else if reward[1] == 0 {
            Ok(("reward-0a.html", None))
        } else if found == "reward-1.html" {
            Ok((found, Some(castle_name(reward[0])?)))
        } else {
            Ok((found, None))
        }
    }
}

/// Java `TerritoryStatus.onCommand`: the castle's owner (or none), its tax and its kingdom.
pub(crate) fn territory_status_handler(
    world: &mut World,
    player: ObjectId,
    _command: &str,
    target: Option<ObjectId>,
) -> bool {
    let Some(npc) = target.filter(|t| world.npcs.contains_key(t)) else {
        return false;
    };
    // `npc.getCastle()` throws for an NPC without a castle.
    let Some(castle) = world.npc_castle(npc).and_then(|c| world.castle(c)).cloned() else {
        warn!("TerritoryStatus: NullPointerException: the NPC {npc} has no castle.");
        return false;
    };
    let owner = (castle.owner_id > 0).then(|| world.clan(castle.owner_id).map(|c| c.id));
    let path = if owner.is_some() {
        "data/html/territorystatus.htm"
    } else {
        "data/html/territorynoclan.htm"
    };
    let Some(mut html) = world.get_htm(player, path) else {
        if let Some(message) = world.html_from_file(player, npc.0, path) {
            world.send_html(player, &message);
        }
        return true;
    };
    if let Some(owner) = owner {
        // `ClanTable.getClan(ownerId).getName()` throws for a clan the table does not know.
        let Some(clan) = owner else {
            warn!(
                "TerritoryStatus: NullPointerException: no clan {}",
                castle.owner_id
            );
            return false;
        };
        let name = world.clan(clan).map(|c| c.name.clone()).unwrap_or_default();
        html = html.replace("%clanname%", &name);
        html = html.replace("%clanleadername%", &world.clan_leader_name(clan));
    }
    html = html.replace("%castlename%", &castle.name);
    html = html.replace("%taxpercent%", &castle.tax_percent.to_string());
    html = html.replace("%objectId%", &npc.0.to_string());
    let kingdom = if castle.id.0 > 6 {
        "The Kingdom of Elmore"
    } else {
        "The Kingdom of Aden"
    };
    html = html.replace("%territory%", kingdom);
    if let Some(message) = NpcHtmlMessage::new(npc.0, &html) {
        world.send_html(player, &message);
    }
    true
}
