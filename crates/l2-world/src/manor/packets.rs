//! The manor packets built from the lists: what Java's constructors of `ExShowManorDefaultInfo`,
//! `ExShowSeedInfo`, `ExShowCropInfo`, `ExShowSeedSetting`, `ExShowCropSetting`,
//! `ExShowProcureCropDetail`, `ExShowSellCropList`, `BuyListSeed` and `ExSendManorList` read from
//! the manor, the inventory and the castles.
// Java: network/serverpackets/{ExShowManorDefaultInfo,ExShowSeedInfo,ExShowCropInfo,
//       ExShowSeedSetting,ExShowCropSetting,ExShowProcureCropDetail,ExShowSellCropList,
//       BuyListSeed,ExSendManorList}.java
//
// The chamberlain pages (M7-NPC-01) and the `ManorManager` bypass ask these.

use l2_data::seed::java_hash_order;
use l2_protocol_game::server::{
    BuyListSeed, CropOffer, CropProcureInfo, CropSettingInfo, ExSendManorList, ExShowCropInfo,
    ExShowCropSetting, ExShowManorDefaultInfo, ExShowProcureCropDetail, ExShowSeedInfo, ExShowSeedSetting,
    ExShowSellCropList, ManorDefaultCrop, PeriodCropSales, PeriodSales, ProcureCastle, SeedForSale,
    SeedProductionInfo, SeedSettingInfo, SellCropItem,
};
use l2_util::ObjectId;

use crate::world::World;

impl World {
    /// Java `new ExSendManorList()`: the castles by id with their lower case names.
    pub fn manor_list_packet(&self) -> ExSendManorList {
        let mut castles: Vec<(i32, String)> = self
            .castles()
            .iter()
            .map(|c| (c.id.0, c.name.to_lowercase()))
            .collect();
        castles.sort_by_key(|(id, _)| *id);
        ExSendManorList { castles }
    }

    /// Java `new ExShowManorDefaultInfo(hideButtons)`.
    pub fn manor_default_info_packet(&self, hide_buttons: bool) -> ExShowManorDefaultInfo {
        let crops = self
            .manor
            .seeds
            .crops()
            .into_iter()
            .map(|crop| ManorDefaultCrop {
                crop_id: crop.crop_id,
                level: crop.level,
                seed_price: crop.seed_reference_price,
                crop_price: crop.crop_reference_price,
                reward_1: crop.reward(1),
                reward_2: crop.reward(2),
            })
            .collect();
        ExShowManorDefaultInfo { hide_buttons, crops }
    }

    /// Java `new ExShowSeedInfo(manorId, nextPeriod, hideButtons)`. The next period is not shown
    /// before it is approved; a castle the manor does not know shows no seeds.
    pub fn manor_seed_info_packet(
        &self,
        manor_id: i32,
        next_period: bool,
        hide_buttons: bool,
    ) -> ExShowSeedInfo {
        let list = if next_period && !self.manor_is_approved() {
            None
        } else {
            self.manor_seed_production(manor_id, next_period)
        };
        let seeds = list
            .unwrap_or_default()
            .iter()
            .map(|seed| {
                // `s == null`: zeros.
                let (level, reward_1, reward_2) = match self.manor.seeds.get(seed.id) {
                    Some(s) => (s.level, s.reward(1), s.reward(2)),
                    None => (0, 0, 0),
                };
                SeedProductionInfo {
                    seed_id: seed.id,
                    left: seed.amount,
                    started: seed.start_amount,
                    price: seed.price,
                    level,
                    reward_1,
                    reward_2,
                }
            })
            .collect();
        ExShowSeedInfo {
            hide_buttons,
            manor_id,
            seeds,
        }
    }

    /// Java `new ExShowCropInfo(manorId, nextPeriod, hideButtons)`.
    pub fn manor_crop_info_packet(
        &self,
        manor_id: i32,
        next_period: bool,
        hide_buttons: bool,
    ) -> ExShowCropInfo {
        let list = if next_period && !self.manor_is_approved() {
            None
        } else {
            self.manor_crop_procure(manor_id, next_period)
        };
        let crops = list
            .unwrap_or_default()
            .iter()
            .map(|crop| {
                let (level, reward_1, reward_2) = match self.manor.seeds.by_crop(crop.id) {
                    Some(s) => (s.level, s.reward(1), s.reward(2)),
                    None => (0, 0, 0),
                };
                CropProcureInfo {
                    crop_id: crop.id,
                    left: crop.amount,
                    started: crop.start_amount,
                    price: crop.price,
                    reward: crop.reward as u8,
                    level,
                    reward_1,
                    reward_2,
                }
            })
            .collect();
        ExShowCropInfo {
            hide_buttons,
            manor_id,
            crops,
        }
    }

