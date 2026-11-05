//! The views behind the navigation, and the [`View`] trait they implement.
//!
//! # Adding a view (the pattern every GUI card copies)
//! 1. New file `views/<name>.rs` with a struct that implements [`View`].
//! 2. Add a field for it to [`Views`] and a match arm in [`Views::get_mut`].
//! 3. Flip [`ViewId::is_available`] to `true` for its [`ViewId`] (the entry exists already as a
//!    greyed-out placeholder).
//!
//! A view keeps its own UI state in its struct. Everything shared (settings, connection state,
//! the bridge to the supervisor) comes in through [`AppCtx`]. `ui` runs every frame and must not
//! block: send requests with `cx.bridge.request(..)` and handle the answers in
//! [`View::on_event`], which receives every [`BridgeEvent`].

pub mod accounts;
pub mod backups;
pub mod config_editor;
pub mod connection;
pub mod data_browser;
pub mod game_admin;
pub mod logs;
pub mod packets;
pub mod players;
pub mod servers;
pub mod setup;
pub mod setup_core;

use crate::app::AppCtx;
use crate::bridge::BridgeEvent;

pub trait View {
    /// Draws the view into the central area. Called every frame while the view is selected.
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut AppCtx<'_>);

    /// Called for every bridge event, whether or not the view is selected, so a view can keep
    /// collecting data (e.g. log lines) while hidden. Default: ignore.
    fn on_event(&mut self, _event: &BridgeEvent) {}
}

/// Entries of the left navigation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewId {
    Connection,
    Setup,
    Servers,
    Players,
    GameAdmin,
    Accounts,
    Backups,
    Logs,
    Packets,
    Data,
    Config,
}

impl ViewId {
    pub const ALL: [ViewId; 11] = [
        ViewId::Connection,
        ViewId::Setup,
        ViewId::Servers,
        ViewId::Players,
        ViewId::GameAdmin,
        ViewId::Accounts,
        ViewId::Backups,
        ViewId::Logs,
        ViewId::Packets,
        ViewId::Data,
        ViewId::Config,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ViewId::Connection => "Connection",
            ViewId::Setup => "Setup",
            ViewId::Servers => "Servers",
            ViewId::Players => "Players",
            ViewId::GameAdmin => "Game admin",
            ViewId::Accounts => "Accounts",
            ViewId::Backups => "Backups",
            ViewId::Logs => "Logs",
            ViewId::Packets => "Packets",
            ViewId::Data => "Data",
            ViewId::Config => "Config",
        }
    }

    /// Placeholders (`false`) are shown greyed out and cannot be selected.
    pub fn is_available(self) -> bool {
        matches!(
            self,
            ViewId::Connection
                | ViewId::Setup
                | ViewId::Servers
                | ViewId::Players
                | ViewId::GameAdmin
                | ViewId::Accounts
                | ViewId::Backups
                | ViewId::Logs
                | ViewId::Packets
                | ViewId::Data
                | ViewId::Config
        )
    }

    /// Which card adds the view; shown in the placeholder's tooltip. Empty for available views.
    /// A new placeholder is a new `ViewId` whose arm names its card here.
    pub fn planned_in(self) -> &'static str {
        match self {
            ViewId::Connection
            | ViewId::Setup
            | ViewId::Servers
            | ViewId::Players
            | ViewId::GameAdmin
            | ViewId::Accounts
            | ViewId::Backups
            | ViewId::Logs
            | ViewId::Packets
            | ViewId::Data
            | ViewId::Config => "",
        }
    }
}

/// All view instances, owned by the app.
#[derive(Default)]
pub struct Views {
    pub connection: connection::ConnectionView,
    pub setup: setup::SetupView,
    pub servers: servers::ServersView,
    pub players: players::PlayersView,
    pub game_admin: game_admin::GameAdminView,
    pub accounts: accounts::AccountsView,
    pub backups: backups::BackupsView,
    pub logs: logs::LogsView,
    pub packets: packets::PacketsView,
    pub data: data_browser::DataBrowserView,
    pub config: config_editor::ConfigEditorView,
}

impl Views {
    /// The view for `id`, or `None` for a placeholder.
    pub fn get_mut(&mut self, id: ViewId) -> Option<&mut dyn View> {
        match id {
            ViewId::Connection => Some(&mut self.connection),
            ViewId::Setup => Some(&mut self.setup),
            ViewId::Servers => Some(&mut self.servers),
            ViewId::Players => Some(&mut self.players),
            ViewId::GameAdmin => Some(&mut self.game_admin),
            ViewId::Accounts => Some(&mut self.accounts),
            ViewId::Backups => Some(&mut self.backups),
            ViewId::Logs => Some(&mut self.logs),
            ViewId::Packets => Some(&mut self.packets),
            ViewId::Data => Some(&mut self.data),
            ViewId::Config => Some(&mut self.config),
        }
    }

    pub fn dispatch(&mut self, event: &BridgeEvent) {
        self.connection.on_event(event);
        self.setup.on_event(event);
        self.servers.on_event(event);
        self.players.on_event(event);
        self.game_admin.on_event(event);
        self.accounts.on_event(event);
        self.backups.on_event(event);
        self.logs.on_event(event);
        self.packets.on_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_available_views_have_an_instance() {
        let mut views = Views::default();
        for id in ViewId::ALL {
            assert_eq!(views.get_mut(id).is_some(), id.is_available(), "{id:?}");
        }
    }

    #[test]
    fn placeholders_name_their_card() {
        for id in ViewId::ALL {
            assert_eq!(id.planned_in().is_empty(), id.is_available(), "{id:?}");
        }
    }
}
