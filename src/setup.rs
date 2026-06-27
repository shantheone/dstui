use crate::config::{Config, ConnectionConfig, DownloadConfig, SortConfig, config_path};
use anyhow::Result;
use std::io::{self, Write};

fn prompt(label: &str, default: &str) -> Result<String> {
    if default.is_empty() {
        print!("  {} : ", label);
    } else {
        print!("  {} [{}]: ", label, default);
    }
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim().to_string();

    if trimmed.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(trimmed)
    }
}

fn prompt_password(label: &str) -> Result<String> {
    print!("  {} : ", label);
    io::stdout().flush()?;
    // Use rpassword for hidden input
    let password = rpassword::read_password()?;
    Ok(password)
}

fn prompt_optional_u64(label: &str, default: Option<u64>) -> Result<Option<u64>> {
    let default_str = default
        .map(|v| v.to_string())
        .unwrap_or_else(|| "off".to_string());
    print!("  {} [{}] (0 or 'off' to disable): ", label, default_str);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim().to_string();

    if trimmed.is_empty() {
        Ok(default)
    } else if trimmed == "off" || trimmed == "0" {
        Ok(None)
    } else {
        Ok(trimmed
            .parse::<u64>()
            .ok()
            .filter(|&v| v > 0)
            .map(Some)
            .unwrap_or(default))
    }
}

pub fn run_setup() -> Result<Config> {
    println!();
    println!("  ╭─────────────────────────────────────╮");
    println!("  │   dstui — first time setup wizard   │");
    println!("  ╰─────────────────────────────────────╯");
    println!();
    println!("  No config file found. Let's create one.");
    println!("  Press Enter to accept the default value shown in brackets.");
    println!();

    println!("  ── Connection ───────────────────────────────────────────────────");
    let url = prompt("DiskStation URL", "http://diskstation:5000")?;
    let username = prompt("Username", "admin")?;
    let password = prompt_password("Password")?;
    let accept_invalid_certs = {
        let input = prompt("Accept invalid/self-signed certificates? (y/n)", "n")?;
        input.trim().to_lowercase() == "y"
    };
    println!();

    println!("  ── Downloads ────────────────────────────────────────────────────");
    let destination = prompt("Download destination", "downloads")?;
    let refresh_interval = prompt_optional_u64("Auto-refresh interval (seconds)", Some(30))?;
    println!();

    println!("  ── Sorting ──────────────────────────────────────────────────────");
    let sort_column = {
        println!("  Available columns:          name, size, downloaded, uploaded,");
        println!("                              progress, uploadspeed, downloadspeed,");
        println!("                              ratio, status");
        prompt("Default sort column", "name")?
    };
    let sort_order = {
        let input = prompt("Default sort order (ascending/descending)", "ascending")?;
        if input.to_lowercase().starts_with('d') {
            "descending".to_string()
        } else {
            "ascending".to_string()
        }
    };
    println!();

    let config = Config {
        connection: ConnectionConfig {
            url,
            username,
            password,
            accept_invalid_certs,
        },
        downloads: DownloadConfig {
            destination,
            refresh_interval,
        },
        sorting: SortConfig {
            column: sort_column,
            order: sort_order,
        },
    };

    // Write to file
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let toml = toml::to_string_pretty(&config)?;
    std::fs::write(&path, &toml)?;

    // Change file permissions to owner read/write only
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }

    println!("  Config saved to: {}", path.display());
    println!();

    Ok(config)
}
