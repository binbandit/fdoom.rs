//! Port of `fdoom.saveload.Version`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    make: i32,
    major: i32,
    minor: i32,
    dev: i32,
    valid: bool,
}

impl Version {
    pub fn new(version: &str) -> Version {
        Self::parse(version, true)
    }

    fn parse(version: &str, print_error: bool) -> Version {
        let mut v = Version {
            make: 0,
            major: 0,
            minor: 0,
            dev: 0,
            valid: true,
        };
        let nums: Vec<&str> = version.split('.').collect();

        let result: Result<(), std::num::ParseIntError> = (|| {
            if !nums.is_empty() && !nums[0].is_empty() {
                v.make = nums[0].parse()?;
            }
            if nums.len() > 1 {
                v.major = nums[1].parse()?;
            }
            let min = if nums.len() > 2 { nums[2] } else { "" };
            if min.contains('-') {
                let mindev: Vec<&str> = min.split('-').collect();
                v.minor = mindev[0].parse()?;
                v.dev = mindev[1].replace("pre", "").replace("dev", "").parse()?;
            } else if !min.is_empty() {
                v.minor = min.parse()?;
            }
            Ok(())
        })();

        if result.is_err() {
            if print_error {
                eprintln!("INVALID version number: \"{version}\"");
            }
            v.valid = false;
        }
        v
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn is_valid_str(version: &str) -> bool {
        Self::parse(version, false).is_valid()
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, ov: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        if self.make != ov.make {
            return self.make.cmp(&ov.make);
        }
        if self.major != ov.major {
            return self.major.cmp(&ov.major);
        }
        if self.minor != ov.minor {
            return self.minor.cmp(&ov.minor);
        }
        if self.dev != ov.dev {
            if self.dev == 0 {
                return Ordering::Greater; // 0 is the last "dev" version, as it is not a dev
            }
            if ov.dev == 0 {
                return Ordering::Less;
            }
            return self.dev.cmp(&ov.dev);
        }
        Ordering::Equal
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}.{}{}",
            self.make,
            self.major,
            self.minor,
            if self.dev == 0 {
                String::new()
            } else {
                format!("-dev{}", self.dev)
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_compares() {
        assert!(Version::new("2.6") > Version::new("2.5.9"));
        assert!(Version::new("2.0.4-dev3") < Version::new("2.0.4"));
        assert!(Version::new("2.0.4-pre2") < Version::new("2.0.4-dev3"));
        assert_eq!(Version::new("2.6").to_string(), "2.6.0");
        assert_eq!(Version::new("1.9.4-dev3").to_string(), "1.9.4-dev3");
        assert!(!Version::new("bogus").is_valid());
        assert!(Version::is_valid_str("2.0.4"));
    }
}
