use dstui::{api::SynologyClient, config::AppConfig, config::run_config_wizard, ui::app::App};
use std::error::Error;

#[tokio::main]
async fn main() {
    // Run our real async entrypoint
    let result = async_main().await;

    // Restore the terminal state
    ratatui::restore();

    if let Err(err) = result {
        eprintln!("Application error: {}", err);
        std::process::exit(1);
    }
}

// Load or create dstui config
async fn async_main() -> Result<(), Box<dyn Error>> {
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
        .map_err(|e| format!("Login failed: {}", e))?;

    // Launching the main app
    {
        let mut app_term = ratatui::init();
        let mut app = App::new();

        // Get refresh interval from the config file
        let interval = config.refresh_interval;

        app.load_tasks(&client).await;
        app.run(&mut app_term, &mut client, interval).await?;
    }
    // Restore terminal
    ratatui::restore();

    // Call logout method to close the connection
    client.logout("DownloadStation").await?;

    Ok(())
}
