//! A clan warehouse window waits for fresh rows (M6-SYNC-01, D-39).
// Java: entity/itemcontainer/ClanWarehouse.java, ItemContainer.java (restore),
// Java: dist/game/data/scripts/handlers/bypass/npc/ClanWarehouse.java
//
// The window of `WithdrawC`, `WithdrawSortedC` and `DepositC` is not built from the container in
// memory: the persistence task reads the `CLANWH` rows of the clan first
// (`PersistCommand::ClanWarehouseRequest`, D-15: the world never waits for the database) and the
// answer (`WorldCommand::ClanWarehouseLoaded`) replaces `Clan::warehouse` and then continues with
// the window. A failed read opens nothing and never shows the old container.

use l2_protocol_game::client::WarehouseListEntry;
use l2_protocol_game::server::ActionFailed;
use l2_util::ObjectId;
use tracing::warn;

use super::sorted::WarehouseListType;
use crate::outbox::PersistCommand;
use crate::player::ItemData;
use crate::world::World;

/// Which clan warehouse window is waiting for the rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PendingClanWindow {
    /// `WithdrawC` (`None`) or `WithdrawSortedC` (the list type and order).
    Withdraw(Option<(WarehouseListType, i8)>),
    /// `DepositC`.
    Deposit,
    /// `SendWareHouseWithDrawList` on the clan warehouse: the packet waits for the rows and then
    /// runs on the refreshed container.
    WithdrawPacket(Vec<WarehouseListEntry>),
}

impl World {
    /// Asks for the `CLANWH` rows of `clan_id` and remembers the window. A second request of the
    /// same player replaces the first; the answer of the first one finds nothing waiting and is
    /// dropped.
    pub(crate) fn request_clan_window(&mut self, player: ObjectId, clan_id: i32, window: PendingClanWindow) {
        let Some(p) = self.players.get_mut(&player) else {
            return;
        };
        p.warehouse.pending_clan = Some((clan_id, window));
        self.outbox
            .persist(PersistCommand::ClanWarehouseRequest { player, clan_id });
    }

    /// `WorldCommand::ClanWarehouseLoaded`: the container of the clan is replaced by the rows and
    /// the window goes on as if the rows had been in memory all along.
    pub(crate) fn on_clan_warehouse_loaded(
        &mut self,
        player: ObjectId,
        clan_id: i32,
        result: Result<Vec<ItemData>, ()>,
    ) {
        let Some(p) = self.players.get_mut(&player) else {
            return;
        };
        if !matches!(p.warehouse.pending_clan, Some((pending, _)) if pending == clan_id) {
            return;
        }
        let Some((_, window)) = p.warehouse.pending_clan.take() else {
            return;
        };
        let Ok(mut items) = result else {
            warn!(
                "ClanWarehouse: could not read the items of clan {clan_id} for {player}; the window is not opened."
            );
            self.send(player, &ActionFailed);
            return;
        };
        // The player may have left the clan while the database answered.
        if self.clan_id_of(player) != clan_id {
            return;
        }
        // `Item.restoreFromDb` returns null for a template the datapack does not know.
        self.drop_unknown_items(&mut items);
        let Some(clan) = self.clan_mut(clan_id) else {
            return;
        };
        clan.warehouse = items;
        match window {
            PendingClanWindow::Withdraw(sorted) => self.show_clan_withdraw_window(player, sorted),
            PendingClanWindow::Deposit => self.show_clan_deposit_window(player, clan_id),
            PendingClanWindow::WithdrawPacket(items) => self.warehouse_withdraw(player, items, true),
        }
    }
}
