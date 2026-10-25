//! The points of the war and the badges they are worth.
// Java: managers/TerritoryWarManager.java (giveTWQuestPoint, giveTWPoint, calcReward,
//       debugReward, resetReward)

use l2_util::ObjectId;

use super::TERRITORY_ID_BASE;
use crate::world::World;

/// `_participantPoints` indices.
const SIDE: usize = 0;
const PC_KILLS: usize = 1;
const QUESTS: usize = 2;
const NPC_KILLS: usize = 3;
const CATAPULTS: usize = 4;
const WARDS: usize = 5;
const ONLINE: usize = 6;

impl World {
    /// `_participantPoints.putIfAbsent(objectId, [siegeSide, 0, 0, 0, 0, 0, 0])`.
    fn territory_points_mut(&mut self, player: ObjectId) -> &mut [i32; 7] {
        let side = self.siege_side_of(player);
        self.territory
            .participant_points
            .entry(player)
            .or_insert([side, 0, 0, 0, 0, 0, 0])
    }

    /// Java `giveTWQuestPoint(player)` (the Territory War quests call it, D-05).
    pub fn territory_give_quest_point(&mut self, player: ObjectId) {
        self.territory_points_mut(player)[QUESTS] += 1;
    }

    /// Java `giveTWPoint(killer, victimSide, type)`: `type` 1 a kill, 3 an NPC, 4 a catapult, 5 a
    /// ward, 6 a minute online. Kills of a party (types below 5) count for every member on a
    /// side within 2000 who is not of the victim's side.
    pub fn territory_give_point(&mut self, killer: ObjectId, victim_side: i32, kind: usize) {
        if victim_side == 0 || kind > ONLINE {
            return;
        }
        let party = self.party_of(killer);
        if let Some(party) = party.filter(|_| kind < 5) {
            for member in self.party_members(party) {
                let side = self.siege_side_of(member);
                if side == victim_side || side == 0 || !self.check_if_in_range(2000, killer, member, false) {
                    continue;
                }
                self.territory_points_mut(member)[kind] += 1;
            }
        } else {
            let side = self.siege_side_of(killer);
            if side == victim_side || side == 0 {
                return;
            }
            self.territory_points_mut(killer)[kind] += 1;
        }
    }

    /// Java `calcReward(player)`: `[territory, badges]`. Ten minutes online at least, else no
    /// badges (which is also how a player who took his reward is told apart).
    pub fn territory_calc_reward(&self, player: ObjectId) -> [i32; 2] {
        let Some(temp) = self.territory.participant_points.get(&player) else {
            return [0, 0];
        };
        let mut reward = [temp[SIDE], 0];
        // Badges for being online. If char was not online at least 10 mins than he cannot get
        // rewards (also this will handle that player already get his/her rewards).
        if temp[ONLINE] < 10 {
            return reward;
        }
        reward[1] += if temp[ONLINE] > 70 {
            7
        } else {
            (f64::from(temp[ONLINE]) * 0.1) as i32
        };
        // Badges for player quests.
        reward[1] += temp[QUESTS] * 7;
        // Badges for player kills.
        if temp[PC_KILLS] < 50 {
            // `reward[1] += temp[1] * 0.1`: the compound assignment casts the double sum.
            reward[1] = (f64::from(reward[1]) + f64::from(temp[PC_KILLS]) * 0.1) as i32;
        } else if temp[PC_KILLS] < 120 {
            reward[1] += 5 + ((temp[PC_KILLS] - 50) / 14);
        } else {
            reward[1] += 10;
        }
        // Badges for territory npcs.
        reward[1] += temp[NPC_KILLS];
        // Badges for territory catapults.
        reward[1] += temp[CATAPULTS] * 2;
        // Badges for territory wards.
        reward[1] += if temp[WARDS] > 0 { 5 } else { 0 };
        // Badges for territory quest done.
        // NOTE(java-quirk): `_territoryList.get(temp[0] - 80)` throws for a side that is no
        // territory; here it adds nothing.
        if let Some(t) = self.territory.territories.get(&(temp[SIDE] - TERRITORY_ID_BASE)) {
            reward[1] += t.quest_done[0].min(10);
            reward[1] += t.quest_done[1];
            reward[1] += i32::try_from(t.owned_ward_ids().len()).unwrap_or(0);
        }
        reward
    }

    /// Java `debugReward(player)`.
    pub fn territory_debug_reward(&self, player: ObjectId) {
        let side = self.siege_side_of(player);
        self.send_message(player, &format!("Registred TerrId: {side}"));
        match self.territory.participant_points.get(&player) {
            Some(temp) => {
                self.send_message(player, &format!("TerrId: {}", temp[SIDE]));
                self.send_message(player, &format!("PcKill: {}", temp[PC_KILLS]));
                self.send_message(player, &format!("PcQuests: {}", temp[QUESTS]));
                self.send_message(player, &format!("npcKill: {}", temp[NPC_KILLS]));
                self.send_message(player, &format!("CatatKill: {}", temp[CATAPULTS]));
                self.send_message(player, &format!("WardKill: {}", temp[WARDS]));
                self.send_message(player, &format!("onlineTime: {}", temp[ONLINE]));
            }
            None => self.send_message(player, "No points for you!"),
        }
        if let Some(t) = self.territory.territories.get(&(side - TERRITORY_ID_BASE)) {
            self.send_message(player, "Your Territory's jobs:");
            self.send_message(player, &format!("npcKill: {}", t.quest_done[0]));
            self.send_message(player, &format!("WardCaptured: {}", t.quest_done[1]));
        }
    }

    /// Java `resetReward(player)`: the online minutes go back to 0.
    pub fn territory_reset_reward(&mut self, player: ObjectId) {
        if let Some(temp) = self.territory.participant_points.get_mut(&player) {
            temp[ONLINE] = 0;
        }
    }

    /// The points of a player (for tests and the GM pages).
    pub fn territory_points(&self, player: ObjectId) -> Option<[i32; 7]> {
        self.territory.participant_points.get(&player).copied()
    }
}

#[cfg(test)]
mod tests {
    /// `reward[1] += (temp[6] > 70 ? 7 : (int) (temp[6] * 0.1))` and the kill part with its
    /// compound assignment of a double.
    #[test]
    fn the_kill_and_online_badges_round_like_java() {
        let online = |m: i32| if m > 70 { 7 } else { (f64::from(m) * 0.1) as i32 };
        assert_eq!(online(10), 1);
        assert_eq!(online(69), 6);
        assert_eq!(online(71), 7);
        // `int r = 3; r += 49 * 0.1;` is `(int) (3 + 4.9)` = 7, not 3 + 4.
        let kills = |base: i32, k: i32| (f64::from(base) + f64::from(k) * 0.1) as i32;
        assert_eq!(kills(3, 49), 7);
        assert_eq!(kills(0, 9), 0);
    }
}
