use crate::api::SynologyClient;
use crate::app::App;
use crate::config::{AppConfig, run_config_wizard};
use std::error::Error;

pub mod api;
pub mod app;
pub mod config;
pub mod event;
pub mod ui;
pub mod util;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = if let Some(cfg) = AppConfig::load() {
        cfg
    } else {
        // No config file found -> run the wizard
        let mut wiz_term = ratatui::init();
        let cfg = run_config_wizard(&mut wiz_term)?;
        cfg.save()?; // Save config to file
        cfg
    };

    // Build client call from the config
    let endpoint = format!("{}:{}", config.server_url, config.port);
    let mut client = SynologyClient::new(&endpoint);

    // Logging in
    println!("󰍂  Logging in and downloading task list. Just a sec...");
    client.get_available_apis().await?;
    client
        .login(&config.username, &config.password, "DownloadStation")
        .await
        .map_err(|e| format!("Login failed: {e}"))?;

    // Launching the main app
    {
        let mut app_term = ratatui::init();
        let app = App::new();

        // Get refresh interval from the config file
        // let interval = config.refresh_interval;

        app.run(&mut app_term, &mut client).await?;
    }
    // Restore terminal
    ratatui::restore();

    // Call logout method to close the connection
    client.logout("DownloadStation").await?;

    Ok(())
}
