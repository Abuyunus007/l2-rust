//! Server packets of the manor: the default crop table, the seed and crop production of a period,
//! the lord's settings, the procurement detail and the crop sale list.
//!
//! Same rules as [`super::party`]: values only. The manor tables, the periods (current and next)
//! and the inventory are the world's; what Java reads from them arrives as plain fields. A
//! missing row is passed as zeros, which is what Java writes for it.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

/// Java writes `0xFFFFFFFF` as the manor of a crop sale row that has no procurement.
const NO_MANOR: i32 = -1;

/// The seed reward block of the seed and crop rows: the level, then two reward item ids, each
/// behind a type byte that Java always writes as 1.
fn write_seed_rewards(w: &mut PacketWriter, level: i32, reward_1: i32, reward_2: i32) {
    w.i32(level).u8(1).i32(reward_1).u8(1).i32(reward_2);
}

/// Reads the block [`write_seed_rewards`] writes: level, reward 1, reward 2.
fn read_seed_rewards(r: &mut PacketReader) -> Result<(i32, i32, i32), PacketError> {
    let level = r.i32()?;
    r.u8()?;
    let reward_1 = r.i32()?;
    r.u8()?;
    let reward_2 = r.i32()?;
    Ok((level, reward_1, reward_2))
}

// Java: gameserver/network/serverpackets/ExShowManorDefaultInfo.java
/// One crop of the default crop table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManorDefaultCrop {
    pub crop_id: i32,
    pub level: i32,
    /// The reference price of the seed.
    pub seed_price: i32,
    /// The reference price of the crop.
    pub crop_price: i32,
    pub reward_1: i32,
    pub reward_2: i32,
}

/// The default crop table (`EX_SHOW_MANOR_DEFAULT_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExShowManorDefaultInfo {
    /// Hides the "Seed Purchase" and "Crop Sales" buttons.
    pub hide_buttons: bool,
    pub crops: Vec<ManorDefaultCrop>,
}

impl ExShowManorDefaultInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowManorDefaultInfo;
}

impl Encode for ExShowManorDefaultInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.bool(self.hide_buttons).i32(self.crops.len() as i32);
        for crop in &self.crops {
            w.i32(crop.crop_id)
                .i32(crop.level)
                .i32(crop.seed_price)
                .i32(crop.crop_price);
            w.u8(1).i32(crop.reward_1).u8(1).i32(crop.reward_2);
        }
    }
}

impl Decode for ExShowManorDefaultInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let hide_buttons = r.bool()?;
        let count = read_count(r, "manor crop count")?;
        let mut crops = Vec::new();
        for _ in 0..count {
            let crop_id = r.i32()?;
            let level = r.i32()?;
            let seed_price = r.i32()?;
            let crop_price = r.i32()?;
            r.u8()?;
            let reward_1 = r.i32()?;
            r.u8()?;
            let reward_2 = r.i32()?;
            crops.push(ManorDefaultCrop {
                crop_id,
                level,
                seed_price,
                crop_price,
                reward_1,
                reward_2,
            });
        }
        Ok(Self { hide_buttons, crops })
    }
}

// Java: gameserver/network/serverpackets/ExShowSeedInfo.java
/// One seed the castle produces in a period (the `SeedProduction` rows of the castle).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedProductionInfo {
    pub seed_id: i32,
    /// Java `getAmount()`: how many are left to buy.
    pub left: i64,
    /// Java `getStartAmount()`: how many were put up for sale.
    pub started: i64,
    /// Java `getPrice()`: the sell price.
    pub price: i64,
    pub level: i32,
    pub reward_1: i32,
    pub reward_2: i32,
}

/// The seeds of a castle for one period (`EX_SHOW_SEED_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExShowSeedInfo {
    /// Hides the "Seed Purchase" button.
    pub hide_buttons: bool,
    pub manor_id: i32,
    /// Empty when Java has no production to show (the next period before the approval).
    pub seeds: Vec<SeedProductionInfo>,
}

impl ExShowSeedInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowSeedInfo;
}

