pub fn parse_user_expires_string(ts: String) -> Option<u64> {
    if ts.is_empty() {
        return None;
    }

    let normalized_ts = ts.to_lowercase();

    if normalized_ts.contains("next week") {
        return Some(next_week());
    }

    None
}

pub fn next_week() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 60 * 60 * 24 * 7
}

pub fn validate_code(code: &str) -> bool {
    let clen = code.replace('-', "").len();

    clen == 16 || clen == 12
}
