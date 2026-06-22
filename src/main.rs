use crate::app::App;
use config::load_config;
use setup::run_setup;
use std::io::{self, Write};
use tokio::time::{Duration, interval};

pub mod app;
mod config;
pub mod event;
mod setup;
pub mod ui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load config first, before anything else
    let config = match load_config() {
        Ok(c) => c,
        Err(e) if e.to_string() == "no_config" => {
            // No config file — run the setup wizard
            match run_setup() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Setup failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Configuration error:");
            eprintln!("  {}", e);
            eprintln!();
            eprintln!(
                "Config file location: {}",
                config::config_path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            );
            std::process::exit(1);
        }
    };

    // Spinner setup
    let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let mut frame = 0;

    let app_future = tokio::spawn(App::new(config));

    let mut ticker = interval(Duration::from_millis(80));
    loop {
        ticker.tick().await;
        print!(
            "\r {} Connecting to DownloadStation...",
            spinner_frames[frame % spinner_frames.len()]
        );
        io::stdout().flush()?;
        frame += 1;

        if app_future.is_finished() {
            break;
        }
    }

    // Clear the spinner line
    print!("\r{}\r", " ".repeat(50));
    io::stdout().flush()?;

    // Handle connection error cleanly before ratatui takes over
    let app = match app_future.await? {
        Ok(app) => app,
        Err(e) => {
            eprintln!("Failed to connect to DownloadStation:");
            eprintln!("  {}", e);
            eprintln!();
            eprintln!(
                "Please check your config file: {}",
                config::config_path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "unknown".to_string())
            );
            std::process::exit(1);
        }
    };

    // Only initialize ratatui after successful connection
    let terminal = ratatui::init();
    let result = app.run(terminal).await;
    ratatui::restore();

    if let Err(e) = result {
        eprintln!("Application error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
