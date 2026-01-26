use dialoguer::{Confirm, Input, Password, theme::ColorfulTheme};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self},
    path::PathBuf,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub server_url: String,
    pub server_port: u16,
    pub username: String,
    pub password: String,
    pub refresh_interval: u64,
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        dirs::config_dir() // Use the OS agnostic config dir on all systems
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dstui")
            .join("config.toml")
    }

    pub fn load() -> Option<Self> {
        let path = Self::config_path();
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
    }

    pub fn save(&self) -> io::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, toml::to_string_pretty(self).unwrap())
    }
}

pub fn run_config_wizard() -> io::Result<AppConfig> {
    let mut server_url = String::new();
    let mut server_port: u16 = 5000;
    let mut username = String::new();
    let mut password = String::new();
    let mut refresh_interval: u64 = 60;

    println!("- Thank you for trying dstui!");
    println!("! Configuration file not found. Please enter the following details:\n");

    let mut ok = false;

    while !ok {
        server_url = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Server URL")
            .with_initial_text("http://")
            .validate_with({
                move |input: &String| -> Result<(), &str> {
                    if input.starts_with("http://")
                        || input.starts_with("https://")
                            && input != "http://"
                            && input != "https://"
                    {
                        Ok(())
                    } else {
                        Err("Please enter a valid URL including http or https")
                    }
                }
            })
            .interact_text()
            .unwrap();

        if server_url.ends_with('/') {
            server_url.pop();
        }

        server_port = Input::with_theme(&ColorfulTheme::default())
            // server_port = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Server port")
            .with_initial_text("5000")
            .interact_text()
            .unwrap();

        username = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Username")
            .interact_text()
            .unwrap();

        password = Password::with_theme(&ColorfulTheme::default())
            .allow_empty_password(true)
            .with_prompt("Password")
            .with_confirmation("Confirm password", "Passwords mismatching")
            .interact()
            .unwrap();

        refresh_interval = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Refresh interval (in sec)")
            .with_initial_text("60")
            .interact_text()
            .unwrap();

        if let Some(true) = Confirm::new()
            .with_prompt("Confirm")
            .default(true)
            .wait_for_newline(true)
            .interact_opt()
            .unwrap()
        {
            ok = true;
        }
    }

    // Build final config
    Ok(AppConfig {
        server_url,
        server_port,
        username,
        password,
        refresh_interval,
    })
}
