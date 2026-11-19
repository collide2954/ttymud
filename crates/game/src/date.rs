pub(crate) fn date(seconds: u64) -> String {
    let days = seconds / 86_400 + 719_468;
    let era = days / 146_097;
    let of_era = days - era * 146_097;
    let year_of_era = (of_era - of_era / 1_460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * of_year + 2) / 153;
    let day = of_year - (153 * shifted + 2) / 5 + 1;
    let month = if shifted < 10 {
        shifted + 3
    } else {
        shifted - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

pub(crate) fn stamp(seconds: u64) -> String {
    let (hour, minute) = (seconds / 3_600 % 24, seconds / 60 % 60);
    format!("{} {hour:02}:{minute:02}", date(seconds))
}

#[cfg(test)]
mod tests {
    use super::{date, stamp};

    #[test]
    fn stamps_add_the_hour_and_the_minute() {
        assert_eq!(stamp(60), "1970-01-01 00:01");
        assert_eq!(stamp(1_700_000_000), "2023-11-14 22:13");
    }

    #[test]
    fn dates_count_from_the_start_of_1970() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(86_399), "1970-01-01");
        assert_eq!(date(86_400), "1970-01-02");
    }

    #[test]
    fn dates_know_leap_years() {
        assert_eq!(date(951_782_400), "2000-02-29");
        assert_eq!(date(1_700_000_000), "2023-11-14");
        assert_eq!(date(4_107_542_400), "2100-03-01");
    }
}