impl Encode for ExShowSeedInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.bool(self.hide_buttons).i32(self.manor_id).i32(0);
        w.i32(self.seeds.len() as i32);
        for seed in &self.seeds {
            w.i32(seed.seed_id)
                .i64(seed.left)
                .i64(seed.started)
                .i64(seed.price);
            write_seed_rewards(w, seed.level, seed.reward_1, seed.reward_2);
        }
    }
}

impl Decode for ExShowSeedInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let hide_buttons = r.bool()?;
        let manor_id = r.i32()?;
        r.i32()?;
        let count = read_count(r, "seed count")?;
        let mut seeds = Vec::new();
        for _ in 0..count {
            let seed_id = r.i32()?;
            let left = r.i64()?;
            let started = r.i64()?;
            let price = r.i64()?;
            let (level, reward_1, reward_2) = read_seed_rewards(r)?;
            seeds.push(SeedProductionInfo {
                seed_id,
                left,
                started,
                price,
                level,
                reward_1,
                reward_2,
            });
        }
        Ok(Self {
            hide_buttons,
            manor_id,
            seeds,
        })
    }
}

// Java: gameserver/network/serverpackets/ExShowCropInfo.java
/// One crop the castle buys in a period (the `CropProcure` rows of the castle).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CropProcureInfo {
    pub crop_id: i32,
    /// Java `getAmount()`: how many are left to buy.
    pub left: i64,
    /// Java `getStartAmount()`: how many were wanted.
    pub started: i64,
    /// Java `getPrice()`: the buy price.
    pub price: i64,
    /// Java `getReward()`: the reward type of the seed.
    pub reward: u8,
    pub level: i32,
    pub reward_1: i32,
    pub reward_2: i32,
}

/// The crops of a castle for one period (`EX_SHOW_CROP_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExShowCropInfo {
    /// Hides the "Crop Sales" button.
    pub hide_buttons: bool,
    pub manor_id: i32,
    /// Empty when Java has no procurement to show.
    pub crops: Vec<CropProcureInfo>,
}

impl ExShowCropInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowCropInfo;
}

impl Encode for ExShowCropInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.bool(self.hide_buttons).i32(self.manor_id).i32(0);
        w.i32(self.crops.len() as i32);
        for crop in &self.crops {
            w.i32(crop.crop_id)
                .i64(crop.left)
                .i64(crop.started)
                .i64(crop.price)
                .u8(crop.reward);
            write_seed_rewards(w, crop.level, crop.reward_1, crop.reward_2);
        }
    }
}

impl Decode for ExShowCropInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let hide_buttons = r.bool()?;
        let manor_id = r.i32()?;
        r.i32()?;
        let count = read_count(r, "crop count")?;
        let mut crops = Vec::new();
        for _ in 0..count {
            let crop_id = r.i32()?;
            let left = r.i64()?;
            let started = r.i64()?;
            let price = r.i64()?;
            let reward = r.u8()?;
            let (level, reward_1, reward_2) = read_seed_rewards(r)?;
            crops.push(CropProcureInfo {
                crop_id,
                left,
                started,
                price,
                reward,
                level,
                reward_1,
                reward_2,
            });
        }
        Ok(Self {
            hide_buttons,
            manor_id,
            crops,
        })
    }
}

/// The amount and price of a seed in one period of the seed settings (zeros when the period has
/// no row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeriodSales {
    /// The sales (Java `getStartAmount()`).
    pub amount: i64,
    pub price: i64,
}

/// One seed of the lord's seed settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedSettingInfo {
    pub seed_id: i32,
    pub level: i32,
    pub reward_1: i32,
    pub reward_2: i32,
    /// Java `getSeedLimit()`: the next sale limit.
    pub sale_limit: i32,
    /// Java `getSeedReferencePrice()`.
    pub reference_price: i32,
    /// Java `getSeedMinPrice()`.
    pub min_price: i32,
    /// Java `getSeedMaxPrice()`.
    pub max_price: i32,
    pub current: PeriodSales,
    pub next: PeriodSales,
}

/// The seed settings of a castle (`EX_SHOW_SEED_SETTING`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExShowSeedSetting {
    pub manor_id: i32,
    pub seeds: Vec<SeedSettingInfo>,
}

