#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! End to end for the social systems of M6: players log in through the in-process login server and
//! game server and use parties, trade, private stores, clans, servitors, NPC shops, warehouses,
//! exchange lists, enchant scrolls and mail.
//!
//! Every scenario is written once (`play_*`) against a `Harness` and runs twice, like
//! `e2e_m4.rs` and `e2e_m5.rs`:
//!
//! * `Net`: the real thing. The Rust login server and the Rust game server in one process, a
//!   throwaway MariaDB, `FakeGameClient`s on sockets. It runs only when `L2_TEST_DATABASE_URL`
//!   is set (CI's MariaDB job; never point it at a database with real data). Both servers listen
//!   on port 0 on loopback. After a scenario the rows the game server wrote are read back and
//!   compared with what the Java statements write (D-39).
//! * `BareWorld`: the same datapack, configuration, geodata, zones and spawns loaded the way the
//!   game server loads them, but with the world driven directly (no sockets, no database, ticks
//!   instead of time) and a seeded RNG. It always runs. The persistence commands the world sends
//!   are applied to a small model of the tables (`SimDb`), so the same row checks run here.
//!
//! A client action is an `Act`: the `ClientInput` for the bare world and the bytes of the packet
//! the real client sends for `Net`. `every_act_decodes_on_the_wire` checks that the bytes decode
//! as the packet each action stands for.
//!
//! There is no datapack and no geodata in CI: `e2e/datapack.rs` and `e2e/m3_data.rs` to
//! `e2e/m6_data.rs` write them into a temporary folder.

#[path = "e2e/datapack.rs"]
mod datapack;
#[allow(dead_code)]
#[path = "e2e/m3_data.rs"]
mod m3_data;
#[allow(dead_code)]
#[path = "e2e/m4_data.rs"]
mod m4_data;
#[allow(dead_code)]
#[path = "e2e/m5_data.rs"]
mod m5_data;
#[path = "e2e/m6_data.rs"]
mod m6_data;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use game_server::world_data::{
    DatapackData, multisell_settings, regen_settings, skill_load_options, summon_settings, world_settings,
};
use game_server::{Paths, Server, clan_settings, geo, hexid, mail_settings, npcs};
use l2_config::GameConfig;
use l2_data::{BuyListOptions, BuyLists, Datapack, DatapackOptions, MerchantPriceConfigs};
use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::{LeaveWorld, StatusUpdate, SystemMessage, SystemMessageParam};
use l2_protocol_game::{SystemMessageId, client};
use l2_protocol_login::server::PlayOk;
use l2_testkit::fake_game_client::{
    FakeGameClient, Recv, character_create, enter_world_packet, names_of, packet_name,
};
use l2_testkit::fake_login_client::FakeClient as LoginClient;
use l2_testkit::test_db::{DEFAULT_SERVER_INI, SAMPLE_HEXID, SAMPLE_SERVER_ID, TestDb};
use l2_world::{
    ClanWrite, ClientInput, IdFactory, ItemData, ItemRowOp, LeaveReason, LsEvent, ObjectId, Outbox, PacketSink,
    MailWrite, PersistCommand, PlayerData, PlayerProfile, SessionId, SkillData, StaticData, SummonWrite, World,
    WorldCommand, WorldRng,
};
use l2_world::clan::CrestWrite;
use login_server::state::{LoginState, ServerStatus as ListStatus};

use l2_db::Pool;

const ACCOUNT_A: &str = "e2em6acca";
const ACCOUNT_B: &str = "e2em6accb";
const PASSWORD: &str = "secret";
const NAME_A: &str = "Leader";
const NAME_B: &str = "Member";
/// A protocol revision the game server accepts by default.
const PROTOCOL: i32 = 273;
const ADENA_ID: i32 = 57;
const SOULSHOT_ID: i32 = 1835;
/// `StatusUpdate` attribute ids.
const CUR_HP: i32 = 0x09;
const CUR_MP: i32 = 0x0b;

/// Every table of the game schema (the files of `crates/l2-db/sql/game`), so the game server
/// finds whatever it reads or writes.
const GAME_SCHEMA: [&str; 40] = [
    include_str!("../../l2-db/sql/game/buylists.sql"),
    include_str!("../../l2-db/sql/game/character_contacts.sql"),
    include_str!("../../l2-db/sql/game/character_friends.sql"),
    include_str!("../../l2-db/sql/game/character_hennas.sql"),
    include_str!("../../l2-db/sql/game/character_instance_time.sql"),
    include_str!("../../l2-db/sql/game/character_item_reuse_save.sql"),
    include_str!("../../l2-db/sql/game/character_macroses.sql"),
    include_str!("../../l2-db/sql/game/character_pet_skills_save.sql"),
    include_str!("../../l2-db/sql/game/character_quests.sql"),
    include_str!("../../l2-db/sql/game/character_raid_points.sql"),
    include_str!("../../l2-db/sql/game/character_recipebook.sql"),
    include_str!("../../l2-db/sql/game/character_reco_bonus.sql"),
    include_str!("../../l2-db/sql/game/character_shortcuts.sql"),
    include_str!("../../l2-db/sql/game/character_skills.sql"),
    include_str!("../../l2-db/sql/game/character_skills_save.sql"),
    include_str!("../../l2-db/sql/game/character_subclasses.sql"),
    include_str!("../../l2-db/sql/game/character_summon_skills_save.sql"),
    include_str!("../../l2-db/sql/game/character_summons.sql"),
    include_str!("../../l2-db/sql/game/character_variables.sql"),
    include_str!("../../l2-db/sql/game/characters.sql"),
    include_str!("../../l2-db/sql/game/clan_data.sql"),
    include_str!("../../l2-db/sql/game/clan_notices.sql"),
    include_str!("../../l2-db/sql/game/clan_privs.sql"),
    include_str!("../../l2-db/sql/game/clan_skills.sql"),
    include_str!("../../l2-db/sql/game/clan_subpledges.sql"),
    include_str!("../../l2-db/sql/game/clan_wars.sql"),
    include_str!("../../l2-db/sql/game/crests.sql"),
    include_str!("../../l2-db/sql/game/heroes.sql"),
    include_str!("../../l2-db/sql/game/item_attributes.sql"),
    include_str!("../../l2-db/sql/game/item_elementals.sql"),
    include_str!("../../l2-db/sql/game/item_variables.sql"),
    include_str!("../../l2-db/sql/game/items.sql"),
    include_str!("../../l2-db/sql/game/itemsonground.sql"),
    include_str!("../../l2-db/sql/game/merchant_lease.sql"),
    include_str!("../../l2-db/sql/game/messages.sql"),
    include_str!("../../l2-db/sql/game/mods_wedding.sql"),
    include_str!("../../l2-db/sql/game/olympiad_nobles.sql"),
    include_str!("../../l2-db/sql/game/pets.sql"),
    include_str!("../../l2-db/sql/game/punishments.sql"),
    include_str!("../../l2-db/sql/game/seven_signs.sql"),
];

// ------------------------------------------------------------------------------------------
// The game server's folder (shared by the real server and the bare world)
// ------------------------------------------------------------------------------------------

/// `Server.ini` of the game server: port 0 on loopback, the login server's link port, packet
/// encryption on (the real client always uses it) and the test datapack.
fn game_server_ini(ls_port: u16) -> String {
    format!(
        "GameserverHostname = 127.0.0.1\nGameserverPort = 0\nLoginHost = 127.0.0.1\nLoginPort = {ls_port}\n\
         PacketEncryption = True\nDatapackRoot = ./datapack\nMaximumOnlineUsers = 10\n"
    )
}

