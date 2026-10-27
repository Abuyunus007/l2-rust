//! The item auctions at startup: `data/ItemAuctions.xml`, the `item_auction` rows and the settings
//! (L-05).
// Java: managers/ItemAuctionManager.java (constructor)
// Java: entity/item/auction/ItemAuctionInstance.java (constructor, loadAuction: the reads)
//
// Only reads here (D-15, D-39): the world writes through `PersistCommand::ItemAuction`. The checks
// of the rows (`startingTime >= endingTime`, an unknown item, an invalid state, an expired
// auction) are the world's (`World::load_item_auctions`), so one function judges the rows of the
// start and of a re-read.

use std::collections::BTreeSet;
use std::path::Path;

use l2_config::game::GeneralConfig;
use l2_config::{GameConfig, IniFile};
use l2_db::{DbError, ItemAuctionRepository, Pool};
use l2_world::{
    AuctionItemDef, DateGenerator, InstanceDef, ItemAuctionLoad, ItemAuctionRecord, ItemAuctionSettings,
};
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use tracing::{error, info, warn};

/// The file below the datapack root.
pub const ITEM_AUCTIONS_FILE: &str = "data/ItemAuctions.xml";

/// `AltItemAuctionEnabled`, `AltItemAuctionExpiredAfter` and `AltItemAuctionTimeExtendsOnBid` of
/// `General.ini`, and `CycleDatabaseWrites` of `config/SideBySide.ini` (D-53).
///
/// The config generator leaves `AltItemAuctionTimeExtendsOnBid` out (a post-processed `long`), so
/// it is read from `General.ini` here: seconds in the file, milliseconds in Java's constant.
pub fn item_auction_settings(config: &GameConfig, config_dir: &Path) -> ItemAuctionSettings {
    let path = config_dir.join(GeneralConfig::FILE);
    let extends_seconds = match IniFile::load(&path) {
        Ok(ini) => ini.get_int("AltItemAuctionTimeExtendsOnBid", 0),
        Err(e) => {
            warn!(
                "Cannot read {}: {e}; AltItemAuctionTimeExtendsOnBid = 0.",
                path.display()
            );
            0
        }
    };
    ItemAuctionSettings {
        enabled: config.general.alt_item_auction_enabled,
        expired_after_days: config.general.alt_item_auction_expired_after,
        time_extends_on_bid_ms: i64::from(extends_seconds) * 1000,
        cycle_database_writes: crate::cycle_database_writes(config_dir),
    }
}

/// Everything the world needs at its start. With the auctions off it is empty (Java logs
/// "Disabled." and loads nothing).
pub async fn load_item_auctions(
    pool: &Pool,
    settings: &ItemAuctionSettings,
    datapack_root: &Path,
    item_exists: impl Fn(i32) -> bool,
) -> ItemAuctionLoad {
    if !settings.enabled {
        return ItemAuctionLoad {
            next_auction_id: 1,
            ..ItemAuctionLoad::default()
        };
    }
    let repo = ItemAuctionRepository::new(pool.clone());
    // `SELECT auctionId .. ORDER BY auctionId DESC LIMIT 0, 1`; a failed read leaves the ids at 1.
    let next_auction_id = match repo.highest_auction_id().await {
        Ok(Some(highest)) => highest.wrapping_add(1),
        Ok(None) => 1,
        Err(e) => {
            error!("ItemAuctionManager: Failed loading auctions: {e}");
            1
        }
    };
    let path = datapack_root.join(ITEM_AUCTIONS_FILE);
    let instances = match std::fs::read(&path) {
        Ok(bytes) => instances_from_xml(&String::from_utf8_lossy(&bytes), &item_exists),
        Err(_) => {
            warn!("ItemAuctionManager: Missing ItemAuctions.xml!");
            Vec::new()
        }
    };
    let mut auctions = Vec::new();
    for instance in &instances {
        match read_instance_records(&repo, instance.id).await {
            Ok(records) => auctions.extend(records),
            // Java: "Failed loading auctions." and the constructor returns.
            Err(e) => error!("ItemAuctionInstance: Failed loading auctions: {e}"),
        }
    }
    ItemAuctionLoad {
        next_auction_id,
        instances,
        auctions,
    }
}

