//! The production and the procurement of the castles: the lists of both periods, their costs,
//! the lord's next period, the sales' updates, `storeMe` and `resetManorData`.
// Java: managers/CastleManorManager.java

use super::{CropProcure, ManorProcureRow, ManorProductionRow, ManorWrite, SeedProduction};
use crate::outbox::PersistCommand;
use crate::residence::Writes;
use crate::world::World;

impl World {
    /// Java `getSeedProduction(castleId, nextPeriod)`: `None` is Java's `null` (a castle the
    /// manor never loaded).
    pub fn manor_seed_production(&self, castle_id: i32, next_period: bool) -> Option<&[SeedProduction]> {
        let lists = if next_period {
            &self.manor.production_next
        } else {
            &self.manor.production
        };
        lists.get(&castle_id).map(Vec::as_slice)
    }

    /// Java `getSeedProduct(castleId, seedId, nextPeriod)`.
    pub fn manor_seed_product(
        &self,
        castle_id: i32,
        seed_id: i32,
        next_period: bool,
    ) -> Option<&SeedProduction> {
        self.manor_seed_production(castle_id, next_period)?
            .iter()
            .find(|sp| sp.id == seed_id)
    }

    pub(crate) fn manor_seed_product_mut(
        &mut self,
        castle_id: i32,
        seed_id: i32,
        next_period: bool,
    ) -> Option<&mut SeedProduction> {
        let lists = if next_period {
            &mut self.manor.production_next
        } else {
            &mut self.manor.production
        };
        lists.get_mut(&castle_id)?.iter_mut().find(|sp| sp.id == seed_id)
    }

    /// Java `getCropProcure(castleId, nextPeriod)`.
    pub fn manor_crop_procure(&self, castle_id: i32, next_period: bool) -> Option<&[CropProcure]> {
        let lists = if next_period {
            &self.manor.procure_next
        } else {
            &self.manor.procure
        };
        lists.get(&castle_id).map(Vec::as_slice)
    }

    /// Java `getCropProcure(castleId, cropId, nextPeriod)`.
    pub fn manor_crop_procure_of(
        &self,
        castle_id: i32,
        crop_id: i32,
        next_period: bool,
    ) -> Option<&CropProcure> {
        self.manor_crop_procure(castle_id, next_period)?
            .iter()
            .find(|cp| cp.id == crop_id)
    }

    pub(crate) fn manor_crop_procure_mut(
        &mut self,
        castle_id: i32,
        crop_id: i32,
        next_period: bool,
    ) -> Option<&mut CropProcure> {
        let lists = if next_period {
            &mut self.manor.procure_next
        } else {
            &mut self.manor.procure
        };
        lists.get_mut(&castle_id)?.iter_mut().find(|cp| cp.id == crop_id)
    }

    /// Java `getManorCost(castleId, nextPeriod)`: what the castle pays to run the period, the
    /// reference price of every seed it sells times its start amount, plus the price of every crop
    /// it buys times its start amount. 0 for a castle the manor never loaded (Java would throw).
    pub fn manor_cost(&self, castle_id: i32, next_period: bool) -> i64 {
        let mut total_cost: i64 = 0;
        for seed in self
            .manor_seed_production(castle_id, next_period)
            .unwrap_or_default()
        {
            // `(s != null) ? (s.getSeedReferencePrice() * seed.getStartAmount()) : 1`
            total_cost = total_cost.wrapping_add(match self.manor.seeds.get(seed.id) {
                Some(s) => i64::from(s.seed_reference_price).wrapping_mul(seed.start_amount),
                None => 1,
            });
        }
        for crop in self
            .manor_crop_procure(castle_id, next_period)
            .unwrap_or_default()
        {
            total_cost = total_cost.wrapping_add(crop.price.wrapping_mul(crop.start_amount));
        }
        total_cost
    }

    /// Hands a manor row write to the persistence task as `writes` says (D-49, D-51). Returns
    /// whether it went out.
    pub(crate) fn manor_write(&mut self, writes: Writes, write: ManorWrite) -> bool {
        let go = match writes {
            Writes::Player => true,
            Writes::Job => self.siege_database_writes(),
            Writes::Never => false,
        };
        if go {
            self.outbox.persist(PersistCommand::Manor(write));
        } else {
            tracing::debug!("SideBySide: not written (SiegeDatabaseWrites = False): {write:?}");
        }
        go
    }