impl ExShowSeedSetting {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowSeedSetting;
}

impl Encode for ExShowSeedSetting {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.manor_id).i32(self.seeds.len() as i32);
        for seed in &self.seeds {
            w.i32(seed.seed_id);
            write_seed_rewards(w, seed.level, seed.reward_1, seed.reward_2);
            w.i32(seed.sale_limit)
                .i32(seed.reference_price)
                .i32(seed.min_price)
                .i32(seed.max_price);
            w.i64(seed.current.amount).i64(seed.current.price);
            w.i64(seed.next.amount).i64(seed.next.price);
        }
    }
}

impl Decode for ExShowSeedSetting {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let manor_id = r.i32()?;
        let count = read_count(r, "seed setting count")?;
        let mut seeds = Vec::new();
        for _ in 0..count {
            let seed_id = r.i32()?;
            let (level, reward_1, reward_2) = read_seed_rewards(r)?;
            let sale_limit = r.i32()?;
            let reference_price = r.i32()?;
            let min_price = r.i32()?;
            let max_price = r.i32()?;
            let current = PeriodSales {
                amount: r.i64()?,
                price: r.i64()?,
            };
            let next = PeriodSales {
                amount: r.i64()?,
                price: r.i64()?,
            };
            seeds.push(SeedSettingInfo {
                seed_id,
                level,
                reward_1,
                reward_2,
                sale_limit,
                reference_price,
                min_price,
                max_price,
                current,
                next,
            });
        }
        Ok(Self { manor_id, seeds })
    }
}

/// The amount, price and reward type of a crop in one period of the crop settings (zeros when the
/// period has no row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeriodCropSales {
    /// The amount the manor wants (Java `getStartAmount()`).
    pub amount: i64,
    pub price: i64,
    pub reward: u8,
}

/// One crop of the lord's crop settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CropSettingInfo {
    pub crop_id: i32,
    pub level: i32,
    pub reward_1: i32,
    pub reward_2: i32,
    /// Java `getCropLimit()`: the next sale limit.
    pub sale_limit: i32,
    /// Java `getCropMinPrice()`.
    pub min_price: i32,
    /// Java `getCropMaxPrice()`.
    pub max_price: i32,
    pub current: PeriodCropSales,
    pub next: PeriodCropSales,
}

/// The crop settings of a castle (`EX_SHOW_CROP_SETTING`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExShowCropSetting {
    pub manor_id: i32,
    pub crops: Vec<CropSettingInfo>,
}

impl ExShowCropSetting {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowCropSetting;
}

fn write_period_crop(w: &mut PacketWriter, period: &PeriodCropSales) {
    w.i64(period.amount).i64(period.price).u8(period.reward);
}

fn read_period_crop(r: &mut PacketReader) -> Result<PeriodCropSales, PacketError> {
    Ok(PeriodCropSales {
        amount: r.i64()?,
        price: r.i64()?,
        reward: r.u8()?,
    })
}

impl Encode for ExShowCropSetting {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.manor_id).i32(self.crops.len() as i32);
        for crop in &self.crops {
            w.i32(crop.crop_id);
            write_seed_rewards(w, crop.level, crop.reward_1, crop.reward_2);
            // Java writes a constant 0 here (`// ???`).
            w.i32(crop.sale_limit)
                .i32(0)
                .i32(crop.min_price)
                .i32(crop.max_price);
            write_period_crop(w, &crop.current);
            write_period_crop(w, &crop.next);
        }
    }
}

impl Decode for ExShowCropSetting {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let manor_id = r.i32()?;
        let count = read_count(r, "crop setting count")?;
        let mut crops = Vec::new();
        for _ in 0..count {
            let crop_id = r.i32()?;
            let (level, reward_1, reward_2) = read_seed_rewards(r)?;
            let sale_limit = r.i32()?;
            r.i32()?;
            let min_price = r.i32()?;
            let max_price = r.i32()?;
            let current = read_period_crop(r)?;
            let next = read_period_crop(r)?;
            crops.push(CropSettingInfo {
                crop_id,
                level,
                reward_1,
                reward_2,
                sale_limit,
                min_price,
                max_price,
                current,
                next,
            });
        }
        Ok(Self { manor_id, crops })
    }
}

