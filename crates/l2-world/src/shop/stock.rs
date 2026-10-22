//! The stock of limited products and their restock: Java `Product` (count, `decreaseCount`,
//! `restock`, `save`), `BuyListData.load`'s database half and `BuyListTaskManager`.
// Java: mechanics/buylist/Product.java
// Java: data/xml/BuyListData.java (load)
// Java: taskmanagers/BuyListTaskManager.java
//
// Java creates the task manager the first time something asks for it: the first `save()` or
// `decreaseCount`, or the loading of a row with a restock time. Its two tasks then run for ever.
// Here the 60 s scan is armed at that point and re-armed every run; the 50 ms task that restocks
// one queued product per run only exists while products wait.

use l2_data::ProductDef;
use tracing::warn;

use super::{RESTOCK_SCAN_FIRST_MS, RESTOCK_SCAN_PERIOD_MS, ShopTimer};
use crate::outbox::PersistCommand;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

/// A row of the `buylists` table: the stock and the next restock time of one product.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductRow {
    pub list_id: i32,
    pub item_id: i32,
    pub count: i64,
    pub next_restock_time: i64,
}

type Key = (i32, i32);

impl World {
    /// The clock the restock times use (Java `System.currentTimeMillis()`); tests can fix it.
    pub(crate) fn shop_now_ms(&self) -> i64 {
        self.settings
            .shop
            .clock_override_unix_ms
            .unwrap_or_else(unix_ms_now)
    }

    fn product_def(&self, key: Key) -> Option<ProductDef> {
        self.shop.lists.as_ref()?.get(key.0)?.product(key.1).cloned()
    }

    /// Java `Product.getCount()`: what is left of a limited product, never negative; 0 for a
    /// product without a limit.
    pub fn product_count(&self, list_id: i32, item_id: i32) -> i64 {
        let key = (list_id, item_id);
        let Some(def) = self.product_def(key) else {
            return 0;
        };
        if !def.has_limited_stock() {
            return 0;
        }
        self.shop
            .counts
            .get(&key)
            .copied()
            .unwrap_or(def.max_count)
            .max(0)
    }

    /// Java `Product.decreaseCount(value)`: `false` for an unlimited product, or when more were
    /// taken than there were. The restock time starts with the first purchase and is not moved
    /// by the next ones (`putIfAbsent`).
    pub(crate) fn decrease_product_count(&mut self, list_id: i32, item_id: i32, value: i64) -> bool {
        let key = (list_id, item_id);
        let Some(def) = self.product_def(key) else {
            return false;
        };
        if !def.has_limited_stock() {
            return false;
        }
        self.start_restock_manager();
        let end = self.shop_now_ms().wrapping_add(def.restock_delay_ms);
        self.shop.restock_at.entry(key).or_insert(end);
        let count = self.shop.counts.entry(key).or_insert(def.max_count);
        *count = count.wrapping_sub(value);
        let result = *count >= 0;
        self.save_product(key);
        result
    }

    /// Java `Product.restock()`.
    fn restock_product(&mut self, key: Key) {
        let Some(def) = self.product_def(key) else {
            return;
        };
        self.shop.counts.insert(key, def.max_count);
        self.save_product(key);
    }

    /// Java `Product.save()`: the `buylists` row, with the restock time the task manager holds
    /// (0 when it holds none).
    fn save_product(&mut self, key: Key) {
        // `BuyListTaskManager.getInstance()` creates the manager.
        self.start_restock_manager();
        let next_restock_time = self
            .shop
            .restock_at
            .get(&key)
            .copied()
            .filter(|time| *time > 0)
            .unwrap_or(0);
        let row = ProductRow {
            list_id: key.0,
            item_id: key.1,
            count: self.product_count(key.0, key.1),
            next_restock_time,
        };
        self.outbox.persist(PersistCommand::BuyList(row));
    }

    /// Java `BuyListTaskManager`'s constructor: both tasks start. Only the scan runs here.
    fn start_restock_manager(&mut self) {
        if self.shop.started {
            return;
        }
        self.shop.started = true;
        let due = self.tick + TimerWheel::ms_to_ticks(RESTOCK_SCAN_FIRST_MS);
        self.timers
            .schedule_at(due, TimerKind::Shop(ShopTimer::RestockScan));
    }

    /// Java `BuyListData.load`, the part that reads the `buylists` table: the rows the Java server
    /// (or this one) left. A row of a list or product that no longer exists is skipped with a
    /// warning.
    pub fn load_buy_list_stock(&mut self, rows: Vec<ProductRow>) {
        for row in rows {
            let key = (row.list_id, row.item_id);
            let Some(list) = self.shop.lists.as_ref().and_then(|l| l.get(row.list_id)) else {
                warn!(
                    "BuyList found in database but not loaded from xml! BuyListId: {}",
                    row.list_id
                );
                continue;
            };
            let Some(max_count) = list.product(row.item_id).map(|p| p.max_count) else {
                warn!(
                    "ItemId found in database but not loaded from xml! BuyListId: {} ItemId: {}",
                    row.list_id, row.item_id
                );
                continue;
            };
            if row.count < max_count {
                self.shop.counts.insert(key, row.count);
                self.restart_restock_task(key, row.next_restock_time);
            }
        }
    }

    /// Java `Product.restartRestockTask(nextRestockTime)`.
    // NOTE(java-quirk): `BuyListTaskManager.update(this, remainTime)` stores the time that is *left*
    // where the manager expects the time it is *due*, so the product looks long overdue and the
    // next scan restocks it (and its row gets a `next_restock_time` of 0 then). A product that
    // was still waiting when the server stopped is therefore restocked within a minute of the
    // start. The same value is what the next `save()` writes as the restock time.
    fn restart_restock_task(&mut self, key: Key, next_restock_time: i64) {
        let remain = next_restock_time.wrapping_sub(self.shop_now_ms());
        if remain > 0 {
            self.start_restock_manager();
            self.shop.restock_at.insert(key, remain);
        } else {
            self.restock_product(key);
        }
    }

    /// Java `BuyListProductTask.run`: the products whose time passed are queued.
    pub(super) fn on_restock_scan(&mut self) {
        let now = self.shop_now_ms();
        let due: Vec<Key> = self
            .shop
            .restock_at
            .iter()
            .filter(|(_, end)| now > **end)
            .map(|(key, _)| *key)
            .collect();
        for key in due {
            self.shop.restock_at.remove(&key);
            if !self.shop.pending.contains(&key) {
                self.shop.pending.push_back(key);
            }
        }
        let next = self.tick + TimerWheel::ms_to_ticks(RESTOCK_SCAN_PERIOD_MS);
        self.timers
            .schedule_at(next, TimerKind::Shop(ShopTimer::RestockScan));
        self.arm_restock_save();
    }

    /// The 50 ms task exists while products wait.
    fn arm_restock_save(&mut self) {
        if self.shop.save_timer.is_some() || self.shop.pending.is_empty() {
            return;
        }
        let due = self.tick + 1;
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Shop(ShopTimer::RestockSave));
        self.shop.save_timer = Some(timer);
    }

    /// Java `BuyListSaveTask.run`: the first queued product is restocked.
    pub(super) fn on_restock_save(&mut self) {
        self.shop.save_timer = None;
        if let Some(key) = self.shop.pending.pop_front() {
            self.restock_product(key);
        }
        self.arm_restock_save();
    }
}
