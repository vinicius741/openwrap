pub fn retry_delay_seconds(retry_count: u8) -> Option<u64> {
    match retry_count {
        0 => Some(2),
        1 => Some(5),
        2 => Some(10),
        _ => None,
    }
}