/// Everything in the game server's `config` folder except `Database.ini`. Like the M5 rig (see
/// `e2e_m4.rs` for the reasons), with a lot of starting adena (the clan levels cost millions).
fn write_game_config(config_dir: &Path, ls_port: u16) {
    std::fs::create_dir_all(config_dir).unwrap();
    std::fs::write(config_dir.join("Server.ini"), game_server_ini(ls_port)).unwrap();
    std::fs::write(
        config_dir.join("Player.ini"),
        format!(
            "ForbiddenNames = annou\nStartingAdena = {}\nDisconnectAfterDeath = False\nEnableVitality = False\n",
            m6_data::START_ADENA
        ),
    )
    .unwrap();
    std::fs::write(
        config_dir.join("NPC.ini"),
        format!("DefaultCorpseTime = {}\n", m4_data::CORPSE_SECONDS),
    )
    .unwrap();
    std::fs::write(
        config_dir.join("General.ini"),
        "MaxNpcAnimation = 0\nMinimumChatLevel = 0\n",
    )
    .unwrap();
    std::fs::write(
        config_dir.join("GeoEngine.ini"),
        "PathFinding = 2\nGeoDataPath = ./geodata/\nAvoidObstructedPathNodes = True\n",
    )
    .unwrap();
    std::fs::write(
        config_dir.join("FloodProtector.ini"),
        "FloodProtectorCharacterSelectInterval = 0\n",
    )
    .unwrap();
    std::fs::write(
        config_dir.join("ipconfig.xml"),
        "<gameserver address=\"127.0.0.1\" />\n",
    )
    .unwrap();
    hexid::save(config_dir, SAMPLE_SERVER_ID, SAMPLE_HEXID);
}

/// The datapack of this test: the base classes and items, the world data of M3, the hunting data
/// of M4, the skills and items of M5 and the social data of M6.
fn write_datapack(root: &Path) {
    datapack::write_datapack(root);
    m3_data::write_world_data(root);
    m4_data::write_hunting_data(root);
    m5_data::write_skill_data(root);
    m6_data::write_social_data(root);
}

// ------------------------------------------------------------------------------------------
// Looking at packets
// ------------------------------------------------------------------------------------------

fn decode_any<P: Decode>(body: &[u8]) -> P {
    let skip = if body[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&body[skip..])).unwrap_or_else(|e| {
        panic!(
            "cannot decode {} as {}: {e:?}",
            packet_name(body),
            std::any::type_name::<P>()
        )
    })
}

/// Every packet of `batch` called `name`, decoded.
fn all<P: Decode>(batch: &[Vec<u8>], name: &str) -> Vec<P> {
    batch
        .iter()
        .filter(|body| packet_name(body) == name)
        .map(|body| decode_any(body))
        .collect()
}

fn names(batch: &[Vec<u8>]) -> Vec<String> {
    names_of(batch)
}

fn is_named(body: &[u8], name: &str) -> bool {
    packet_name(body) == name
}

fn count_named(batch: &[Vec<u8>], name: &str) -> usize {
    batch.iter().filter(|body| is_named(body, name)).count()
}

/// `wanted` appears in `names` in this order (other packets may be in between).
fn assert_in_order(names: &[String], wanted: &[&str]) {
    let mut rest = names.iter();
    for want in wanted {
        assert!(
            rest.any(|name| name == want),
            "`{want}` is missing or out of order in {names:?} (wanted {wanted:?})"
        );
    }
}

/// The value the last `StatusUpdate` of `object_id` in `batch` gave for `attribute`.
fn last_status(batch: &[Vec<u8>], object_id: i32, attribute: i32) -> Option<i32> {
    all::<StatusUpdate>(batch, "STATUS_UPDATE")
        .iter()
        .filter(|u| u.object_id == object_id)
        .flat_map(|u| u.attributes.iter())
        .filter(|(id, _)| *id == attribute)
        .map(|(_, value)| *value)
        .next_back()
}

fn system_messages(batch: &[Vec<u8>]) -> Vec<SystemMessage> {
    all::<SystemMessage>(batch, "SYSTEM_MESSAGE")
}

fn message_ids(batch: &[Vec<u8>]) -> Vec<SystemMessageId> {
    system_messages(batch).into_iter().map(|m| m.id).collect()
}

/// The first message of `id` in `batch`.
fn message_of(batch: &[Vec<u8>], id: SystemMessageId) -> SystemMessage {
    system_messages(batch)
        .into_iter()
        .find(|m| m.id == id)
        .unwrap_or_else(|| panic!("no {id:?} in {:?} ({:?})", message_ids(batch), names(batch)))
}

fn text(value: &str) -> SystemMessageParam {
    SystemMessageParam::Text(value.to_owned())
}

// ------------------------------------------------------------------------------------------
// Client actions
// ------------------------------------------------------------------------------------------

/// What a player does: the input the bare world takes and the packet a real client sends.
struct Act {
    input: ClientInput,
    wire: Vec<u8>,
}

impl Act {
    fn new<P: Encode>(input: ClientInput, packet: &P) -> Self {
        Self {
            input,
            wire: packet.to_bytes(),
        }
    }
}

/// One constructor per action the scenarios use. `Store`, `Warehouse`, `Clan`, `Mail` and
/// `Enchant` inputs wrap what `game_server::client::enter` makes of the packet.
mod act {
    use super::*;
    use l2_world::{ClanInput, EnchantInput, MailInput, ShopInput, StoreInput, WarehouseInput};

    pub fn click(object_id: i32) -> Act {
        Act::new(
            ClientInput::Action {
                object_id,
                action_id: 0,
            },
            &client::Action {
                object_id,
                origin_x: m3_data::START.0,
                origin_y: m3_data::START.1,
                origin_z: m3_data::START.2,
                action_id: client::Action::SIMPLE_CLICK,
            },
        )
    }

    /// A click on the ground: walk to `target` from `origin`.
    pub fn move_to(target: (i32, i32, i32), origin: (i32, i32, i32)) -> Act {
        Act::new(
            ClientInput::MoveTo {
                target_x: target.0,
                target_y: target.1,
                target_z: target.2,
                origin_x: origin.0,
                origin_y: origin.1,
                origin_z: origin.2,
                movement_mode: 1,
            },
            &client::MoveToLocation {
                target_x: target.0,
                target_y: target.1,
                target_z: target.2,
                origin_x: origin.0,
                origin_y: origin.1,
                origin_z: origin.2,
                movement_mode: 1,
            },
        )
    }

    pub fn use_skill(magic_id: i32) -> Act {
        Act::new(
            ClientInput::MagicSkillUse {
                magic_id,
                ctrl_pressed: false,
                shift_pressed: false,
            },
            &client::RequestMagicSkillUse {
                magic_id,
                ctrl_pressed: false,
                shift_pressed: false,
            },
        )
    }

    pub fn use_item(object_id: i32) -> Act {
        Act::new(
            ClientInput::UseItem {
                object_id,
                ctrl_pressed: false,
            },
            &client::UseItem {
                object_id,
                ctrl_pressed: false,
            },
        )
    }

    pub fn bypass(command: &str) -> Act {
        Act::new(
            ClientInput::Bypass {
                command: command.to_owned(),
            },
            &client::RequestBypassToServer {
                command: command.to_owned(),
            },
        )
    }

    pub fn action_use(action_id: i32) -> Act {
        let packet = client::RequestActionUse {
            action_id,
            ctrl_pressed: false,
            shift_pressed: false,
        };
        // Java: the three store actions go to the store code, the others to the action code.
        let input = if matches!(action_id, 10 | 28 | 61) {
            ClientInput::Store(StoreInput::ActionUse { action_id })
        } else {
            ClientInput::ActionUse {
                action_id,
                ctrl_pressed: false,
                shift_pressed: false,
            }
        };
        Act::new(input, &packet)
    }

