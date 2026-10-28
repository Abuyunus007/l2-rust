//! The Territory Managers (36490 to 36498): the badge products, the noblesse for badges, the
//! rewards of the last war; and the `TerritoryStatus` bypass of the town NPCs.
// Java: dist/game/data/scripts/ai/others/TerritoryManagers/TerritoryManagers.java
// Java: dist/game/data/scripts/handlers/bypass/npc/TerritoryStatus.java

use l2_protocol_game::server::{ActionFailed, NpcHtmlMessage};
use l2_util::ObjectId;
use tracing::warn;

use super::{TERRITORY_ID_BASE, territory_badge};
use crate::residence::CastleId;
use crate::residence::npc::{CastleScript, Reply, ScriptFailed};
use crate::world::World;

const SCRIPT: CastleScript = CastleScript::TerritoryManagers;

/// `preciousSoul1ItemIds`, `preciousSoul2ItemIds`, `preciousSoul3ItemIds`: the items the
/// noblesse quests 241, 242 and 246 lose when `MakeMeNoble` completes them.
const PRECIOUS_SOUL_1_ITEM_IDS: [i32; 6] = [7587, 7588, 7589, 7597, 7598, 7599];
const PRECIOUS_SOUL_2_ITEM_IDS: [i32; 1] = [7595];
const PRECIOUS_SOUL_3_ITEM_IDS: [i32; 4] = [7678, 7591, 7592, 7593];
/// The Noblesse Tiara.
const NOBLESSE_TIARA: i32 = 7694;

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
                    self.territory_make_me_noble(player, item_id, min_nobless);
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

    /// `MakeMeNoble` once the checks passed: the noblesse quests are completed (those the server
    /// has, S-VM-01), the badges go, the tiara comes, the noblesse is set and the sub-class quest
    /// (Seeds of Chaos for a Kamael, Mimir's Elixir for the others) is completed too.
    fn territory_make_me_noble(&mut self, player: ObjectId, item_id: i32, min_nobless: i32) {
        // Complete the Noblesse related quests.
        // Possessor of a Precious Soul - 1 (241)
        self.process_noblesse_quest(player, 241, &PRECIOUS_SOUL_1_ITEM_IDS);
        // Possessor of a Precious Soul - 2 (242)
        self.process_noblesse_quest(player, 242, &PRECIOUS_SOUL_2_ITEM_IDS);
        // Possessor of a Precious Soul - 3 (246)
        self.process_noblesse_quest(player, 246, &PRECIOUS_SOUL_3_ITEM_IDS);
        // Possessor of a Precious Soul - 4 (247)
        self.process_noblesse_quest(player, 247, &[]);

        // Take the Territory Badges.
        self.destroy_item_by_item_id(player, item_id, i64::from(min_nobless), true);
        // Give Noblesse Tiara to the player.
        self.add_item(player, NOBLESSE_TIARA, 1, true);
        // Set Noblesse status to the player.
        self.set_noble(player, true);
        self.send_user_info(player);

        // Complete the sub-class related quest.
        // Complete quest Seeds of Chaos (236) for Kamael characters.
        // Complete quest Mimir's Elixir (235) for other races characters.
        let kamael = self.player_race(player) == Some(l2_data::Race::Kamael);
        if let Some(quest) = self.scripts.by_quest_id(if kamael { 236 } else { 235 }) {
            self.with_script(quest, |_, ctx| {
                let had = ctx.has_quest_state(player);
                if let Some(qs) = ctx.get_quest_state(player, true) {
                    if !had {
                        qs.set_state(ctx, crate::script::STARTED);
                    }
                    // Completes the quest.
                    qs.exit_quest(ctx, false, false);
                }
            });
        }

        // Remove the following items: Caradine's Letter (7678, 7679), Star of Destiny (5011),
        // Virgil's Letter (1239), Arkenia's Letter (1246).
        for delete in [7678, 7679, 5011, 1239, 1246] {
            // `deleteIfExist`: `destroyItem(DESTROY, item, npc, true)`, the whole stack.
            let item = self
                .inventory_item_by_item_id(player, delete)
                .map(|i| (i.object_id, i.count));
            if let Some((object_id, count)) = item {
                self.destroy_item_by_object_id(player, object_id, count, true);
            }
        }
    }

    /// `processNoblesseQuest(player, questId, itemIds)`: a quest the server has is started when
    /// the player has no state of it, and completed (its items taken first) unless it is already.
    fn process_noblesse_quest(&mut self, player: ObjectId, quest_id: i32, item_ids: &[i32]) {
        let Some(quest) = self.scripts.by_quest_id(quest_id) else {
            return;
        };
        self.with_script(quest, |_, ctx| {
            let had = ctx.has_quest_state(player);
            let Some(qs) = ctx.get_quest_state(player, true) else {
                return;
            };
            if !had {
                qs.set_state(ctx, crate::script::STARTED);
            }
            if !qs.is_completed(ctx) {
                // Take the quest specific items.
                for &item_id in item_ids {
                    ctx.take_items(player, item_id, -1);
                }
                // Completes the quest.
                qs.exit_quest(ctx, false, false);
            }
        });
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
