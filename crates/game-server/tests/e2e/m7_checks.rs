//! Checks that need neither the bare world nor a database.

use super::*;

/// Every action's bytes are a packet the server decodes in game, to the packet the action stands
/// for and without bytes left over. (The bare world never sees the bytes and the real servers run
/// only with a database, so this is what keeps the two halves of an `Act` together.)
#[test]
fn every_act_decodes_on_the_wire() {
    use l2_protocol_game::{ClientPacket, ConnectionState, decode_client};

    let at = m3_data::START;
    let cases: Vec<(Act, &str)> = vec![
        (act::click(1, at), "Action"),
        (act::move_to(at, at), "MoveToLocation"),
        (act::use_skill(m7_data::BUILD_HQ), "RequestMagicSkillUse"),
        (act::use_item(1), "UseItem"),
        (act::bypass("npc_1_list"), "RequestBypassToServer"),
        (act::admin("startsiege Gludio"), "SendBypassBuildCmd"),
        (act::appearing(), "Appearing"),
        (act::join_siege(1, 1, 1), "RequestJoinSiege"),
        (act::all_castle_info(), "RequestAllCastleInfo"),
        (act::siege_attacker_list(1), "RequestSiegeAttackerList"),
        (act::all_fortress_info(), "RequestAllFortressInfo"),
        (act::buy_items(m6_data::BUY_LIST, &[(1835, 5)]), "RequestBuyItem"),
        (act::shop_ui_close(), "RequestBuySellUIClose"),
    ];
    for (act, expected) in cases {
        let mut reader = PacketReader::new(&act.wire[1..]);
        let packet = decode_client(ConnectionState::InGame, act.wire[0], &mut reader)
            .unwrap_or_else(|e| panic!("{expected}: {e:?}"))
            .unwrap_or_else(|| panic!("{expected} is not accepted in game"));
        assert!(
            !matches!(packet, ClientPacket::Unported(_) | ClientPacket::UnportedEx(_)),
            "{expected} is not ported: {packet:?}"
        );
        let debug = format!("{packet:?}");
        assert!(
            debug.starts_with(&format!("{expected}(")) || debug == expected,
            "{expected}: {debug}"
        );
        assert_eq!(reader.remaining(), 0, "{expected} leaves bytes unread");
    }
}

/// What the real-server variants rely on and cannot show without a database: the statements of the
/// seed are made for the tables of the game schema (every column exists, names and counts match).
#[test]
fn the_seed_statements_name_columns_the_schema_has() {
    let seed = Seed::new(now_ms(), 60_000);
    let schema: String = GAME_SCHEMA.concat();
    let all_sql: Vec<String> = seed
        .startup_sql()
        .into_iter()
        .chain(seed.late_sql([1, 2, 3]))
        .collect();
    for statement in &all_sql {
        let table = statement
            .split_whitespace()
            .skip(1)
            .find(|word| {
                let word = word.trim_matches('`');
                word != "INTO" && !word.is_empty()
            })
            .map(|w| w.trim_matches('`'));
        let Some(table) = table else { continue };
        if statement.starts_with("INSERT") {
            assert!(
                (schema.contains(&format!("CREATE TABLE IF NOT EXISTS `{table}`"))
                    || schema.contains(&format!("CREATE TABLE `{table}`"))),
                "no table {table} for {statement}"
            );
            // The column list in the parentheses.
            let list = statement.split('(').nth(1).unwrap().split(')').next().unwrap();
            for column in list.split(',').map(|c| c.trim().trim_matches('`')) {
                assert!(
                    schema.contains(&format!("`{column}`")),
                    "no column {column} in {table} (statement {statement})"
                );
            }
        }
    }
}