/// The `item_auction` rows of an instance with their bids (`SELECT_AUCTION_ID_BY_INSTANCE_ID`,
/// then `SELECT_AUCTION_INFO` and `SELECT_PLAYERS_ID_BY_AUCTION_ID` for each id). An auction whose
/// rows cannot be read is skipped with a warning, like `loadAuction` throwing `SQLException`.
pub async fn read_instance_records(
    repo: &ItemAuctionRepository,
    instance_id: i32,
) -> Result<Vec<ItemAuctionRecord>, DbError> {
    let mut records = Vec::new();
    for auction_id in repo.auction_ids(instance_id).await? {
        let row = match repo.auction(auction_id, instance_id).await {
            Ok(Some(row)) => row,
            Ok(None) => {
                warn!("ItemAuctionInstance: Auction data not found for auction: {auction_id}");
                continue;
            }
            Err(e) => {
                warn!("ItemAuctionInstance: Failed loading auction: {auction_id}: {e}");
                continue;
            }
        };
        let bids = match repo.bids(auction_id).await {
            Ok(bids) => bids,
            Err(e) => {
                warn!("ItemAuctionInstance: Failed loading auction: {auction_id}: {e}");
                continue;
            }
        };
        records.push(ItemAuctionRecord {
            auction_id,
            instance_id,
            auction_item_id: row.auction_item_id,
            starting_time: row.starting_time,
            ending_time: row.ending_time,
            state_id: row.auction_state_id,
            bids: bids
                .into_iter()
                .map(|b| (b.player_obj_id, b.player_bid))
                .collect(),
        });
    }
    Ok(records)
}

fn attribute(element: &BytesStart<'_>, key: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attr| attr.key.as_ref() == key)
        .and_then(|attr| {
            attr.normalized_value(XmlVersion::Implicit1_0)
                .ok()
                .map(std::borrow::Cow::into_owned)
        })
}

/// An attribute of the XML as an int. `Err` is what Java does with it: a missing attribute is a
/// `NullPointerException`, a bad number a `NumberFormatException`.
enum Attr {
    Missing,
    Bad,
}

fn int_attribute(element: &BytesStart<'_>, key: &str) -> Result<i32, Attr> {
    let text = attribute(element, key).ok_or(Attr::Missing)?;
    text.trim().parse().map_err(|_| Attr::Bad)
}

/// An optional attribute (`StatSet.getInt(name, -1)`): `Err` for a bad number.
fn optional_int(element: &BytesStart<'_>, key: &str) -> Result<Option<i32>, String> {
    match attribute(element, key) {
        None => Ok(None),
        Some(text) => text
            .trim()
            .parse()
            .map(Some)
            .map_err(|_| format!("Invalid integer value for '{key}': {text}")),
    }
}

/// One `<instance>` while it is read.
struct InstanceBuilder {
    id: i32,
    generator: DateGenerator,
    items: Vec<AuctionItemDef>,
}

