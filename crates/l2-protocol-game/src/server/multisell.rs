//! Server packet of the multisell window (the NPC exchange lists).
//!
//! Same rules as [`super::trade`]: values only. The world turns a prepared list into these
//! entries; nothing here knows the item tables.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

/// Java `MultisellData.PAGE_SIZE`: entries per packet.
pub const PAGE_SIZE: usize = 40;

/// What the client shows for an item without a template (Java writes 65535 as the `type2`).
pub const TYPE2_UNKNOWN: u16 = 65535;

/// The enchantment, augmentation and element values that follow every item of an entry.
///
/// Java writes them from the ingredient's `ItemInfo` when it has one and from its plain enchant
/// level otherwise (everything else 0); the world decides which, so these are the final values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MultiSellItemDetail {
    /// Enchant level (Java `writeShort(int)`).
    pub enchant: i16,
    /// Augmentation id.
    pub augment_id: i32,
    /// Attack element type (a Java `byte` written as a short: -1 and -2 are 0xFFFF and 0xFFFE).
    pub attack_element: i16,
    pub attack_element_power: i16,
    /// Fire, water, wind, earth, holy, dark defense.
    pub element_defense: [i16; 6],
}

impl MultiSellItemDetail {
    fn write(&self, w: &mut PacketWriter) {
        w.i16(self.enchant)
            .i32(self.augment_id)
            .i32(0) // mana
            .i16(self.attack_element)
            .i16(self.attack_element_power);
        for defense in self.element_defense {
            w.i16(defense);
        }
    }

    fn read(r: &mut PacketReader) -> Result<Self, PacketError> {
        let enchant = r.i16()?;
        let augment_id = r.i32()?;
        r.skip(4)?; // mana
        let attack_element = r.i16()?;
        let attack_element_power = r.i16()?;
        let mut element_defense = [0; 6];
        for defense in &mut element_defense {
            *defense = r.i16()?;
        }
        Ok(Self {
            enchant,
            augment_id,
            attack_element,
            attack_element_power,
            element_defense,
        })
    }
}

/// A product of an entry: what the player gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiSellProduct {
    /// The template's display id, or the raw id of an item without a template (the special
    /// products -100, -200, -300 are sent as they are).
    pub item_id: i32,
    /// `getBodyPart().getMask()`; 0 without a template.
    pub body_part: i32,
    /// `getType2()`; [`TYPE2_UNKNOWN`] without a template.
    pub type2: u16,
    pub count: i64,
    pub detail: MultiSellItemDetail,
}

/// An ingredient of an entry: what the player pays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiSellIngredient {
    /// The template's display id, or the raw id without a template.
    pub item_id: i32,
    /// `getType2()`; [`TYPE2_UNKNOWN`] without a template.
    pub type2: u16,
    pub count: i64,
    pub detail: MultiSellItemDetail,
}

/// One line of the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiSellEntry {
    pub entry_id: i32,
    pub stackable: bool,
    pub products: Vec<MultiSellProduct>,
    pub ingredients: Vec<MultiSellIngredient>,
}

// Java: gameserver/network/serverpackets/MultiSellList.java
/// One page of a multisell list (`MULTI_SELL_LIST`, 0xD0). Java sends the pages of a list one
/// after the other; an empty list is one page without entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiSellList {
    pub list_id: i32,
    /// `1 + index / PAGE_SIZE`.
    pub page: i32,
    /// No page follows.
    pub finished: bool,
    pub entries: Vec<MultiSellEntry>,
}

impl MultiSellList {
    pub const OPCODE: ServerOpcode = ServerOpcode::MultiSellList;

    /// The constructor of Java's `MultiSellList(list, index)`: how many of `total` entries the page
    /// starting at `index` holds, and whether it is the last page.
    pub fn page_len(total: usize, index: usize) -> (usize, bool) {
        let size = total.saturating_sub(index);
        if size > PAGE_SIZE {
            (PAGE_SIZE, false)
        } else {
            (size, true)
        }
    }

