use std::num::ParseIntError;
use std::ops::Add;
use time::{Date, Duration, Month};

pub struct TimeParser {
    regex_yyyymmdd: regex::Regex,
    regex_mmddyyyy: regex::Regex,
    regex_american_edge_case: regex::Regex,
    regex_engdate: regex::Regex,
}

impl TimeParser {
    pub fn new() -> TimeParser {
        TimeParser {
            regex_yyyymmdd: regex::Regex::new(r"(?:(\d{4})[/-])?(\d{1,2})[/-](\d{1,2})").unwrap(), // 2024/1/1
            regex_mmddyyyy: regex::Regex::new(r"(\d{1,2})[/-](\d{1,2})[/-]?(\d{1,4})?").unwrap(), // 1/1/2024
            regex_american_edge_case: regex::Regex::new(r"(\d{1,2})[/-](\d{1,2})[/-]?(\d{2})")
                .unwrap(), // 1/1/24
            regex_engdate: regex::Regex::new(r"(\w{3,16}) (\d{1,2})(?:\w{2})?(?:,? (\d{4}))?") // Jan 1st, 2024
                .unwrap(),
        }
    }

    pub fn parse(&self, ts: String, safety_net: bool) -> Option<u64> {
        if ts.is_empty() {
            return None;
        }

        let normalized_ts = ts.to_lowercase();

        if safety_net {
            self.parse_user_expires_string(normalized_ts)
                .map(|unixtime| self.safety_net(unixtime, &ts))
        } else {
            self.parse_user_expires_string(normalized_ts)
        }
    }

    fn parse_user_expires_string(&self, normalized_ts: String) -> Option<u64> {
        if normalized_ts.contains("next week") {
            return Some(next_week());
        }

        // stupid assumption: Swap numbers if time contains AM or PM
        let is_american = normalized_ts.contains("am") || normalized_ts.contains("pm");

        if is_american {
            if let Some(mtch) = self.regex_american_edge_case.captures(&normalized_ts) {
                return self
                    .handle_captures(mtch, Some(3), 1, 2, false, is_american)
                    .unwrap_or(None);
            }
        }

        if let Some(mtch) = self.regex_yyyymmdd.captures(&normalized_ts) {
            return self
                .handle_captures(mtch, Some(1), 2, 3, false, is_american)
                .unwrap_or(None);
        }

        if let Some(mtch) = self.regex_mmddyyyy.captures(&normalized_ts) {
            return self
                .handle_captures(mtch, Some(3), 1, 2, false, is_american)
                .unwrap_or(None);
        }

        if let Some(mtch) = self.regex_engdate.captures(&normalized_ts) {
            return self
                .handle_captures(mtch, Some(3), 1, 2, true, is_american)
                .unwrap_or(None);
        }

        info!(
            "Failed to parse date from '{}', no candidates matched.",
            normalized_ts
        );

        None
    }

    fn handle_captures(
        &self,
        mtch: regex::Captures,
        year_index: Option<usize>,
        mut month_index: usize,
        mut day_index: usize,
        month_is_string: bool,
        is_american: bool,
    ) -> Result<Option<u64>, ParseIntError> {
        if is_american && !month_is_string {
            debug!("Swapping month and day as american date indicated");
            (month_index, day_index) = (day_index, month_index);
        }

        let m = if month_is_string {
            let m_str = match mtch.get(month_index) {
                Some(m) => m.as_str().to_string(),
                None => return Ok(None),
            };

            self.month_from_str(m_str)
        } else {
            match mtch.get(month_index) {
                Some(m) => m.as_str().parse::<u8>(),
                None => return Ok(None),
            }?
        };

        let d = match mtch.get(day_index) {
            Some(m) => m.as_str().parse::<u8>(),
            None => return Ok(None),
        }?;

        let mut y = match year_index {
            Some(i) => match mtch.get(i) {
                Some(yr) => yr.as_str().parse::<i32>().unwrap_or(self.predict_year(m)),
                None => self.predict_year(m),
            },
            None => self.predict_year(m),
        };

        y = self.normalize_year(y);

        Ok(self.format_from_ymd(y, m, d))
    }

    fn format_from_ymd(&self, y: i32, mut m: u8, mut d: u8) -> Option<u64> {
        // perhaps wrongly assumed date is american
        if m > 12 && d <= 12 {
            warn!("Swapping month and day as month > 12 (m={}, d={})", m, d);
            (d, m) = (m, d);
        }

        let month = match Month::try_from(m) {
            Ok(m) => m,
            Err(_) => return None,
        };

        match Date::from_calendar_date(y, month, d) {
            Ok(d) => self.date_to_unix(d),
            Err(_) => None,
        }
    }

    fn predict_year(&self, month: u8) -> i32 {
        let now = time::OffsetDateTime::now_utc();
        let year = now.year();

        let parsed_month = match Month::try_from(month) {
            Ok(m) => m,
            Err(_) => return year,
        };

        if parsed_month.eq(&Month::January) && now.month().eq(&Month::December) {
            year + 1
        } else {
            year
        }
    }

