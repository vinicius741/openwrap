pub fn retry_delay_seconds(retry_count: u8) -> Option<u64> {
    match retry_count {
        0 => Some(2),
        1 => Some(5),
        2 => Some(10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::retry_delay_seconds;

    #[test]
    fn first_retry_has_2_second_delay() {
        assert_eq!(retry_delay_seconds(0), Some(2));
    }

    #[test]
    fn second_retry_has_5_second_delay() {
        assert_eq!(retry_delay_seconds(1), Some(5));
    }

    #[test]
    fn third_retry_has_10_second_delay() {
        assert_eq!(retry_delay_seconds(2), Some(10));
    }

    #[test]
    fn no_delay_after_max_retries() {
        assert_eq!(retry_delay_seconds(3), None);
        assert_eq!(retry_delay_seconds(4), None);
        assert_eq!(retry_delay_seconds(10), None);
        assert_eq!(retry_delay_seconds(255), None);
    }
}