/// Java `ItemAuctionManager` + `ItemAuctionInstance` constructor, for the text of the XML.
///
/// Java stops at the first instance whose constructor throws (a duplicate id, no items, bad date
/// settings, a missing attribute): "Failed loading auctions from xml." and the instances before it
/// stay. An `<item>` with a bad number, a length below 1, an unknown item or a duplicate
/// `auctionItemId` is only skipped (`IllegalArgumentException`).
pub fn instances_from_xml(xml: &str, item_exists: &impl Fn(i32) -> bool) -> Vec<InstanceDef> {
    let mut reader = Reader::from_str(xml);
    let mut done: Vec<InstanceDef> = Vec::new();
    let mut seen_ids: BTreeSet<i32> = BTreeSet::new();
    let mut current: Option<InstanceBuilder> = None;
    // The `<item>` being read, so an `<extra>` inside it can set the augmentation.
    let mut open_item: Option<AuctionItemDef> = None;
    let mut aborted = false;

    // Closes the instance; false when Java's constructor throws.
    let finish = |builder: InstanceBuilder, done: &mut Vec<InstanceDef>| -> bool {
        if builder.items.is_empty() {
            error!("ItemAuctionManager: Failed loading auctions from xml: No items defined");
            return false;
        }
        done.push(InstanceDef {
            id: builder.id,
            generator: builder.generator,
            items: builder.items,
        });
        true
    };

    loop {
        let event = reader.read_event();
        match event {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                let empty = matches!(event, Ok(Event::Empty(_)));
                let name = e.name().as_ref().to_ascii_lowercase();
                match name.as_str() {
                    "instance" => {
                        let id = match int_attribute(e, "id") {
                            Ok(id) => id,
                            Err(_) => {
                                error!(
                                    "ItemAuctionManager: Failed loading auctions from xml: bad instance id"
                                );
                                aborted = true;
                                break;
                            }
                        };
                        if !seen_ids.insert(id) {
                            error!(
                                "ItemAuctionManager: Failed loading auctions from xml: Dublicated instanceId {id}"
                            );
                            aborted = true;
                            break;
                        }
                        let generator = optional_int(e, "interval")
                            .and_then(|interval| Ok((interval, optional_int(e, "day_of_week")?)))
                            .and_then(|(interval, day)| {
                                Ok((
                                    interval,
                                    day,
                                    optional_int(e, "hour_of_day")?,
                                    optional_int(e, "minute_of_hour")?,
                                ))
                            })
                            .and_then(|(interval, day, hour, minute)| {
                                DateGenerator::new(interval, day, hour, minute)
                            });
                        match generator {
                            Ok(generator) => {
                                let builder = InstanceBuilder {
                                    id,
                                    generator,
                                    items: Vec::new(),
                                };
                                if empty {
                                    // `<instance/>`: no items.
                                    if !finish(builder, &mut done) {
                                        aborted = true;
                                        break;
                                    }
                                } else {
                                    current = Some(builder);
                                }
                            }
                            Err(message) => {
                                error!("ItemAuctionManager: Failed loading auctions from xml: {message}");
                                aborted = true;
                                break;
                            }
                        }
                    }
                    "item" => {
                        let Some(builder) = current.as_mut() else {
                            continue;
                        };
                        match read_item(e, builder, item_exists) {
                            ItemRead::Item(item) => {
                                if empty {
                                    builder.items.push(item);
                                } else {
                                    open_item = Some(item);
                                }
                            }
                            // IllegalArgumentException: the item is skipped.
                            ItemRead::Skipped => {}
                            // NullPointerException: Java's constructor throws.
                            ItemRead::Fatal => {
                                error!(
                                    "ItemAuctionManager: Failed loading auctions from xml: missing item attribute"
                                );
                                aborted = true;
                                break;
                            }
                        }
                    }
                    "extra" => {
                        if let Some(item) = open_item.as_mut()
                            && let Some(text) = attribute(e, "augmentation_id")
                        {
                            // `StatSet.getInt("augmentation_id", 0)`; a bad number throws when the
                            // item is created.
                            item.augmentation_id = text.trim().parse().unwrap_or(0);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name().as_ref().to_ascii_lowercase();
                match name.as_str() {
                    "item" => {
                        if let (Some(item), Some(builder)) = (open_item.take(), current.as_mut()) {
                            builder.items.push(item);
                        }
                    }
                    "instance" => {
                        if let Some(builder) = current.take()
                            && !finish(builder, &mut done)
                        {
                            aborted = true;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => {
                error!("ItemAuctionManager: Failed loading auctions from xml: {e}");
                aborted = true;
                break;
            }
        }
    }
    if !aborted {
        info!("ItemAuctionManager: Loaded {} instance(s).", done.len());
    }
    done
}

enum ItemRead {
    Item(AuctionItemDef),
    Skipped,
    Fatal,
}

/// One `<item>` of an instance (`ItemAuctionInstance` constructor).
fn read_item(
    element: &BytesStart<'_>,
    builder: &InstanceBuilder,
    item_exists: &impl Fn(i32) -> bool,
) -> ItemRead {
    // The attributes in Java's order: `auctionLength`, `itemId`, then (after the length check)
    // `auctionItemId`, `auctionInitBid`, `itemCount`.
    let auction_length = match int_attribute(element, "auctionLength") {
        Ok(v) => v,
        Err(Attr::Missing) => return ItemRead::Fatal,
        Err(Attr::Bad) => {
            warn!("ItemAuctionInstance: Failed loading auction item: bad auctionLength");
            return ItemRead::Skipped;
        }
    };
    let item_id = match int_attribute(element, "itemId") {
        Ok(v) => v,
        Err(Attr::Missing) => return ItemRead::Fatal,
        Err(Attr::Bad) => {
            warn!("ItemAuctionInstance: Failed loading auction item: bad itemId");
            return ItemRead::Skipped;
        }
    };
    if auction_length < 1 {
        warn!(
            "ItemAuctionInstance: Failed loading auction item: auctionLength < 1 for instanceId: {}, itemId {item_id}",
            builder.id
        );
        return ItemRead::Skipped;
    }
    let mut numbers = [0_i32; 3];
    for (slot, key) in numbers
        .iter_mut()
        .zip(["auctionItemId", "auctionInitBid", "itemCount"])
    {
        match int_attribute(element, key) {
            Ok(v) => *slot = v,
            Err(Attr::Missing) => return ItemRead::Fatal,
            Err(Attr::Bad) => {
                warn!("ItemAuctionInstance: Failed loading auction item: bad {key}");
                return ItemRead::Skipped;
            }
        }
    }
    let [auction_item_id, auction_init_bid, item_count] = numbers;
    if !item_exists(item_id) {
        warn!("ItemAuctionInstance: Failed loading auction item: Item with id {item_id} not found");
        return ItemRead::Skipped;
    }
    if builder.items.iter().any(|i| i.auction_item_id == auction_item_id) {
        warn!(
            "ItemAuctionInstance: Failed loading auction item: Dublicated auction item id {auction_item_id}"
        );
        return ItemRead::Skipped;
    }
    ItemRead::Item(AuctionItemDef {
        auction_item_id,
        auction_length,
        auction_init_bid: i64::from(auction_init_bid),
        item_id,
        item_count: i64::from(item_count),
        augmentation_id: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<!-- a comment -->
	<instance id="32320" day_of_week="5" hour_of_day="17">
		<item auctionItemId="1" itemId="9901" itemCount="1" auctionInitBid="100000000" auctionLength="300" />
		<item auctionItemId="2" itemId="9902" itemCount="2" auctionInitBid="5" auctionLength="60">
			<extra enchant_level="15" augmentation_id="1234" />
		</item>
		<item auctionItemId="3" itemId="1" itemCount="1" auctionInitBid="5" auctionLength="60" />
		<item auctionItemId="1" itemId="9903" itemCount="1" auctionInitBid="5" auctionLength="60" />
		<item auctionItemId="4" itemId="9904" itemCount="1" auctionInitBid="5" auctionLength="0" />
	</instance>
	<instance id="32321" interval="1" hour_of_day="3" minute_of_hour="30">
		<item auctionItemId="1" itemId="9905" itemCount="1" auctionInitBid="7" auctionLength="10" />
	</instance>
</list>"#;

    fn exists(id: i32) -> bool {
        (9900..9910).contains(&id)
    }

    #[test]
    fn the_instances_and_their_items_are_read_like_java() {
        let instances = instances_from_xml(XML, &exists);
        assert_eq!(instances.len(), 2);
        let first = &instances[0];
        assert_eq!(first.id, 32320);
        // Item 3 does not exist, the second `auctionItemId` 1 is a duplicate, item 4 has no length.
        assert_eq!(
            first.items.iter().map(|i| i.auction_item_id).collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(first.items[0].auction_init_bid, 100_000_000);
        assert_eq!(first.items[0].augmentation_id, 0);
        assert_eq!(first.items[1].augmentation_id, 1234);
        assert_eq!(first.items[1].item_count, 2);
        assert_eq!(first.items[1].auction_length, 60);
        assert_eq!(instances[1].id, 32321);
        assert_eq!(instances[1].items.len(), 1);
        assert_eq!(
            instances[0].generator,
            DateGenerator::new(None, Some(5), Some(17), None).unwrap()
        );
        assert_eq!(
            instances[1].generator,
            DateGenerator::new(Some(1), None, Some(3), Some(30)).unwrap()
        );
    }

    #[test]
    fn a_broken_instance_stops_the_load_but_keeps_the_ones_before() {
        let xml = r#"<list>
<instance id="1" interval="1" hour_of_day="1"><item auctionItemId="1" itemId="9901" itemCount="1" auctionInitBid="1" auctionLength="1"/></instance>
<instance id="2" hour_of_day="1"><item auctionItemId="1" itemId="9901" itemCount="1" auctionInitBid="1" auctionLength="1"/></instance>
<instance id="3" interval="1" hour_of_day="1"><item auctionItemId="1" itemId="9901" itemCount="1" auctionInitBid="1" auctionLength="1"/></instance>
</list>"#;
        // Instance 2 has neither a day nor an interval: Java throws and instance 3 is never read.
        let instances = instances_from_xml(xml, &exists);
        assert_eq!(instances.iter().map(|i| i.id).collect::<Vec<_>>(), [1]);
        // An instance without a usable item stops it too, and so does a duplicate id.
        let xml = r#"<list>
<instance id="1" interval="1" hour_of_day="1"><item auctionItemId="1" itemId="5" itemCount="1" auctionInitBid="1" auctionLength="1"/></instance>
</list>"#;
        assert!(instances_from_xml(xml, &exists).is_empty());
        let xml = r#"<list>
<instance id="1" interval="1" hour_of_day="1"><item auctionItemId="1" itemId="9901" itemCount="1" auctionInitBid="1" auctionLength="1"/></instance>
<instance id="1" interval="1" hour_of_day="1"><item auctionItemId="1" itemId="9901" itemCount="1" auctionInitBid="1" auctionLength="1"/></instance>
</list>"#;
        assert_eq!(instances_from_xml(xml, &exists).len(), 1);
    }

    #[test]
    fn the_shipped_file_parses() {
        let path = std::path::Path::new("C:/l2-rust/l2jmobius/dist/game/data/ItemAuctions.xml");
        let Ok(bytes) = std::fs::read(path) else {
            eprintln!("skipped: the reference datapack is not here");
            return;
        };
        let instances = instances_from_xml(&String::from_utf8_lossy(&bytes), &|_| true);
        assert!(!instances.is_empty());
        assert!(instances.iter().all(|i| !i.items.is_empty()));
    }
}
