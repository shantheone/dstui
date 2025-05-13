// tests/config_roundtrip.rs

use dstui::config::AppConfig;
use std::fs;

#[test]
fn test_config_roundtrip() {
    let path = AppConfig::config_path();
    let _ = fs::remove_file(&path); // clean up first

    let config = AppConfig {
        server_url: "http://localhost".into(),
        port: 5000,
        username: "user".into(),
        password: "pass".into(),
        refresh_interval: 30,
    };

    config.save().unwrap();
    let loaded = AppConfig::load().unwrap();

    assert_eq!(config.server_url, loaded.server_url);
    assert_eq!(config.port, loaded.port);
    assert_eq!(config.username, loaded.username);
    assert_eq!(config.password, loaded.password);
    assert_eq!(config.refresh_interval, loaded.refresh_interval);

    // remove test config file
    let _ = fs::remove_file(&path);
}
