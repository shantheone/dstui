use crate::app::App;
use crate::config::{AppConfig, run_config_wizard};
use std::error::Error;
use syno_download_station::client::SynoDS;

pub mod api;
pub mod app;
pub mod config;
pub mod event;
pub mod ui;
pub mod util;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("+----------------------------------+");
    println!("|   Synology DownloadStation TUI   |");
    println!("+----------------------------------+");
    let config = if let Some(cfg) = AppConfig::load() {
        cfg
    } else {
        // No config file found -> run the wizard
        let cfg = run_config_wizard()?;
        cfg.save()?; // Save config to file
        cfg
    };

    // Build client call from the config
    let endpoint = format!("{}:{}", config.server_url, config.server_port);
    let mut client = SynoDS::new(endpoint, config.username, config.password, 100)?;

    // Logging in
    println!("󰍂  Logging in and downloading task list. Just a sec...");
    client.authorize().await?;

    // Launching the main app
    {
        let mut app_term = ratatui::init();
        let app = App::new();

        app.run(&mut app_term, &mut client).await?;
    }
    // Restore terminal
    ratatui::restore();

    // Call logout method to close the connection
    // TODO: implement logout method in syno-download-station crate
    // client.logout("DownloadStation").await?;

    Ok(())
}