/// One castle that buys a crop, in the procurement detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcureCastle {
    /// The castle (manor) id.
    pub castle_id: i32,
    /// Java `getAmount()`: how many the castle still wants.
    pub amount: i64,
    /// Java `getPrice()`.
    pub price: i64,
    pub reward: u8,
}

// Java: gameserver/network/serverpackets/ExShowProcureCropDetail.java
/// The castles that buy one crop (`EX_SHOW_PROCURE_CROP_DETAIL`). Java lists them in ascending
/// castle id order (a `HashMap` of small ids).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExShowProcureCropDetail {
    pub crop_id: i32,
    pub castles: Vec<ProcureCastle>,
}

impl ExShowProcureCropDetail {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowProcureCropDetail;
}

impl Encode for ExShowProcureCropDetail {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.crop_id).i32(self.castles.len() as i32);
        for castle in &self.castles {
            w.i32(castle.castle_id)
                .i64(castle.amount)
                .i64(castle.price)
                .u8(castle.reward);
        }
    }
}

impl Decode for ExShowProcureCropDetail {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let crop_id = r.i32()?;
        let count = read_count(r, "procure castle count")?;
        let mut castles = Vec::new();
        for _ in 0..count {
            castles.push(ProcureCastle {
                castle_id: r.i32()?,
                amount: r.i64()?,
                price: r.i64()?,
                reward: r.u8()?,
            });
        }
        Ok(Self { crop_id, castles })
    }
}

/// The procurement of the packet's manor for one crop, when the castle buys it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropOffer {
    pub amount: i64,
    pub price: i64,
    pub reward: u8,
}

/// One crop item of the player's inventory in the crop sale list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SellCropItem {
    /// The object id of the item in the player's inventory.
    pub object_id: i32,
    /// The crop item id.
    pub item_id: i32,
    /// The seed's level and rewards (the seed the crop comes from).
    pub seed_level: i32,
    pub reward_1: i32,
    pub reward_2: i32,
    /// `None` when the manor does not buy this crop: Java writes [`NO_MANOR`] and zeros.
    pub offer: Option<CropOffer>,
    /// Java `item.getCount()`: how many the player has.
    pub my_crops: i64,
}

// Java: gameserver/network/serverpackets/ExShowSellCropList.java
/// The crops the player can sell to the manor (`EX_SHOW_SELL_CROP_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExShowSellCropList {
    pub manor_id: i32,
    pub items: Vec<SellCropItem>,
}

impl ExShowSellCropList {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowSellCropList;
}

impl Encode for ExShowSellCropList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.manor_id).i32(self.items.len() as i32);
        for item in &self.items {
            w.i32(item.object_id).i32(item.item_id);
            write_seed_rewards(w, item.seed_level, item.reward_1, item.reward_2);
            match &item.offer {
                Some(offer) => {
                    w.i32(self.manor_id)
                        .i64(offer.amount)
                        .i64(offer.price)
                        .u8(offer.reward);
                }
                None => {
                    w.i32(NO_MANOR).i64(0).i64(0).u8(0);
                }
            }
            w.i64(item.my_crops);
        }
    }
}

impl Decode for ExShowSellCropList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let manor_id = r.i32()?;
        let count = read_count(r, "sell crop count")?;
        let mut items = Vec::new();
        for _ in 0..count {
            let object_id = r.i32()?;
            let item_id = r.i32()?;
            let (seed_level, reward_1, reward_2) = read_seed_rewards(r)?;
            let offer_manor = r.i32()?;
            let offer_amount = r.i64()?;
            let offer_price = r.i64()?;
            let offer_reward = r.u8()?;
            let offer = (offer_manor != NO_MANOR).then_some(CropOffer {
                amount: offer_amount,
                price: offer_price,
                reward: offer_reward,
            });
            let my_crops = r.i64()?;
            items.push(SellCropItem {
                object_id,
                item_id,
                seed_level,
                reward_1,
                reward_2,
                offer,
                my_crops,
            });
        }
        Ok(Self { manor_id, items })
    }
}