    pub fn join_party(name: &str, distribution_type_id: i32) -> Act {
        Act::new(
            ClientInput::JoinParty {
                name: name.to_owned(),
                distribution_type_id,
            },
            &client::RequestJoinParty {
                name: name.to_owned(),
                party_distribution_type_id: distribution_type_id,
            },
        )
    }

    pub fn answer_join_party(response: i32) -> Act {
        Act::new(
            ClientInput::AnswerJoinParty { response },
            &client::RequestAnswerJoinParty { response },
        )
    }

    pub fn withdraw_party() -> Act {
        Act::new(ClientInput::WithdrawParty, &client::RequestWithDrawalParty)
    }

    pub fn trade_request(object_id: i32) -> Act {
        Act::new(
            ClientInput::TradeRequest { object_id },
            &client::TradeRequest { object_id },
        )
    }

    pub fn answer_trade(response: i32) -> Act {
        Act::new(
            ClientInput::AnswerTradeRequest { response },
            &client::AnswerTradeRequest { response },
        )
    }

    pub fn add_trade_item(trade_id: i32, object_id: i32, count: i64) -> Act {
        Act::new(
            ClientInput::AddTradeItem {
                trade_id,
                object_id,
                count,
            },
            &client::AddTradeItem {
                trade_id,
                object_id,
                count,
            },
        )
    }

    pub fn trade_done(response: i32) -> Act {
        Act::new(
            ClientInput::TradeDone { response },
            &client::TradeDone { response },
        )
    }

    pub fn store_msg_sell(msg: &str) -> Act {
        Act::new(
            ClientInput::Store(StoreInput::MsgSell { msg: msg.to_owned() }),
            &client::SetPrivateStoreMsgSell {
                store_msg: msg.to_owned(),
            },
        )
    }

    /// `(object id, count, price)` per item.
    pub fn store_list_sell(package_sale: bool, items: &[(i32, i64, i64)]) -> Act {
        let entries: Vec<client::SellListEntry> = items
            .iter()
            // Java names the field `itemId`; it holds the object id of the inventory item.
            .map(|&(object_id, count, price)| client::SellListEntry {
                item_id: object_id,
                count,
                price,
            })
            .collect();
        Act::new(
            ClientInput::Store(StoreInput::ListSell {
                package_sale,
                items: Some(entries.clone()),
            }),
            &client::SetPrivateStoreListSell {
                package_sale,
                items: Some(entries),
            },
        )
    }

    /// `(object id, count, price)` per item.
    pub fn store_buy(store_player_id: i32, items: &[(i32, i64, i64)]) -> Act {
        let entries: Vec<client::StoreBuyRequestEntry> = items
            .iter()
            .map(|&(object_id, count, price)| client::StoreBuyRequestEntry {
                object_id,
                count,
                price,
            })
            .collect();
        Act::new(
            ClientInput::Store(StoreInput::Buy {
                store_player_id,
                items: Some(entries.clone()),
            }),
            &client::RequestPrivateStoreBuy {
                store_player_id,
                items: Some(entries),
            },
        )
    }

    /// `(item id, count)` per product.
    pub fn buy_items(list_id: i32, items: &[(i32, i64)]) -> Act {
        let packet = client::RequestBuyItem {
            list_id,
            items: Some(
                items
                    .iter()
                    .map(|&(item_id, count)| client::BuyItemEntry { item_id, count })
                    .collect(),
            ),
        };
        Act::new(ClientInput::Shop(ShopInput::Buy(packet.clone())), &packet)
    }

    /// `(object id, item id, count)` per item.
    pub fn sell_items(list_id: i32, items: &[(i32, i32, i64)]) -> Act {
        let packet = client::RequestSellItem {
            list_id,
            items: Some(
                items
                    .iter()
                    .map(|&(object_id, item_id, count)| client::SellItemEntry {
                        object_id,
                        item_id,
                        count,
                    })
                    .collect(),
            ),
        };
        Act::new(ClientInput::Shop(ShopInput::Sell(packet.clone())), &packet)
    }

    pub fn shop_ui_close() -> Act {
        Act::new(ClientInput::Shop(ShopInput::UiClose), &client::RequestBuySellUIClose)
    }

    fn warehouse_entries(items: &[(i32, i64)]) -> Vec<client::WarehouseListEntry> {
        items
            .iter()
            .map(|&(object_id, count)| client::WarehouseListEntry { object_id, count })
            .collect()
    }

    /// `(object id, count)` per item.
    pub fn warehouse_deposit(items: &[(i32, i64)]) -> Act {
        let items = Some(warehouse_entries(items));
        Act::new(
            ClientInput::Warehouse(WarehouseInput::Deposit { items: items.clone() }),
            &client::SendWareHouseDepositList { items },
        )
    }

    /// `(object id, count)` per item.
    pub fn warehouse_withdraw(items: &[(i32, i64)]) -> Act {
        let items = Some(warehouse_entries(items));
        Act::new(
            ClientInput::Warehouse(WarehouseInput::Withdraw { items: items.clone() }),
            &client::SendWareHouseWithDrawList { items },
        )
    }

    pub fn multisell_choose(list_id: i32, entry_id: i32, amount: i64) -> Act {
        Act::new(
            ClientInput::MultiSellChoose {
                list_id,
                entry_id,
                amount,
            },
            &client::MultiSellChoose::new(list_id, entry_id, amount),
        )
    }

    pub fn put_enchant_target(object_id: i32) -> Act {
        Act::new(
            ClientInput::Enchant(EnchantInput::PutTargetItem { object_id }),
            &client::RequestExTryToPutEnchantTargetItem { object_id },
        )
    }

    pub fn enchant_item(object_id: i32) -> Act {
        Act::new(
            ClientInput::Enchant(EnchantInput::Enchant { object_id }),
            &client::RequestEnchantItem { object_id },
        )
    }

    pub fn join_pledge(target: i32, pledge_type: i32) -> Act {
        Act::new(
            ClientInput::Clan(ClanInput::JoinPledge { target, pledge_type }),
            &client::RequestJoinPledge { target, pledge_type },
        )
    }

    pub fn answer_join_pledge(answer: i32) -> Act {
        Act::new(
            ClientInput::Clan(ClanInput::AnswerJoinPledge { answer }),
            &client::RequestAnswerJoinPledge { answer },
        )
    }

    pub fn set_pledge_crest(data: &[u8]) -> Act {
        let length = i32::try_from(data.len()).unwrap();
        Act::new(
            ClientInput::Clan(ClanInput::SetPledgeCrest {
                length,
                data: Some(data.to_vec()),
            }),
            &client::RequestSetPledgeCrest {
                length,
                data: Some(data.to_vec()),
            },
        )
    }

    pub fn pledge_crest(crest_id: i32) -> Act {
        Act::new(
            ClientInput::Clan(ClanInput::PledgeCrest { crest_id }),
            &client::RequestPledgeCrest { crest_id },
        )
    }

    pub fn pledge_member_list() -> Act {
        Act::new(
            ClientInput::Clan(ClanInput::PledgeMemberList),
            &client::RequestPledgeMemberList,
        )
    }

    pub fn send_post(packet: client::RequestSendPost) -> Act {
        Act::new(ClientInput::Mail(MailInput::Send(packet.clone())), &packet)
    }

    pub fn received_post_list() -> Act {
        Act::new(
            ClientInput::Mail(MailInput::ReceivedPostList),
            &client::RequestReceivedPostList,
        )
    }

