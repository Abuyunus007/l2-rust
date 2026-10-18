//! Admin GUI (D-31): talks to the supervisor over the admin API.
//!
//! Structure: `app` = window and layout, `bridge` = async client on a background thread,
//! `settings` = persisted configuration, `views/` = one file per screen (see `views/mod.rs`).

mod app;
mod bridge;
mod settings;
mod views;

use anyhow::anyhow;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("L2 Admin")
            .with_inner_size([960.0, 600.0])
            .with_min_inner_size([640.0, 360.0]),
        ..Default::default()
    };
    eframe::run_native(
        "l2-admin-gui",
        options,
        Box::new(|cc| Ok(Box::new(app::AdminApp::new(cc)?))),
    )
    .map_err(|e| anyhow!("{e}"))
}
