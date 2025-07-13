/// Collection of small utilities used all over the place
use chrono::{DateTime, Utc};
use clipboard_rs::{self, Clipboard};
use humantime::format_duration;
use std::{fs, path::PathBuf, time::Duration};

pub fn format_timestamp(ts: u64) -> String {
    let time = DateTime::from_timestamp(ts as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "N/A".into());
    if time == "1970-01-01 00:00:00" {
        "-".to_string()
    } else {
        time
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;

    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    if size == 0.0 {
        return "0 B".to_string();
    }

    format!("{:.2} {}", size, UNITS[unit])
}

pub fn format_seconds(secs: u64) -> String {
    format_duration(Duration::from_secs(secs)).to_string()
}

pub fn calculate_elapsed_time(start: u64, finish: u64) -> String {
    let now_ts = Utc::now().timestamp() as u64;
    if finish == 0 {
        format_seconds(now_ts.saturating_sub(start))
    } else {
        format_seconds(finish.saturating_sub(start))
    }
}

pub fn render_progress_bar(percentage: u64, width: usize) -> String {
    let percent_text = format!("{percentage:>3}%"); // e.g. " 42%"
    let text_len = percent_text.len();

    // how many blocks should be “filled” (left of the bar)
    let filled = (percentage as usize * width / 100).min(width);

    // Build the bar character by character
    let mut bar = String::with_capacity(width);
    for i in 0..width {
        // if current position is within the slice where the text should go:
        let start = (width.saturating_sub(text_len)) / 2;
        let end = start + text_len;
        if i >= start && i < end {
            // insert the appropriate character from percent_text
            let ch = percent_text.chars().nth(i - start).unwrap();
            bar.push(ch);
        } else {
            // otherwise draw filled or empty block
            bar.push(if i < filled { '█' } else { ' ' });
        }
    }

    format!("[{bar}]")
}

// Get text from clipboard
pub fn get_clipboard() -> String {
    let ctx = clipboard_rs::ClipboardContext::new().unwrap();
    ctx.get_text().unwrap_or("".to_string())
}

// Validate if the URL from the clipboard is OK
pub fn validate_url(url: &str) -> Result<(), String> {
    if url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("ftp://")
        || url.starts_with("ftps://")
        || url.starts_with("sftp://")
        || url.starts_with("magnet:")
        || url.starts_with("thunder://")
        || url.starts_with("flashget://")
        || url.starts_with("qqdl://")
    {
        Ok(())
    } else {
        Err(format!("Invalid URL: {url}"))
    }
}

pub fn get_files() -> Vec<(String, String)> {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut items = vec![];

    // Manually add "." and ".."
    items.push((".".to_string(), "Dir".to_string()));
    items.push(("..".to_string(), "Dir".to_string()));

    if let Ok(entries) = fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            let file_name = entry
                .file_name()
                .into_string()
                .unwrap_or_else(|_| "???".to_string());

            let file_type = match entry.file_type() {
                Ok(ft) if ft.is_dir() => "Dir",
                Ok(ft) if ft.is_file() => "File",
                Ok(ft) if ft.is_symlink() => "Symlink",
                _ => "Unknown",
            };

            items.push((file_name, file_type.to_string()));
        }
    }
    // Sort with custom comparator
    items.sort_by(|a, b| {
        match (a.0.as_str(), b.0.as_str()) {
            // "." and ".." always go first
            ("." | "..", "." | "..") => a.0.cmp(&b.0),
            ("." | "..", _) => std::cmp::Ordering::Less,
            (_, "." | "..") => std::cmp::Ordering::Greater,
            // Directories before files
            (_, _) if a.1 == b.1 => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
            (_, _) if a.1 == "Dir" => std::cmp::Ordering::Less,
            (_, _) if b.1 == "Dir" => std::cmp::Ordering::Greater,
            // Otherwise keep order
            _ => std::cmp::Ordering::Equal,
        }
    });

    items
}

// Half-assed attempt on testing
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
    }

    #[test]
    fn test_render_progress_bar() {
        assert_eq!(render_progress_bar(50, 10), "[███ 50%   ]");
        assert_eq!(render_progress_bar(100, 10), "[███100%███]");
    }

    #[test]
    fn test_format_seconds() {
        assert_eq!(format_seconds(50), "50s");
        assert_eq!(format_seconds(121), "2m 1s");
        assert_eq!(format_seconds(25674), "7h 7m 54s");
        assert_eq!(format_seconds(9879654), "3months 23days 40m 6s");
    }

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(1271651654), "2010-04-19 04:34:14");
        assert_eq!(format_timestamp(989791271651654), "N/A");
    }
}