    pub fn received_post(msg_id: i32) -> Act {
        Act::new(
            ClientInput::Mail(MailInput::ReceivedPost { msg_id }),
            &client::RequestReceivedPost { msg_id },
        )
    }

    pub fn post_attachment(msg_id: i32) -> Act {
        Act::new(
            ClientInput::Mail(MailInput::PostAttachment { msg_id }),
            &client::RequestPostAttachment { msg_id },
        )
    }
}

// ------------------------------------------------------------------------------------------
// The scenarios' view of the world
// ------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Who {
    A,
    B,
}

impl Who {
    fn index(self) -> usize {
        match self {
            Who::A => 0,
            Who::B => 1,
        }
    }

    fn name(self) -> &'static str {
        [NAME_A, NAME_B][self.index()]
    }
}

/// One row of `items`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ItemRow {
    object_id: i32,
    item_id: i32,
    count: i64,
    enchant_level: i32,
    loc: String,
    loc_data: i32,
}

/// What the store says about a character that left (or about a table after the scenario).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Left {
    level: i32,
    exp: i64,
    sp: i64,
    clan_id: i32,
    /// Every row of `items` owned by the character, sorted by object id.
    items: Vec<ItemRow>,
}

impl Left {
    /// The items of `item_id` in `loc`, summed.
    fn count_in(&self, item_id: i32, loc: &str) -> i64 {
        self.items
            .iter()
            .filter(|i| i.item_id == item_id && i.loc == loc)
            .map(|i| i.count)
            .sum()
    }

    /// The count of `item_id` in the inventory (worn or not).
    fn count_of(&self, item_id: i32) -> i64 {
        self.count_in(item_id, "INVENTORY") + self.count_in(item_id, "PAPERDOLL")
    }

    fn row_of(&self, item_id: i32, loc: &str) -> Option<&ItemRow> {
        self.items.iter().find(|i| i.item_id == item_id && i.loc == loc)
    }
}

/// One row of `messages` (the columns the scenarios look at).
#[derive(Debug, Clone, PartialEq, Eq)]
struct MailRow {
    message_id: i32,
    sender_id: i32,
    receiver_id: i32,
    subject: String,
    content: String,
    req_adena: i64,
    has_attachments: bool,
    unread: bool,
    deleted_by_sender: bool,
    deleted_by_receiver: bool,
}

/// The columns of `clan_data` the scenarios look at.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ClanDbRow {
    clan_id: i32,
    name: String,
    level: i32,
    leader_id: i32,
    crest_id: i32,
}

/// One row of `crests`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CrestDbRow {
    crest_id: i32,
    data: Vec<u8>,
    crest_type: i32,
}

/// One row of `character_summons`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SummonDbRow {
    owner_id: i32,
    summon_skill_id: i32,
    cur_hp: i32,
    cur_mp: i32,
    time: i32,
}

/// The two ways to play a scenario. `collect_until` returns everything the player's client got
/// since the last call, up to the packet `stop` accepts and a moment after it (the rest of that
/// event); `collect_quiet` returns what arrives in a quiet moment.
trait Harness {
    fn object_id(&self, who: Who) -> i32;
    /// Logs in, selects and enters the world; returns every packet up to the closing `SKILL_LIST`.
    async fn enter(&mut self, who: Who) -> Vec<Vec<u8>>;
    async fn act(&mut self, who: Who, act: Act);
    async fn collect_until(&mut self, who: Who, stop: &dyn Fn(&[u8]) -> bool) -> Vec<Vec<u8>>;
    async fn collect_quiet(&mut self, who: Who) -> Vec<Vec<u8>>;
    /// Lets `seconds` of game time pass (the bare world ticks; the real server sleeps).
    async fn wait_seconds(&mut self, seconds: u64);
    /// Logs out and returns what was stored.
    async fn leave(&mut self, who: Who) -> Left;
    /// The rows of `messages` as the store has them now, by message id.
    async fn mail_rows(&mut self) -> Vec<MailRow>;
    /// The rows of `clan_data` and `crests` as the store has them now, by id.
    async fn clan_rows(&mut self) -> (Vec<ClanDbRow>, Vec<CrestDbRow>);
    /// The rows of `character_summons` as the store has them now.
    async fn summon_rows(&mut self) -> Vec<SummonDbRow>;
}

// ------------------------------------------------------------------------------------------
// The bare world: the same datapack and configuration, driven without sockets or database
// ------------------------------------------------------------------------------------------

/// Packets sent to the players: `(object id, bytes)`.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

/// `(object id of the receiver, bytes)`.
type Sent = Vec<(i32, Vec<u8>)>;

impl Log {
    fn peek(&self, id: i32) -> Vec<Vec<u8>> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|(to, _)| *to == id)
            .map(|(_, bytes)| bytes.clone())
            .collect()
    }

    fn take(&self, id: i32) -> Vec<Vec<u8>> {
        let mut all = self.0.lock().unwrap();
        let (mine, others): (Vec<_>, Vec<_>) = all.drain(..).partition(|(to, _)| *to == id);
        *all = others;
        mine.into_iter().map(|(_, bytes)| bytes).collect()
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push((self.0, bytes));
    }

    fn left_world(&self, _reason: LeaveReason) {}
}

/// Everything the world sent to the persistence task, in order.
#[derive(Clone, Default)]
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

struct RecordingOutbox(Persisted);

impl Outbox for RecordingOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }

    fn ls_event(&mut self, _event: LsEvent) {}
}

impl Persisted {
    fn commands(&self) -> Vec<PersistCommand> {
        self.0.lock().unwrap().clone()
    }

