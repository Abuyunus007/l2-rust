//! The `CREATE TABLE` statements of every game server table this port reads or writes, copied
//! from the Java datapack's `sql/game` files (LF line endings). Tests that start a real game
//! server or its loader create all of them, so a new table cannot be forgotten in one test's list.

/// Every file of `crates/l2-db/sql/game`, in file name order.
pub const GAME_SCHEMA: &[&str] = &[
    include_str!("../sql/game/account_gsdata.sql"),
    include_str!("../sql/game/airships.sql"),
    include_str!("../sql/game/announcements.sql"),
    include_str!("../sql/game/auction.sql"),
    include_str!("../sql/game/auction_bid.sql"),
    include_str!("../sql/game/bot_reported_char_data.sql"),
    include_str!("../sql/game/buylists.sql"),
    include_str!("../sql/game/castle.sql"),
    include_str!("../sql/game/castle_doorupgrade.sql"),
    include_str!("../sql/game/castle_functions.sql"),
    include_str!("../sql/game/castle_manor_procure.sql"),
    include_str!("../sql/game/castle_manor_production.sql"),
    include_str!("../sql/game/castle_siege_guards.sql"),
    include_str!("../sql/game/castle_trapupgrade.sql"),
    include_str!("../sql/game/character_contacts.sql"),
    include_str!("../sql/game/character_friends.sql"),
    include_str!("../sql/game/character_hennas.sql"),
    include_str!("../sql/game/character_instance_time.sql"),
    include_str!("../sql/game/character_item_reuse_save.sql"),
    include_str!("../sql/game/character_macroses.sql"),
    include_str!("../sql/game/character_offline_trade.sql"),
    include_str!("../sql/game/character_offline_trade_items.sql"),
    include_str!("../sql/game/character_pet_skills_save.sql"),
    include_str!("../sql/game/character_quests.sql"),
    include_str!("../sql/game/character_raid_points.sql"),
    include_str!("../sql/game/character_recipebook.sql"),
    include_str!("../sql/game/character_recipeshoplist.sql"),
    include_str!("../sql/game/character_reco_bonus.sql"),
    include_str!("../sql/game/character_shortcuts.sql"),
    include_str!("../sql/game/character_skills.sql"),
    include_str!("../sql/game/character_skills_save.sql"),
    include_str!("../sql/game/character_subclasses.sql"),
    include_str!("../sql/game/character_summon_skills_save.sql"),
    include_str!("../sql/game/character_summons.sql"),
    include_str!("../sql/game/character_tpbookmark.sql"),
    include_str!("../sql/game/character_variables.sql"),
    include_str!("../sql/game/characters.sql"),
    include_str!("../sql/game/clan_data.sql"),
    include_str!("../sql/game/clan_notices.sql"),
    include_str!("../sql/game/clan_privs.sql"),
    include_str!("../sql/game/clan_skills.sql"),
    include_str!("../sql/game/clan_subpledges.sql"),
    include_str!("../sql/game/clan_wars.sql"),
    include_str!("../sql/game/clanhall.sql"),
    include_str!("../sql/game/clanhall_functions.sql"),
    include_str!("../sql/game/clanhall_siege_attackers.sql"),
    include_str!("../sql/game/clanhall_siege_guards.sql"),
    include_str!("../sql/game/crests.sql"),
    include_str!("../sql/game/cursed_weapons.sql"),
    include_str!("../sql/game/dimensional_rift.sql"),
    include_str!("../sql/game/fishing_championship.sql"),
    include_str!("../sql/game/global_variables.sql"),
    include_str!("../sql/game/fort.sql"),
    include_str!("../sql/game/fort_functions.sql"),
    include_str!("../sql/game/fort_siege_guards.sql"),
    include_str!("../sql/game/fort_spawnlist.sql"),
    include_str!("../sql/game/fortsiege_clans.sql"),
    include_str!("../sql/game/grandboss_data.sql"),
    include_str!("../sql/game/grandboss_list.sql"),
    include_str!("../sql/game/heroes.sql"),
    include_str!("../sql/game/heroes_diary.sql"),
    include_str!("../sql/game/item_attributes.sql"),
    include_str!("../sql/game/item_auction.sql"),
    include_str!("../sql/game/item_auction_bid.sql"),
    include_str!("../sql/game/item_elementals.sql"),
    include_str!("../sql/game/item_variables.sql"),
    include_str!("../sql/game/items.sql"),
    include_str!("../sql/game/itemsonground.sql"),
    include_str!("../sql/game/merchant_lease.sql"),
    include_str!("../sql/game/messages.sql"),
    include_str!("../sql/game/mods_wedding.sql"),
    include_str!("../sql/game/olympiad_data.sql"),
    include_str!("../sql/game/olympiad_fights.sql"),
    include_str!("../sql/game/olympiad_nobles.sql"),
    include_str!("../sql/game/olympiad_nobles_eom.sql"),
    include_str!("../sql/game/petition_feedback.sql"),
    include_str!("../sql/game/pets.sql"),
    include_str!("../sql/game/punishments.sql"),
    include_str!("../sql/game/raidboss_spawnlist.sql"),
    include_str!("../sql/game/rainbowsprings_attacker_list.sql"),
    include_str!("../sql/game/random_spawn.sql"),
    include_str!("../sql/game/random_spawn_loc.sql"),
    include_str!("../sql/game/seven_signs.sql"),
    include_str!("../sql/game/seven_signs_festival.sql"),
    include_str!("../sql/game/seven_signs_status.sql"),
    include_str!("../sql/game/siegable_clanhall.sql"),
    include_str!("../sql/game/siegable_hall_flagwar_attackers.sql"),
    include_str!("../sql/game/siegable_hall_flagwar_attackers_members.sql"),
    include_str!("../sql/game/siege_clans.sql"),
    include_str!("../sql/game/territories.sql"),
    include_str!("../sql/game/territory_registrations.sql"),
    include_str!("../sql/game/territory_spawnlist.sql"),
    include_str!("../sql/game/underground_coliseum.sql"),
];

#[cfg(test)]
mod tests {
    #[test]
    fn every_game_schema_file_is_listed() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("sql")
            .join("game");
        let count = std::fs::read_dir(dir)
            .map(|d| {
                d.filter_map(Result::ok)
                    .filter(|e| e.path().extension().is_some_and(|x| x == "sql"))
                    .count()
            })
            .unwrap_or(0);
        assert_eq!(super::GAME_SCHEMA.len(), count, "add the new file to GAME_SCHEMA");
    }
}