    /// The page number of the page that starts at entry `index` (starts at 1).
    pub fn page_number(index: usize) -> i32 {
        i32::try_from(1 + index / PAGE_SIZE).unwrap_or(i32::MAX)
    }
}

impl Encode for MultiSellList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.list_id)
            .i32(self.page)
            .i32(i32::from(self.finished))
            .i32(PAGE_SIZE as i32)
            .i32(self.entries.len() as i32);
        for entry in &self.entries {
            w.i32(entry.entry_id)
                .bool(entry.stackable)
                .i16(0) // C6
                .i32(0) // C6
                .i32(0) // T1
                .u16(65534) // T1
                .bytes(&[0; 14]) // T1: seven shorts
                .i16(entry.products.len() as i16)
                .i16(entry.ingredients.len() as i16);
            for product in &entry.products {
                w.i32(product.item_id)
                    .i32(product.body_part)
                    .u16(product.type2)
                    .i64(product.count);
                product.detail.write(w);
            }
            for ingredient in &entry.ingredients {
                w.i32(ingredient.item_id)
                    .u16(ingredient.type2)
                    .i64(ingredient.count);
                ingredient.detail.write(w);
            }
        }
    }
}

impl Decode for MultiSellList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let list_id = r.i32()?;
        let page = r.i32()?;
        let finished = r.i32()? != 0;
        r.skip(4)?; // the page size, always PAGE_SIZE
        let count = read_count(r, "entries")?;
        let mut entries = Vec::new();
        for _ in 0..count {
            let entry_id = r.i32()?;
            let stackable = r.bool()?;
            r.skip(2 + 4 + 4 + 2 + 14)?;
            let products = usize::from(r.u16()?);
            let ingredients = usize::from(r.u16()?);
            let mut product_list = Vec::new();
            for _ in 0..products {
                product_list.push(MultiSellProduct {
                    item_id: r.i32()?,
                    body_part: r.i32()?,
                    type2: r.u16()?,
                    count: r.i64()?,
                    detail: MultiSellItemDetail::read(r)?,
                });
            }
            let mut ingredient_list = Vec::new();
            for _ in 0..ingredients {
                ingredient_list.push(MultiSellIngredient {
                    item_id: r.i32()?,
                    type2: r.u16()?,
                    count: r.i64()?,
                    detail: MultiSellItemDetail::read(r)?,
                });
            }
            entries.push(MultiSellEntry {
                entry_id,
                stackable,
                products: product_list,
                ingredients: ingredient_list,
            });
        }
        Ok(Self {
            list_id,
            page,
            finished,
            entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail(enchant: i16) -> MultiSellItemDetail {
        MultiSellItemDetail {
            enchant,
            augment_id: 0x0102_0304,
            attack_element: -1,
            attack_element_power: 20,
            element_defense: [1, 2, 3, 4, 5, 6],
        }
    }

    fn entry(id: i32) -> MultiSellEntry {
        MultiSellEntry {
            entry_id: id,
            stackable: false,
            products: vec![MultiSellProduct {
                item_id: 22,
                body_part: 0x0001,
                type2: 1,
                count: 1,
                detail: detail(3),
            }],
            ingredients: vec![
                MultiSellIngredient {
                    item_id: 57,
                    type2: 4,
                    count: 2100,
                    detail: MultiSellItemDetail::default(),
                },
                MultiSellIngredient {
                    item_id: -300,
                    type2: TYPE2_UNKNOWN,
                    count: 5,
                    detail: MultiSellItemDetail::default(),
                },
            ],
        }
    }

    #[test]
    fn a_page_round_trips() {
        let packet = MultiSellList {
            list_id: 1235,
            page: 2,
            finished: true,
            entries: vec![entry(100_000), entry(200_003)],
        };
        let bytes = packet.to_bytes();
        let mut reader = PacketReader::new(&bytes[1..]);
        assert_eq!(MultiSellList::decode(&mut reader).unwrap(), packet);
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn an_empty_list_is_one_page_without_entries() {
        let packet = MultiSellList {
            list_id: 9,
            page: 1,
            finished: true,
            entries: Vec::new(),
        };
        // opcode, then five ints.
        let mut expected = vec![0xD0];
        for value in [9, 1, 1, 40, 0] {
            expected.extend_from_slice(&i32::to_le_bytes(value));
        }
        assert_eq!(packet.to_bytes(), expected);
    }

    #[test]
    fn the_layout_is_javas() {
        let packet = MultiSellList {
            list_id: 7,
            page: 1,
            finished: false,
            entries: vec![entry(5)],
        };
        let bytes = packet.to_bytes();
        let le32 = |v: i32| v.to_le_bytes().to_vec();
        let mut expected = vec![0xD0];
        // list id, page, finished (an int), page size, entry count.
        for v in [7, 1, 0, 40, 1] {
            expected.extend(le32(v));
        }
        expected.extend(le32(5)); // entry id
        expected.push(0); // stackable
        expected.extend([0; 2]); // C6 short
        expected.extend([0; 4]); // C6 int
        expected.extend([0; 4]); // T1 int
        expected.extend(65534u16.to_le_bytes()); // T1 short
        expected.extend([0; 14]); // seven T1 shorts
        expected.extend(1i16.to_le_bytes()); // products
        expected.extend(2i16.to_le_bytes()); // ingredients
        // the product: display id, body part, type2, count, then the detail.
        expected.extend(le32(22));
        expected.extend(le32(1));
        expected.extend(1u16.to_le_bytes());
        expected.extend(1i64.to_le_bytes());
        expected.extend(3i16.to_le_bytes()); // enchant
        expected.extend(le32(0x0102_0304)); // augmentation
        expected.extend(le32(0)); // mana
        expected.extend((-1i16).to_le_bytes()); // attack element
        expected.extend(20i16.to_le_bytes());
        for v in 1..=6i16 {
            expected.extend(v.to_le_bytes());
        }
        // ingredient 1: adena, no augmentation, nothing set.
        expected.extend(le32(57));
        expected.extend(4u16.to_le_bytes());
        expected.extend(2100i64.to_le_bytes());
        expected.extend([0; 2 + 4 + 4 + 2 + 2 + 12]);
        // ingredient 2: the fame id has no template.
        expected.extend(le32(-300));
        expected.extend(65535u16.to_le_bytes());
        expected.extend(5i64.to_le_bytes());
        expected.extend([0; 2 + 4 + 4 + 2 + 2 + 12]);
        assert_eq!(bytes, expected);
    }

    #[test]
    fn paging_follows_javas_constructor() {
        // 100 entries: pages of 40, 40, 20.
        assert_eq!(MultiSellList::page_len(100, 0), (40, false));
        assert_eq!(MultiSellList::page_len(100, 40), (40, false));
        assert_eq!(MultiSellList::page_len(100, 80), (20, true));
        // Exactly one full page is finished.
        assert_eq!(MultiSellList::page_len(40, 0), (40, true));
        assert_eq!(MultiSellList::page_len(41, 0), (40, false));
        // An empty list still has its page.
        assert_eq!(MultiSellList::page_len(0, 0), (0, true));
        assert_eq!(MultiSellList::page_number(0), 1);
        assert_eq!(MultiSellList::page_number(40), 2);
        assert_eq!(MultiSellList::page_number(80), 3);
    }

    #[test]
    fn a_short_packet_is_an_error() {
        let bytes = MultiSellList {
            list_id: 1,
            page: 1,
            finished: true,
            entries: vec![entry(1)],
        }
        .to_bytes();
        for cut in [1, 10, 25, bytes.len() - 2] {
            assert!(MultiSellList::decode(&mut PacketReader::new(&bytes[1..cut])).is_err());
        }
    }
}