    /// Java `new ExShowSeedSetting(manorId)`: the seeds of the castle with the sales of both
    /// periods.
    pub fn manor_seed_setting_packet(&self, manor_id: i32) -> ExShowSeedSetting {
        let rate = self.manor.settings.rate_drop_manor;
        let seeds = self
            .manor
            .seeds
            .for_castle(manor_id)
            .into_iter()
            .map(|s| {
                let sales = |next: bool| {
                    self.manor_seed_product(manor_id, s.seed_id, next).map_or(
                        PeriodSales { amount: 0, price: 0 },
                        |sp| PeriodSales {
                            amount: sp.start_amount,
                            price: sp.price,
                        },
                    )
                };
                SeedSettingInfo {
                    seed_id: s.seed_id,
                    level: s.level,
                    reward_1: s.reward(1),
                    reward_2: s.reward(2),
                    sale_limit: s.seed_limit(rate),
                    reference_price: s.seed_reference_price,
                    min_price: s.seed_min_price(),
                    max_price: s.seed_max_price(),
                    current: sales(false),
                    next: sales(true),
                }
            })
            .collect();
        ExShowSeedSetting { manor_id, seeds }
    }

    /// Java `new ExShowCropSetting(manorId)`.
    pub fn manor_crop_setting_packet(&self, manor_id: i32) -> ExShowCropSetting {
        let rate = self.manor.settings.rate_drop_manor;
        let crops = self
            .manor
            .seeds
            .for_castle(manor_id)
            .into_iter()
            .map(|s| {
                let sales = |next: bool| {
                    self.manor_crop_procure_of(manor_id, s.crop_id, next).map_or(
                        PeriodCropSales {
                            amount: 0,
                            price: 0,
                            reward: 0,
                        },
                        |cp| PeriodCropSales {
                            amount: cp.start_amount,
                            price: cp.price,
                            reward: cp.reward as u8,
                        },
                    )
                };
                CropSettingInfo {
                    crop_id: s.crop_id,
                    level: s.level,
                    reward_1: s.reward(1),
                    reward_2: s.reward(2),
                    sale_limit: s.crop_limit(rate),
                    min_price: s.crop_min_price(),
                    max_price: s.crop_max_price(),
                    current: sales(false),
                    next: sales(true),
                }
            })
            .collect();
        ExShowCropSetting { manor_id, crops }
    }

    /// Java `new ExShowProcureCropDetail(cropId)`: the castles that buy the crop in the current
    /// period and still want some, in the order of their ids.
    pub fn manor_procure_crop_detail_packet(&self, crop_id: i32) -> ExShowProcureCropDetail {
        let mut castles: Vec<ProcureCastle> = self
            .castles()
            .iter()
            .filter_map(|castle| {
                let crop = self.manor_crop_procure_of(castle.id.0, crop_id, false)?;
                (crop.amount > 0).then_some(ProcureCastle {
                    castle_id: castle.id.0,
                    amount: crop.amount,
                    price: crop.price,
                    reward: crop.reward as u8,
                })
            })
            .collect();
        // A `HashMap<Integer, ..>` of small ids: ascending.
        castles.sort_by_key(|c| c.castle_id);
        ExShowProcureCropDetail { crop_id, castles }
    }

    /// Java `new ExShowSellCropList(inventory, manorId)`: the crops in the player's inventory.
    /// `None` where Java throws (a manor the lists do not know).
    pub fn manor_sell_crop_list_packet(&self, player: ObjectId, manor_id: i32) -> Option<ExShowSellCropList> {
        let procure = self.manor_crop_procure(manor_id, false)?;
        // `for (int cropId : getCropIds()) inventory.getItemByItemId(cropId)`, into a `HashMap`.
        let mut found: Vec<(i32, ObjectId, i64)> = Vec::new();
        for crop_id in self.manor.seeds.crop_ids() {
            if let Some(item) = self.inventory_item_by_item_id(player, crop_id) {
                found.push((crop_id, item.object_id, item.count));
            }
        }
        let order = java_hash_order(&found.iter().map(|(crop_id, ..)| *crop_id).collect::<Vec<_>>());
        let mut items = Vec::new();
        for crop_id in order {
            let Some((_, object_id, count)) = found.iter().find(|(id, ..)| *id == crop_id).copied() else {
                continue;
            };
            let seed = self.manor.seeds.by_crop(crop_id)?;
            let offer = procure
                .iter()
                .find(|crop| crop.id == crop_id && crop.amount > 0)
                .map(|crop| CropOffer {
                    amount: crop.amount,
                    price: crop.price,
                    reward: crop.reward as u8,
                });
            items.push(SellCropItem {
                object_id: object_id.0,
                item_id: crop_id,
                seed_level: seed.level,
                reward_1: seed.reward(1),
                reward_2: seed.reward(2),
                offer,
                my_crops: count,
            });
        }
        Some(ExShowSellCropList { manor_id, items })
    }

    /// Java `new BuyListSeed(player.getAdena(), castleId)`: the seeds with something left and a
    /// price. `None` where Java throws (a castle the lists do not know).
    pub fn manor_buy_list_seed_packet(&self, player: ObjectId, castle_id: i32) -> Option<BuyListSeed> {
        let seeds = self
            .manor_seed_production(castle_id, false)?
            .iter()
            .filter(|s| s.amount > 0 && s.price > 0)
            .map(|s| SeedForSale {
                seed_id: s.id,
                amount: s.amount,
                price: s.price,
            })
            .collect();
        Some(BuyListSeed {
            money: self.adena_of(player),
            manor_id: castle_id,
            seeds,
        })
    }
}
