//! Date formatting and resolution helpers.

use std::path::Path;

/// Resolve date keywords in a metadata date string.
/// `today`/`now` -> current date, `last-modified` -> file mtime.
/// Returns the resolved date string, or None if the input is not a keyword.
pub(crate) fn resolve_date(date: &str, date_format: Option<&str>, input_path: Option<&Path>) -> Option<String> {
    let date = date.trim();
    let secs = match date {
        "today" | "now" => {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        }
        "last-modified" | "last_modified" | "lastmodified" => {
            let path = input_path?;
            let meta = std::fs::metadata(path).ok()?;
            meta.modified().ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        }
        _ => return None,
    };
    Some(match date_format {
        Some(fmt) => format_date(secs, fmt),
        None => epoch_days_to_date(secs / 86400),
    })
}

/// Convert days since Unix epoch to YYYY-MM-DD string.
pub(crate) fn epoch_days_to_date(total_days: u64) -> String {
    let mut y = 1970i64;
    let mut remaining = total_days as i64;

    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }

    let leap = is_leap(y);
    let month_days: [i64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];

    let mut m = 0;
    for (i, &days) in month_days.iter().enumerate() {
        if remaining < days {
            m = i;
            break;
        }
        remaining -= days;
    }

    format!("{:04}-{:02}-{:02}", y, m + 1, remaining + 1)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Format a YYYY-MM-DD date string with a strftime-style format string.
/// Supports: `%Y`, `%m`, `%d`, `%e`, `%B`, `%b`, `%A`, `%a`.
/// Returns the original string unchanged if parsing fails.
pub fn format_date_str(date: &str, fmt: &str) -> String {
    let parts: Vec<&str> = date.trim().split('-').collect();
    if parts.len() != 3 { return date.to_string(); }
    let (y, m, d) = match (parts[0].parse::<i64>(), parts[1].parse::<usize>(), parts[2].parse::<usize>()) {
        (Ok(y), Ok(m), Ok(d)) if m >= 1 && m <= 12 && d >= 1 && d <= 31 => (y, m, d),
        _ => return date.to_string(),
    };
    format_ymd(y, m, d, fmt)
}

pub(crate) fn format_date(secs: u64, fmt: &str) -> String {
    let days = secs / 86400;
    let ymd = epoch_days_to_date(days);
    let parts: Vec<&str> = ymd.split('-').collect();
    let (y, m, d) = (
        parts[0].parse::<i64>().unwrap_or(1970),
        parts[1].parse::<usize>().unwrap_or(1),
        parts[2].parse::<usize>().unwrap_or(1),
    );
    format_ymd(y, m, d, fmt)
}

fn format_ymd(y: i64, m: usize, d: usize, fmt: &str) -> String {
    static MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December",
    ];
    static MONTHS_SHORT: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    static DAYS: [&str; 7] = [
        "Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday",
    ];
    static DAYS_SHORT: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

    static T: [usize; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let yy = if m < 3 { y - 1 } else { y } as usize;
    let dow = (yy + yy / 4 - yy / 100 + yy / 400 + T[m - 1] + d) % 7;

    fmt.replace("%Y", &format!("{:04}", y))
        .replace("%m", &format!("{:02}", m))
        .replace("%d", &format!("{:02}", d))
        .replace("%e", &d.to_string())
        .replace("%B", MONTHS[m - 1])
        .replace("%b", MONTHS_SHORT[m - 1])
        .replace("%A", DAYS[dow])
        .replace("%a", DAYS_SHORT[dow])
}