    fn normalize_year(&self, mut year: i32) -> i32 {
        let this_year = time::OffsetDateTime::now_utc().year();

        if year < 1000 {
            year += 2000;
        }

        if this_year - 1 > year {
            warn!(
                "Year {} is less than current year {}, assuming this year.",
                year, this_year
            );
            year = this_year;
        }

        year
    }

    fn month_from_str(&self, m: String) -> u8 {
        match m.to_lowercase().as_str() {
            "jan" | "january" => 1,
            "feb" | "february" => 2,
            "mar" | "march" => 3,
            "apr" | "april" => 4,
            "may" => 5,
            "jun" | "june" => 6,
            "jul" | "july" => 7,
            "aug" | "august" => 8,
            "sep" | "september" => 9,
            "oct" | "october" => 10,
            "nov" | "november" => 11,
            "dec" | "december" => 12,
            _ => time::OffsetDateTime::now_utc().month() as u8,
        }
    }

    fn date_to_unix(&self, date: Date) -> Option<u64> {
        let ts = time::OffsetDateTime::new_utc(date, time::Time::MIDNIGHT).unix_timestamp();

        if ts < 0 {
            return None;
        }

        Some(ts as u64)
    }

    /// if ts is incredibly far in the future, just return next week.
    fn safety_net(&self, ts: u64, tsstring: &str) -> u64 {
        let nextweek = next_week();

        // 2764800 = 32 days in seconds
        if ts > nextweek + 2764800 {
            warn!(
                "Had to use safety net for date conversion of '{}', '{}'",
                ts, tsstring
            );
            return nextweek;
        }

        ts
    }
}

pub fn next_week() -> u64 {
    time::OffsetDateTime::now_utc()
        .date()
        .add(Duration::days(7))
        .midnight()
        .assume_utc()
        .unix_timestamp() as u64
}

pub fn validate_code(code: &str) -> bool {
    let clen = code.replace('-', "").len();

    clen == 16 || clen == 12
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_validate_code() {
        assert!(validate_code("1234-5678-1234-5678"));
        assert!(validate_code("1234567812345678"));
        assert!(!validate_code("1234-5678-1234-567"));
        assert!(!validate_code("123456781234567"));
    }

    struct TimeParseUnit {
        pub input: &'static str,
        pub expected: Option<u64>,
    }

    #[test]
    fn test_parse_expires_string() {
        zarthus_env_logger::init_named("liccrawler");

        const SPECIAL_CASE_KEY: u64 = 1;

        let time_parse_units: [TimeParseUnit; 15] = [
            TimeParseUnit {
                input: "next week",
                expected: Some(next_week()),
            },
            TimeParseUnit {
                input: "Next Week",
                expected: Some(next_week()),
            },
            TimeParseUnit {
                input: "idk",
                expected: None,
            },
            TimeParseUnit {
                input: "",
                expected: None,
            },
            TimeParseUnit {
                input: "Expires Jan 26th",
                expected: Some(SPECIAL_CASE_KEY),
            },
            TimeParseUnit {
                // note no need to handle 6AM PST
                // in the grand scheme of things the remote upcasts it to "next day" anyway.
                input: "Expires 1/15/24 6AM PST.",
                expected: Some(1705276800),
            },
            TimeParseUnit {
                // note no need to handle 6AM PST
                // in the grand scheme of things the remote upcasts it to "next day" anyway.
                input: "Expires 1/15/25 6AM PST.",
                expected: Some(1736899200),
            },
            TimeParseUnit {
                input: "This code is active until January 18th @ 2 PM PT.",
                expected: Some(SPECIAL_CASE_KEY),
            },
            TimeParseUnit {
                input: "Expires Jan 10, 2024",
                expected: Some(1704844800),
            },
            TimeParseUnit {
                input: "This code expires on 1/11 at 1:30pm ET/10:30am PT.",
                expected: Some(SPECIAL_CASE_KEY),
            },
            TimeParseUnit {
                input: "Expires 2-4 PM",
                expected: Some(SPECIAL_CASE_KEY),
            },
            TimeParseUnit {
                input: "Expires 3-4",
                expected: Some(SPECIAL_CASE_KEY),
            },
            TimeParseUnit {
                input: "Expires 2024-3-4",
                expected: Some(1711238400),
            },
            TimeParseUnit {
                input: "Expires 2024-3-4 PM",
                expected: Some(1711238400),
            },
            TimeParseUnit {
                input: "Expires 2024-1-1",
                expected: Some(1706054400),
            },
        ];

        let parser = TimeParser::new();

        for unit in time_parse_units.iter() {
            if let Some(ts) = unit.expected {
                if ts == SPECIAL_CASE_KEY {
                    assert!(
                        parser.parse(unit.input.to_string(), false).is_some(),
                        "Failed to parse: {}",
                        unit.input,
                    );

                    continue;
                }
            }

            assert_eq!(
                parser.parse(unit.input.to_string(), false),
                unit.expected,
                "Failed to parse: {}",
                unit.input,
            );
        }
    }

    #[test]
    fn test_safety_net() {
        let future = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            * 60
            * 24
            * 60;

        let parser = TimeParser::new();
        assert!(parser.safety_net(future, "test") < future);
    }
}