    /// The current and next period rows of a castle (`storeMe`'s inserts, one castle at a time).
    fn manor_production_rows(&self, castle_id: i32, next_period: bool) -> Vec<ManorProductionRow> {
        self.manor_seed_production(castle_id, next_period)
            .unwrap_or_default()
            .iter()
            .map(|sp| ManorProductionRow {
                castle_id,
                seed_id: sp.id,
                amount: sp.amount,
                start_amount: sp.start_amount,
                price: sp.price,
                next_period,
            })
            .collect()
    }

    fn manor_procure_rows(&self, castle_id: i32, next_period: bool) -> Vec<ManorProcureRow> {
        self.manor_crop_procure(castle_id, next_period)
            .unwrap_or_default()
            .iter()
            .map(|cp| ManorProcureRow {
                castle_id,
                crop_id: cp.id,
                amount: cp.amount,
                start_amount: cp.start_amount,
                price: cp.price,
                reward_type: cp.reward,
                next_period,
            })
            .collect()
    }

    /// Java `storeMe()`: both tables are written whole, the current period of every castle first,
    /// then the next. A job of the manor, written only with `SiegeDatabaseWrites` (D-51); the
    /// answer is whether it went out.
    pub fn manor_store_me(&mut self) -> bool {
        let mut production = Vec::new();
        for castle_id in self.manor.production.keys().copied().collect::<Vec<_>>() {
            production.extend(self.manor_production_rows(castle_id, false));
        }
        for castle_id in self.manor.production_next.keys().copied().collect::<Vec<_>>() {
            production.extend(self.manor_production_rows(castle_id, true));
        }
        let mut procure = Vec::new();
        for castle_id in self.manor.procure.keys().copied().collect::<Vec<_>>() {
            procure.extend(self.manor_procure_rows(castle_id, false));
        }
        for castle_id in self.manor.procure_next.keys().copied().collect::<Vec<_>>() {
            procure.extend(self.manor_procure_rows(castle_id, true));
        }
        self.manor_write(Writes::Job, ManorWrite::StoreAll { production, procure })
    }

    /// Java `setNextSeedProduction(list, castleId)`: the lord's seeds for the next period. With
    /// `AltManorSaveAllActions` the rows go out at once (a player's action).
    pub fn manor_set_next_seed_production(&mut self, list: Vec<SeedProduction>, castle_id: i32) {
        self.manor.production_next.insert(castle_id, list);
        if self.manor.settings.save_all_actions {
            let rows = self.manor_production_rows(castle_id, true);
            self.manor_write(
                Writes::Player,
                ManorWrite::ReplaceNextProduction { castle_id, rows },
            );
        }
    }

    /// Java `setNextCropProcure(list, castleId)`.
    pub fn manor_set_next_crop_procure(&mut self, list: Vec<CropProcure>, castle_id: i32) {
        self.manor.procure_next.insert(castle_id, list);
        if self.manor.settings.save_all_actions {
            let rows = self.manor_procure_rows(castle_id, true);
            self.manor_write(Writes::Player, ManorWrite::ReplaceNextProcure { castle_id, rows });
        }
    }

    /// Java `updateCurrentProduction(castleId, items)`: the amounts left of the seeds just sold,
    /// `(seed id, amount)` (with `AltManorSaveAllActions`; a player's action).
    pub(crate) fn manor_update_current_production(&mut self, castle_id: i32, amounts: Vec<(i32, i64)>) {
        self.manor_write(
            Writes::Player,
            ManorWrite::UpdateCurrentProduction { castle_id, amounts },
        );
    }

    /// Java `updateCurrentProcure(castleId, items)`: `(crop id, amount)`.
    pub(crate) fn manor_update_current_procure(&mut self, castle_id: i32, amounts: Vec<(i32, i64)>) {
        self.manor_write(
            Writes::Player,
            ManorWrite::UpdateCurrentProcure { castle_id, amounts },
        );
    }

    /// Java `resetManorData(castleId)`: a castle that lost its owner loses its manor. The rows go
    /// with the owner change (`SiegeDatabaseWrites`, D-51) and only with `AltManorSaveAllActions`
    /// (otherwise the next `storeMe` writes the empty lists).
    pub fn manor_reset_data(&mut self, castle_id: i32) {
        if !self.manor.settings.allow_manor {
            return;
        }
        for lists in [&mut self.manor.procure, &mut self.manor.procure_next] {
            if let Some(list) = lists.get_mut(&castle_id) {
                list.clear();
            }
        }
        for lists in [&mut self.manor.production, &mut self.manor.production_next] {
            if let Some(list) = lists.get_mut(&castle_id) {
                list.clear();
            }
        }
        if self.manor.settings.save_all_actions {
            self.manor_write(Writes::Job, ManorWrite::DeleteCastle { castle_id });
        }
    }
}
