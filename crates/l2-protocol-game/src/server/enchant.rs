//! Server packets of item enchanting and element attributes.
//!
//! Values only, like the other server packets: the world works out the numbers.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/ChooseInventoryItem.java
server_packet!(
    /// Opens the scroll window of the enchant scroll `item_id` (`CHOOSE_INVENTORY_ITEM`). It is the
    /// template id (`item.getId()`), not the display id.
    ChooseInventoryItem,
    ChooseInventoryItem {
        item_id: i32,
    }
);

// Java: gameserver/network/serverpackets/EnchantResult.java
server_packet!(
    /// The outcome of an enchant (`ENCHANT_RESULT`). `result`: 0 success, 1 failed with crystals,
    /// 2 error, 3 blessed failure (level 0), 4 failed without crystals, 5 safe failure.
    EnchantResult,
    EnchantResult {
        result: i32,
        /// The crystal item id of result 1.
        crystal: i32,
        /// Java declares the field `int` and writes it with `writeLong`.
        count: i64,
    }
);

// Java: gameserver/network/serverpackets/ExPutEnchantTargetItemResult.java
server_packet!(
    /// The answer to `RequestExTryToPutEnchantTargetItem`: the object id of the accepted item, 0
    /// when it does not fit (`EX_PUT_ENCHANT_TARGET_ITEM_RESULT`).
    ExPutEnchantTargetItemResult,
    ExPutEnchantTargetItemResult {
        result: i32,
    }
);

// Java: gameserver/network/serverpackets/ExPutEnchantSupportItemResult.java
server_packet!(
    /// The answer to `RequestExTryToPutEnchantSupportItem` (`EX_PUT_ENCHANT_SUPPORT_ITEM_RESULT`).
    ExPutEnchantSupportItemResult,
    ExPutEnchantSupportItemResult {
        result: i32,
    }
);

// Java: gameserver/network/serverpackets/ExAttributeEnchantResult.java
server_packet!(
    /// The power an element stone added (`EX_ATTRIBUTE_ENCHANT_RESULT`); 0 when it failed.
    ExAttributeEnchantResult,
    ExAttributeEnchantResult {
        result: i32,
    }
);

// Java: gameserver/network/serverpackets/ExBaseAttributeCancelResult.java
/// An element was taken off an item (`EX_BASE_ATTRIBUTE_CANCEL_RESULT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExBaseAttributeCancelResult {
    pub object_id: i32,
    /// The element removed (Java `byte`, written as an int).
    pub attribute: i32,
}

impl ExBaseAttributeCancelResult {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExBaseAttributeCancelResult;
}

impl Encode for ExBaseAttributeCancelResult {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // The result: Java writes the constant 1.
        w.i32(1).i32(self.object_id).i32(self.attribute);
    }
}

impl Decode for ExBaseAttributeCancelResult {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let _result = r.i32()?;
        Ok(Self {
            object_id: r.i32()?,
            attribute: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExChooseInventoryAttributeItem.java
/// Opens the window to choose the item an element stone goes on
/// (`EX_CHOOSE_INVENTORY_ATTRIBUTE_ITEM`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExChooseInventoryAttributeItem {
    /// The stone's display id.
    pub item_id: i32,
    /// Which element the stone is: fire, water, wind, earth, holy, dark. Java writes each as an int
    /// that is 1 for the stone's element.
    pub elements: [bool; 6],
    /// The highest attribute level the stone can give (`ElementalItemType.getMaxLevel`).
    pub max_level: i32,
}

impl ExChooseInventoryAttributeItem {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExChooseInventoryAttributeItem;

    /// Java's constructor, after it looked the stone up: `element` is `Elementals.getItemElement`
    /// (0 fire to 5 dark).
    pub fn new(item_id: i32, element: i32, max_level: i32) -> Self {
        let mut elements = [false; 6];
        if let Some(slot) = usize::try_from(element).ok().and_then(|e| elements.get_mut(e)) {
            *slot = true;
        }
        Self {
            item_id,
            elements,
            max_level,
        }
    }
}

impl Encode for ExChooseInventoryAttributeItem {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.item_id);
        for flag in self.elements {
            w.i32(i32::from(flag));
        }
        w.i32(self.max_level);
    }
}

impl Decode for ExChooseInventoryAttributeItem {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let item_id = r.i32()?;
        let mut elements = [false; 6];
        for flag in &mut elements {
            *flag = r.i32()? != 0;
        }
        Ok(Self {
            item_id,
            elements,
            max_level: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExShowBaseAttributeCancelWindow.java
/// The window to take an element off an item: every item with an element and what it costs
/// (`EX_SHOW_BASE_ATTRIBUTE_CANCEL_WINDOW`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExShowBaseAttributeCancelWindow {
    /// `(object id, price in adena)`.
    pub items: Vec<(i32, i64)>,
}

impl ExShowBaseAttributeCancelWindow {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowBaseAttributeCancelWindow;
}

impl Encode for ExShowBaseAttributeCancelWindow {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(i32::try_from(self.items.len()).unwrap_or(i32::MAX));
        for &(object_id, price) in &self.items {
            w.i32(object_id).i64(price);
        }
    }
}

impl Decode for ExShowBaseAttributeCancelWindow {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = usize::try_from(r.i32()?).unwrap_or(0);
        let mut items = Vec::new();
        for _ in 0..count {
            items.push((r.i32()?, r.i64()?));
        }
        Ok(Self { items })
    }
}
