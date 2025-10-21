use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr, time::Duration};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DurationWrapper(Duration);

impl From<Duration> for DurationWrapper {
    fn from(value: Duration) -> Self {
        Self(value)
    }
}

impl Into<Duration> for DurationWrapper {
    fn into(self) -> Duration {
        self.0
    }
}

impl From<String> for DurationWrapper {
    fn from(value: String) -> Self {
        Self::from_str(value.as_ref()).unwrap_or(Self(Duration::new(600, 0)))
    }
}

impl FromStr for DurationWrapper {
    type Err = DurationParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_lowercase().replace(" ", "");
        let pattern = &['s', 'm', 'h'];
        let matches = s.split(pattern).zip(s.matches(pattern));

        let mut dur = Duration::ZERO;
        let mut iterations = 0;
        for (text, unit) in matches {
            iterations += 1;
            let val: u64 = text
                .parse()
                .map_err(|_| DurationParseError::InvalidNumber)?;
            match unit {
                "s" => dur += Duration::new(val, 0),
                "m" => dur += Duration::new(val * 60, 0),
                "h" => dur += Duration::new(val * 3600, 0),
                _ => break,
            }
        }

        if iterations == 0 {
            Err(DurationParseError::DurationNotFound)
        } else {
            Ok(Self(dur))
        }
    }
}

impl Display for DurationWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut secs = self.0.as_secs();
        let mins = secs / 60;
        secs = secs % 60;

        write!(f, "{mins}m{secs}s")
    }
}

pub enum DurationParseError {
    InvalidNumber,
    DurationNotFound,
}