    /// The `items` table as the persistence task leaves it after these commands: a full store
    /// replaces the character's inventory and worn rows, the row commands change single rows.
    fn item_table(&self) -> std::collections::BTreeMap<i32, (i32, ItemData)> {
        let mut table: std::collections::BTreeMap<i32, (i32, ItemData)> = Default::default();
        for command in self.commands() {
            match command {
                PersistCommand::StorePlayer(data) => {
                    let owner = data.object_id.0;
                    table.retain(|_, (o, item)| {
                        !(*o == owner && matches!(item.loc.as_str(), "INVENTORY" | "PAPERDOLL"))
                    });
                    for item in &data.items {
                        table.insert(item.object_id.0, (owner, item.clone()));
                    }
                }
                PersistCommand::ItemChanges {
                    owner,
                    upserts,
                    removed,
                } => {
                    for item in upserts {
                        table.insert(item.object_id.0, (owner.0, item));
                    }
                    for id in removed {
                        table.remove(&id.0);
                    }
                }
                PersistCommand::ItemRows { ops, .. } => {
                    for op in ops {
                        match op {
                            ItemRowOp::Write { owner_id, item }
                            | ItemRowOp::Update { owner_id, item }
                            | ItemRowOp::Insert { owner_id, item } => {
                                table.insert(item.object_id.0, (owner_id, item));
                            }
                            ItemRowOp::Delete { object_id } => {
                                table.remove(&object_id.0);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        table
    }

    /// The `messages` table after the mail writes of the world.
    fn mail_table(&self) -> Vec<MailRow> {
        let mut table: std::collections::BTreeMap<i32, MailRow> = Default::default();
        for command in self.commands() {
            let PersistCommand::Mail(write) = command else {
                continue;
            };
            match write {
                MailWrite::Insert(m) => {
                    table.insert(
                        m.id,
                        MailRow {
                            message_id: m.id,
                            sender_id: m.sender_id,
                            receiver_id: m.receiver_id,
                            subject: m.subject.clone(),
                            content: m.content.clone(),
                            req_adena: m.req_adena,
                            has_attachments: m.has_attachments,
                            unread: m.unread,
                            deleted_by_sender: m.deleted_by_sender,
                            deleted_by_receiver: m.deleted_by_receiver,
                        },
                    );
                }
                MailWrite::MarkRead(id) => {
                    if let Some(row) = table.get_mut(&id) {
                        row.unread = false;
                    }
                }
                MailWrite::MarkDeletedBySender(id) => {
                    if let Some(row) = table.get_mut(&id) {
                        row.deleted_by_sender = true;
                    }
                }
                MailWrite::MarkDeletedByReceiver(id) => {
                    if let Some(row) = table.get_mut(&id) {
                        row.deleted_by_receiver = true;
                    }
                }
                MailWrite::RemoveAttachments(id) => {
                    if let Some(row) = table.get_mut(&id) {
                        row.has_attachments = false;
                    }
                }
                MailWrite::Delete(id) => {
                    table.remove(&id);
                }
            }
        }
        table.into_values().collect()
    }

    /// The `character_summons` table after the summon writes of the world.
    fn summon_table(&self) -> Vec<SummonDbRow> {
        let mut table: Vec<SummonDbRow> = Vec::new();
        for command in self.commands() {
            let PersistCommand::Summon(write) = command else {
                continue;
            };
            match write {
                SummonWrite::SaveServitor {
                    owner,
                    summon_skill_id,
                    cur_hp,
                    cur_mp,
                    time,
                } => {
                    // `REPLACE INTO`: the key is (owner, skill).
                    table.retain(|r| !(r.owner_id == owner.0 && r.summon_skill_id == summon_skill_id));
                    table.push(SummonDbRow {
                        owner_id: owner.0,
                        summon_skill_id,
                        cur_hp,
                        cur_mp,
                        time,
                    });
                }
                SummonWrite::RemoveServitor { owner } => table.retain(|r| r.owner_id != owner.0),
                _ => {}
            }
        }
        table
    }

    /// The `clan_data` and `crests` tables after the clan writes of the world.
    fn clan_tables(&self) -> (Vec<ClanDbRow>, Vec<CrestDbRow>) {
        let mut clans: std::collections::BTreeMap<i32, ClanDbRow> = Default::default();
        let mut crests: std::collections::BTreeMap<i32, CrestDbRow> = Default::default();
        for command in self.commands() {
            let PersistCommand::Clan(write) = command else {
                continue;
            };
            match write {
                ClanWrite::Insert(row) => {
                    clans.insert(
                        row.clan_id,
                        ClanDbRow {
                            clan_id: row.clan_id,
                            name: row.name,
                            level: row.level,
                            leader_id: row.leader_id,
                            crest_id: row.crest_id,
                        },
                    );
                }
                ClanWrite::Level { clan_id, level } => {
                    if let Some(row) = clans.get_mut(&clan_id) {
                        row.level = level;
                    }
                }
                ClanWrite::Crest(CrestWrite::ClanCrest { clan_id, crest_id }) => {
                    if let Some(row) = clans.get_mut(&clan_id) {
                        row.crest_id = crest_id;
                    }
                }
                ClanWrite::Crest(CrestWrite::Insert {
                    crest_id,
                    data,
                    crest_type,
                }) => {
                    crests.insert(
                        crest_id,
                        CrestDbRow {
                            crest_id,
                            data,
                            crest_type,
                        },
                    );
                }
                ClanWrite::Crest(CrestWrite::Delete { crest_id }) => {
                    crests.remove(&crest_id);
                }
                _ => {}
            }
        }
        (clans.into_values().collect(), crests.into_values().collect())
    }

    fn item_rows(&self, owner: i32) -> Vec<ItemRow> {
        self.item_table()
            .into_values()
            .filter(|(o, _)| *o == owner)
            .map(|(_, item)| ItemRow {
                object_id: item.object_id.0,
                item_id: item.item_id,
                count: item.count,
                enchant_level: item.enchant_level,
                loc: item.loc,
                loc_data: item.loc_data,
            })
            .collect()
    }
}

struct BareWorld {
    world: World,
    log: Log,
    persisted: Persisted,
    players: [PlayerData; 2],
    /// Keeps the folder of the datapack (nothing reads it after the start).
    _root: tempfile::TempDir,
}

/// Far above the ids the factory hands out to the NPCs (it counts up from `RUST_FIRST_OBJECT_ID`).
const BARE_IDS: [i32; 2] = [0x7000_0001, 0x7000_0002];
/// The longest the bare world waits for an event: 60 seconds of world time (50 ms ticks).
const BARE_LIMIT_TICKS: usize = 1_200;
/// The moment after an event that is collected with it: 600 ms.
const BARE_SETTLE_TICKS: usize = 12;

/// What a new fighter owns: `(item id, count, paperdoll slot if worn)`. The same list as the
/// `initialEquipment.xml` of `m6_data.rs` (class 0).
fn start_items() -> Vec<(i32, i64, Option<usize>)> {
    vec![
        (2369, 1, Some(paperdoll::RHAND)),
        (1146, 1, Some(paperdoll::CHEST)),
        (1147, 1, Some(paperdoll::LEGS)),
        (5588, 1, None),
        (m5_data::MANA_POTION, m5_data::POTIONS_AT_START, None),
        (SOULSHOT_ID, m5_data::SOULSHOTS_AT_START, None),
        (m6_data::TEST_BLADE, m6_data::BLADES_AT_START, None),
        (m6_data::ENCHANT_SCROLL_D, m6_data::SCROLLS_AT_START, None),
        (m6_data::GEM, m6_data::GEMS_AT_START, None),
        (m6_data::ORE, m6_data::ORE_AT_START, None),
        (m6_data::BLOOD_MARK, m6_data::BLOOD_MARKS_AT_START, None),
    ]
}

/// The object id of the item `item_id` of the bare character `index` (see `bare_player`).
fn bare_item_id(index: usize, item_id: i32) -> i32 {
    let position = start_items()
        .iter()
        .position(|(id, ..)| *id == item_id)
        .expect("a starting item");
    0x7100_0000 + i32::try_from(index * 100 + position).unwrap()
}

/// `clan_leader`: the first player is level 10 with the SP of the clan levels (the clan scenario);
/// otherwise both are new level 1 fighters.
fn bare_player(index: usize, clan_leader: bool) -> PlayerData {
    let (name, account) = [(NAME_A, ACCOUNT_A), (NAME_B, ACCOUNT_B)][index];
    let mut items = vec![ItemData {
        object_id: ObjectId(0x7100_0000 + i32::try_from(index * 100 + 99).unwrap()),
        item_id: ADENA_ID,
        count: m6_data::START_ADENA,
        loc: "INVENTORY".to_owned(),
        mana_left: -1,
        time: -1,
        ..Default::default()
    }];
    for (item_id, count, slot) in start_items() {
        items.push(ItemData {
            object_id: ObjectId(bare_item_id(index, item_id)),
            item_id,
            count,
            loc: if slot.is_some() { "PAPERDOLL" } else { "INVENTORY" }.to_owned(),
            loc_data: slot.map_or(0, |s| i32::try_from(s).unwrap()),
            mana_left: -1,
            time: -1,
            ..Default::default()
        });
    }
    // The first player is the clan leader of the scenarios: level 10 and the SP of the clan levels.
    let leader = index == 0 && clan_leader;
    PlayerData {
        object_id: ObjectId(BARE_IDS[index]),
        account: account.to_owned(),
        name: name.to_owned(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 1,
        hair_style: 1,
        hair_color: 1,
        level: if leader { m6_data::LEADER_LEVEL } else { 1 },
        exp: if leader { m6_data::LEADER_EXP } else { 0 },
        sp: if leader { m6_data::LEADER_SP } else { 0 },
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: m3_data::START.0,
        y: m3_data::START.1,
        z: m3_data::START.2,
        heading: 0,
        cur_hp: 80.0,
        max_hp: 80.0,
        cur_mp: 30.0,
        max_mp: 30.0,
        cur_cp: 32.0,
        max_cp: 32.0,
        online_time: 0,
        items,
        skills: [
            m5_data::WIND_STRIKE,
            m5_data::WIND_WALK,
            m6_data::CHANT_OF_BATTLE,
            m6_data::SUMMON_KAT,
        ]
        .into_iter()
        .map(|skill_id| SkillData {
            skill_id,
            skill_level: 1,
        })
        .collect(),
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

impl BareWorld {
    /// Loads the configuration, datapack, geodata, zones, html and spawns the way
    /// `Server::start_with` does, without the database and the sockets.
    fn start(clan_leader: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        write_game_config(&config_dir, 4242);
        let data_root = root.path().join("datapack");
        write_datapack(&data_root);

        let config = GameConfig::load(&config_dir).unwrap();
        // The datapack options of the config (`options_from_config` of the server, which is
        // private): the NPC values this test sets, the rest are the defaults.
        let options = DatapackOptions {
            skills: skill_load_options(&config, &config_dir),
            max_aggro_range: config.npc.max_aggro_range,
            default_corpse_time: config.npc.default_corpse_time,
            ..DatapackOptions::default()
        };
        let datapack = Arc::new(Datapack::load_with(&data_root, &options).unwrap());
        let static_data: Arc<dyn StaticData> =
            Arc::new(DatapackData::load(Arc::clone(&datapack), &config_dir));
        let settings = world_settings(&config, &config_dir);
        let geo_settings = geo::GeoSettings::load(&config, &config_dir, &data_root);
        let engine = geo::load_engine(&geo_settings);
        let ids = Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap());

        let persisted = Persisted::default();
        let mut world = World::new(
            WorldRng::from_seed(7),
            Box::new(RecordingOutbox(persisted.clone())),
        );
        world.set_static_data(Box::new(static_data));
        world.set_settings(settings);
        world.set_regen_settings(regen_settings(&config));
        world.set_summon_settings(summon_settings(&config));
        world.set_geo(engine);
        world.set_zone_data(Arc::clone(&datapack.zones), Arc::clone(&datapack.map_regions));
        world.set_html_cache(Arc::clone(&datapack.html));
        world.set_teleporter_data(Arc::clone(&datapack.teleporters));
        world.set_hit_condition_bonus(Arc::clone(&datapack.hit_condition_bonus));
        world.set_exp_data(
            Arc::clone(&datapack.experience),
            Arc::clone(&datapack.experience_loss),
            Arc::clone(&datapack.karma_loss),
            Arc::clone(&datapack.skill_trees),
        );
        world.set_skill_learn_data(Arc::clone(&datapack.skill_learn));
        world.set_pet_skill_data(Arc::clone(&datapack.pet_skills));
        world.set_pet_data(Arc::clone(&datapack.pets));
        world.set_cubic_data(Arc::clone(&datapack.cubics));
        world.set_multisell_data(Arc::clone(&datapack.multisell), multisell_settings(&config));
        world.set_enchant_data(
            Arc::clone(&datapack.enchant_items),
            Arc::clone(&datapack.enchant_groups),
            Arc::clone(&datapack.elemental_attributes),
        );
        world.spawn_door_data(&datapack, &ids).unwrap();
        npcs::spawn_npcs(&mut world, &datapack, ids, &config, &config_dir);
        // The tables Java loads from the database at its start: nothing in a fresh one.
        world.set_clan_settings(clan_settings(&config, &config_dir));
        world.load_clans(Vec::new(), Vec::new());
        world.set_mail_settings(mail_settings(&config, &config_dir));
        world.load_mail(l2_world::MailLoad::default());
        let buy_options = BuyListOptions {
            custom_buylist_load: config.general.custom_buylist_load,
            correct_prices: config.general.correct_prices,
        };
        world.set_buy_lists(
            Arc::new(BuyLists::load(&datapack.root, &datapack.items, &buy_options)),
            Arc::new(MerchantPriceConfigs::load(&datapack.root)),
        );
        world.load_crests(Vec::new());
        Self {
            world,
            log: Log::default(),
            persisted,
            players: [bare_player(0, clan_leader), bare_player(1, clan_leader)],
            _root: root,
        }
    }

    fn input(&mut self, who: Who, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who.index() + 1).unwrap()),
            input,
        });
    }

    fn ticks(&mut self, count: usize) {
        for _ in 0..count {
            self.world.tick();
        }
    }
}

impl Harness for BareWorld {
    fn object_id(&self, who: Who) -> i32 {
        BARE_IDS[who.index()]
    }

    async fn enter(&mut self, who: Who) -> Vec<Vec<u8>> {
        let index = who.index();
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(index + 1).unwrap()),
            player: Box::new(self.players[index].clone()),
            sink: Box::new(Sink(BARE_IDS[index], self.log.clone())),
            ip: None,
        });
        let packets = self.log.take(BARE_IDS[index]);
        assert_eq!(names(&packets).last().map(String::as_str), Some("SKILL_LIST"));
        packets
    }

    async fn act(&mut self, who: Who, act: Act) {
        self.input(who, act.input);
    }

    async fn collect_until(&mut self, who: Who, stop: &dyn Fn(&[u8]) -> bool) -> Vec<Vec<u8>> {
        let id = BARE_IDS[who.index()];
        let mut examined = 0;
        for _ in 0..BARE_LIMIT_TICKS {
            let mine = self.log.peek(id);
            if mine[examined..].iter().any(|body| stop(body)) {
                // Let the rest of the event arrive, like the quiet period of a socket does.
                self.ticks(BARE_SETTLE_TICKS);
                return self.log.take(id);
            }
            examined = mine.len();
            self.world.tick();
        }
        panic!(
            "the event did not happen for {who:?} in 60 s of world time; got {:?}",
            names(&self.log.peek(id))
        );
    }

    async fn collect_quiet(&mut self, who: Who) -> Vec<Vec<u8>> {
        self.ticks(10);
        self.log.take(BARE_IDS[who.index()])
    }

    async fn wait_seconds(&mut self, seconds: u64) {
        self.ticks(usize::try_from(seconds).unwrap() * 20);
    }

    async fn leave(&mut self, who: Who) -> Left {
        let id = BARE_IDS[who.index()];
        self.input(who, ClientInput::Logout);
        self.ticks(2);
        let goodbye = self.log.take(id);
        assert_eq!(
            goodbye.last(),
            Some(&LeaveWorld.to_bytes()),
            "{:?}",
            names(&goodbye)
        );
        let data = self
            .persisted
            .commands()
            .into_iter()
            .rev()
            .find_map(|c| match c {
                PersistCommand::StorePlayer(data) if data.object_id == ObjectId(id) => Some(data),
                _ => None,
            })
            .expect("the character was stored");
        let mut items = self.persisted.item_rows(id);
        items.sort_by_key(|i| i.object_id);
        Left {
            level: data.level,
            exp: data.exp,
            sp: data.sp,
            clan_id: data.profile.clan.as_ref().map_or(0, |c| c.clan_id),
            items,
        }
    }

    async fn mail_rows(&mut self) -> Vec<MailRow> {
        self.persisted.mail_table()
    }

    async fn clan_rows(&mut self) -> (Vec<ClanDbRow>, Vec<CrestDbRow>) {
        self.persisted.clan_tables()
    }

    async fn summon_rows(&mut self) -> Vec<SummonDbRow> {
        self.persisted.summon_table()
    }
}

// ------------------------------------------------------------------------------------------
// The real servers: login server, game server, database, sockets
// ------------------------------------------------------------------------------------------

/// Waits for state that changes on another task. Longer than `l2_testkit::wait::until`: the
/// servers run unoptimised next to other tests.
async fn until_long(mut condition: impl FnMut() -> bool) {
    for _ in 0..1_200 {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("condition not met in 30 seconds");
}

struct Rig {
    db: TestDb,
    ls: login_server::Server,
    gs: Server,
    ls_state: Arc<LoginState>,
    ls_client: SocketAddr,
    gs_client: SocketAddr,
    /// The game server's pool, for looking at the rows it wrote.
    pool: Pool,
    /// Hold the server folders until the end.
    _roots: [tempfile::TempDir; 2],
}

impl Rig {
    /// A throwaway database with both schemas, the login server, and the game server registered
    /// with it. `None` (and a printed note) without `L2_TEST_DATABASE_URL`.
    async fn start(tag: &str) -> Option<Self> {
        let db = TestDb::create(tag).await?;
        for file in GAME_SCHEMA {
            let text: String = file
                .lines()
                .filter(|line| !line.trim_start().starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n");
            for statement in text.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                db.exec(statement).await;
            }
        }

        let ls_root = tempfile::tempdir().unwrap();
        db.write_server_folder(ls_root.path(), DEFAULT_SERVER_INI);
        let ls = login_server::Server::start(&login_server::Paths::resolve(
            ls_root.path(),
            Path::new("./config"),
            Path::new("./data"),
        ))
        .await
        .unwrap();
        let ls_state = Arc::clone(ls.state());
        let ls_client = ls.client_addr().unwrap();
        let link_port = ls_state.game_servers.listen_addr().unwrap().port();

        let gs_root = tempfile::tempdir().unwrap();
        db.write_server_folder(gs_root.path(), &game_server_ini(link_port));
        write_game_config(&gs_root.path().join("config"), link_port);
        write_datapack(&gs_root.path().join("datapack"));
        let gs = Server::start(&Paths::resolve(gs_root.path(), Path::new("./config")))
            .await
            .unwrap();
        let gs_client = gs.client_addr().unwrap();
        let pool = gs.state().pool.clone();

        // The game server connects to the login server by itself; clients can log in once it
        // is registered and shows as up.
        until_long(|| {
            ls_state
                .game_servers
                .registered(SAMPLE_SERVER_ID)
                .is_some_and(|server| server.authed && server.status == ListStatus::Auto)
        })
        .await;
        Some(Self {
            db,
            ls,
            gs,
            ls_state,
            ls_client,
            gs_client,
            pool,
            _roots: [ls_root, gs_root],
        })
    }

    /// Does the login server count `account` as playing on a game server?
    fn in_game(&self, account: &str) -> bool {
        self.ls_state.game_servers.is_account_on_any_gs(account)
    }

    /// Waits until the login server no longer counts `account` as playing.
    async fn wait_until_out_of_game(&self, account: &str) {
        until_long(|| !self.in_game(account)).await;
    }

    /// The whole way in: log in at the login server, ask for the game server, connect to it,
    /// handshake, `AuthLogin`, and read the character list. `chars` is the number of characters
    /// the game server must have reported to the login server (0: not checked).
    async fn login(
        &self,
        account: &str,
        chars: u8,
    ) -> (
        FakeGameClient,
        l2_protocol_game::server::CharSelectionInfo,
        PlayOk,
    ) {
        let server_id = u8::try_from(SAMPLE_SERVER_ID).unwrap();
        let mut login = LoginClient::connect(self.ls_client).await;
        let ok = login.login_ok(account, PASSWORD).await;
        // The login server asks the game server for the characters of the account (the game
        // server answers from the database) before it shows the list.
        until_long(|| {
            self.ls_state
                .clients
                .get(account)
                .is_some_and(|c| c.characters.snapshot().is_complete())
        })
        .await;
        let list = login.server_list(&ok).await;
        assert_eq!(list.servers.len(), 1, "one game server is registered");
        assert!(list.servers[0].up, "and it is up");
        if chars > 0 {
            let counts = list.char_info.expect("the character counts");
            let ours = counts
                .iter()
                .find(|c| c.server_id == server_id)
                .expect("our server");
            assert_eq!(ours.chars, chars, "the characters the game server reported");
        }
        let play = login.play_ok(&ok, server_id).await;

        let mut game = FakeGameClient::connect(self.gs_client).await;
        let key = game.handshake(PROTOCOL).await;
        assert!(key.use_encryption, "PacketEncryption is on");
        game.auth(account, &ok, &play).await;
        let list = game.char_selection_info().await;
        assert_eq!(list.max_characters, 7, "CharMaxNumber default");
        // The game server told the login server the account is on it. The login connection
        // stayed open until now, like the real client's does while it connects.
        until_long(|| self.in_game(account)).await;
        drop(login);
        (game, list, play)
    }

    /// Creates a human fighter on a fresh connection and leaves through the character screen.
    /// Returns its object id.
    async fn create_character_and_leave(&self, account: &str, name: &str) -> i32 {
        let (mut game, list, _) = self.login(account, 0).await;
        assert!(list.characters.is_empty());
        game.create_character(&character_create(name, 0)).await.unwrap();
        let list = game.goto_lobby().await;
        let created = &list.characters[0];
        assert_eq!(
            (created.x, created.y, created.z),
            m3_data::START,
            "the creation point of the datapack"
        );
        let char_id = created.object_id;
        // Java: Logout at the character screen disconnects.
        assert!(game.logout().await.is_empty());
        self.wait_until_out_of_game(account).await;
        char_id
    }

    /// The rows of `messages`, by message id. The `true`/`false` enums are Java's.
    async fn mail_rows(&self) -> Vec<MailRow> {
        type Columns = (i32, i32, i32, Option<String>, Option<String>, i64, String, String, String, String);
        let rows: Vec<Columns> = sqlx::query_as(
            "SELECT messageId, senderId, receiverId, subject, content, reqAdena, hasAttachments,              isUnread, isDeletedBySender, isDeletedByReceiver FROM messages ORDER BY messageId",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap();
        rows.into_iter()
            .map(|(message_id, sender_id, receiver_id, subject, content, req_adena, att, unread, by_s, by_r)| {
                MailRow {
                    message_id,
                    sender_id,
                    receiver_id,
                    subject: subject.unwrap_or_default(),
                    content: content.unwrap_or_default(),
                    req_adena,
                    has_attachments: att == "true",
                    unread: unread == "true",
                    deleted_by_sender: by_s == "true",
                    deleted_by_receiver: by_r == "true",
                }
            })
            .collect()
    }

    /// The rows of `character_summons`, by owner.
    async fn summon_rows(&self) -> Vec<SummonDbRow> {
        let rows: Vec<(u32, u32, Option<u32>, Option<u32>, u32)> = sqlx::query_as(
            "SELECT ownerId, summonSkillId, curHp, curMp, `time` FROM character_summons ORDER BY ownerId",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap();
        rows.into_iter()
            .map(|(owner_id, summon_skill_id, cur_hp, cur_mp, time)| SummonDbRow {
                owner_id: i32::try_from(owner_id).unwrap(),
                summon_skill_id: i32::try_from(summon_skill_id).unwrap(),
                cur_hp: i32::try_from(cur_hp.unwrap_or(0)).unwrap(),
                cur_mp: i32::try_from(cur_mp.unwrap_or(0)).unwrap(),
                time: i32::try_from(time).unwrap(),
            })
            .collect()
    }

    /// The rows of `clan_data` and `crests`, by id.
    async fn clan_rows(&self) -> (Vec<ClanDbRow>, Vec<CrestDbRow>) {
        let clans: Vec<(i32, Option<String>, Option<i32>, Option<i32>, Option<i32>)> = sqlx::query_as(
            "SELECT clan_id, clan_name, clan_level, leader_id, crest_id FROM clan_data ORDER BY clan_id",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap();
        let crests: Vec<(i32, Vec<u8>, i8)> =
            sqlx::query_as("SELECT crest_id, data, `type` FROM crests ORDER BY crest_id")
                .fetch_all(&self.pool)
                .await
                .unwrap();
        (
            clans
                .into_iter()
                .map(|(clan_id, name, level, leader_id, crest_id)| ClanDbRow {
                    clan_id,
                    name: name.unwrap_or_default(),
                    level: level.unwrap_or(0),
                    leader_id: leader_id.unwrap_or(0),
                    crest_id: crest_id.unwrap_or(0),
                })
                .collect(),
            crests
                .into_iter()
                .map(|(crest_id, data, crest_type)| CrestDbRow {
                    crest_id,
                    data,
                    crest_type: i32::from(crest_type),
                })
                .collect(),
        )
    }

    /// The rows the scenarios compare: the character's progress and every row of `items` it owns.
    async fn left_of(&self, char_id: i32) -> Left {
        let (level, exp, sp, clan_id): (Option<u8>, Option<u64>, u64, Option<u32>) = sqlx::query_as(
            "SELECT level, exp, sp, clanid FROM characters WHERE charId = ?",
        )
        .bind(char_id)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        let rows: Vec<(i32, Option<i32>, u64, Option<i32>, Option<String>, Option<i32>)> = sqlx::query_as(
            "SELECT object_id, item_id, `count`, enchant_level, loc, loc_data FROM items \
             WHERE owner_id = ? ORDER BY object_id",
        )
        .bind(char_id)
        .fetch_all(&self.pool)
        .await
        .unwrap();
        Left {
            level: i32::from(level.unwrap_or(0)),
            exp: i64::try_from(exp.unwrap_or(0)).unwrap(),
            sp: i64::try_from(sp).unwrap(),
            clan_id: clan_id.map_or(0, |c| i32::try_from(c).unwrap()),
            items: rows
                .into_iter()
                .map(|(object_id, item_id, count, enchant_level, loc, loc_data)| ItemRow {
                    object_id,
                    item_id: item_id.unwrap_or(0),
                    count: i64::try_from(count).unwrap(),
                    enchant_level: enchant_level.unwrap_or(0),
                    loc: loc.unwrap_or_default(),
                    loc_data: loc_data.unwrap_or(0),
                })
                .collect(),
        }
    }
}

/// The scenario against the real servers.
struct Net {
    rig: Rig,
    ids: [i32; 2],
    clients: [Option<FakeGameClient>; 2],
}

impl Net {
    const ACCOUNTS: [&'static str; 2] = [ACCOUNT_A, ACCOUNT_B];

    /// `clan_leader`: see `bare_player`.
    async fn start(tag: &str, clan_leader: bool) -> Option<Self> {
        let rig = Rig::start(tag).await?;
        let a = rig.create_character_and_leave(ACCOUNT_A, NAME_A).await;
        let b = rig.create_character_and_leave(ACCOUNT_B, NAME_B).await;
        if clan_leader {
            sqlx::query("UPDATE characters SET level = ?, exp = ?, sp = ? WHERE charId = ?")
                .bind(m6_data::LEADER_LEVEL)
                .bind(m6_data::LEADER_EXP)
                .bind(m6_data::LEADER_SP)
                .bind(a)
                .execute(&rig.pool)
                .await
                .unwrap();
        }
        Some(Self {
            rig,
            ids: [a, b],
            clients: [None, None],
        })
    }

    fn client(&mut self, who: Who) -> &mut FakeGameClient {
        self.clients[who.index()]
            .as_mut()
            .expect("the client is in the world")
    }

    async fn finish(self) {
        self.rig.gs.shutdown(false).await;
        self.rig.ls.shutdown(false).await;
        self.rig.db.drop_db().await;
    }
}

impl Harness for Net {
    fn object_id(&self, who: Who) -> i32 {
        self.ids[who.index()]
    }

    async fn enter(&mut self, who: Who) -> Vec<Vec<u8>> {
        let (mut game, list, play) = self.rig.login(Self::ACCOUNTS[who.index()], 1).await;
        assert_eq!(list.characters[0].object_id, self.ids[who.index()]);
        let selected = game.select_character(0).await;
        assert_eq!(selected.session_id, play.play_ok1);
        game.send_packet(&enter_world_packet()).await;
        let packets = game.read_until("SKILL_LIST").await;
        self.clients[who.index()] = Some(game);
        packets
    }

    async fn act(&mut self, who: Who, act: Act) {
        self.client(who).send(act.wire).await;
    }

    async fn collect_until(&mut self, who: Who, stop: &dyn Fn(&[u8]) -> bool) -> Vec<Vec<u8>> {
        let client = self.client(who);
        let mut batch = Vec::new();
        loop {
            // A cast takes a few seconds: wait up to 40 s for the next packet.
            match client.recv_within(Duration::from_secs(40)).await {
                Recv::Packet(body) => {
                    let done = stop(&body);
                    batch.push(body);
                    if done {
                        break;
                    }
                }
                Recv::Closed => panic!("the game server closed the connection of {who:?}"),
                Recv::Silent => panic!(
                    "the event did not happen for {who:?} in 40 s; got {:?}",
                    names(&batch)
                ),
            }
        }
        // Longer than the 300 ms delay of the `InventoryUpdate` that follows an item use.
        batch.extend(client.read_quiet(Duration::from_millis(600)).await);
        batch
    }

    async fn collect_quiet(&mut self, who: Who) -> Vec<Vec<u8>> {
        self.client(who).read_quiet(Duration::from_millis(500)).await
    }

    async fn wait_seconds(&mut self, seconds: u64) {
        tokio::time::sleep(Duration::from_secs(seconds)).await;
    }

    async fn leave(&mut self, who: Who) -> Left {
        let mut game = self.clients[who.index()]
            .take()
            .expect("the client is in the world");
        let goodbye = game.logout().await;
        assert_eq!(
            goodbye.last(),
            Some(&LeaveWorld.to_bytes()),
            "{:?}",
            names(&goodbye)
        );
        // The login server hears about the logout only after the character is stored: when it
        // no longer counts the account, the rows are already final.
        self.rig.wait_until_out_of_game(Self::ACCOUNTS[who.index()]).await;
        self.rig.left_of(self.ids[who.index()]).await
    }

    async fn mail_rows(&mut self) -> Vec<MailRow> {
        self.rig.mail_rows().await
    }

    async fn clan_rows(&mut self) -> (Vec<ClanDbRow>, Vec<CrestDbRow>) {
        self.rig.clan_rows().await
    }

    async fn summon_rows(&mut self) -> Vec<SummonDbRow> {
        self.rig.summon_rows().await
    }
}

// ------------------------------------------------------------------------------------------
// Scenario: the party
// ------------------------------------------------------------------------------------------

#[path = "e2e/m6_party.rs"]
mod party;

#[path = "e2e/m6_trade.rs"]
mod trade;

#[path = "e2e/m6_town.rs"]
mod town;

#[path = "e2e/m6_enchant.rs"]
mod enchant;

#[path = "e2e/m6_mail.rs"]
mod mail;

#[path = "e2e/m6_clan.rs"]
mod clan;

#[path = "e2e/m6_summon.rs"]
mod summon;
