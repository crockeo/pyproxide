// reference: https://peps.python.org/pep-0440/
// notably i've chosen not to implement arbitrary equals (yet)
// because i've literally never seen it used in the wild

use std::cmp::Ordering;
use std::str::FromStr;

use lazy_static::lazy_static;
use regex::Regex;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreRelease {
    Alpha(u32),
    Beta(u32),
    ReleaseCandidate(u32),
}

impl PartialOrd for PreRelease {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        use PreRelease::*;

        let make_ord = |pre_release: PreRelease| match pre_release {
            Alpha(n) => (0, n),
            Beta(n) => (1, n),
            ReleaseCandidate(n) => (2, n),
        };

        let self_ord = make_ord(*self);
        let other_ord = make_ord(*other);

        self_ord.partial_cmp(&other_ord)
    }
}

impl ToString for PreRelease {
    fn to_string(&self) -> String {
        use PreRelease::*;

        match self {
            Alpha(n) => format!("a{n}"),
            Beta(n) => format!("b{n}"),
            ReleaseCandidate(n) => format!("rc{n}"),
        }
    }
}

#[derive(Clone, Eq, Debug, PartialEq)]
pub struct Version {
    epoch: Option<u32>,
    versions: Vec<u32>,
    pre_release: Option<PreRelease>,
    post_release: Option<u32>,
    dev_release: Option<u32>,
    local: Option<String>,
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if let Some(epoch_cmp) = self.epoch.partial_cmp(&other.epoch) {
            if epoch_cmp != Ordering::Equal {
                return Some(epoch_cmp);
            }
        }

	if let (None, Some(_)) = (self.pre_release, other.pre_release) {
	    return Some(Ordering::Greater);
	} else if let (Some(_), None) = (self.pre_release, other.pre_release) {
	    return Some(Ordering::Less);
	}

        let versions_cmp = self.versions.cmp(&other.versions);
        if versions_cmp != Ordering::Equal {
            return Some(versions_cmp);
        }

        if let Some(pre_release_cmp) = self.pre_release.partial_cmp(&other.pre_release) {
            if pre_release_cmp != Ordering::Equal {
                return Some(pre_release_cmp);
            }
        }

        if let Some(post_release_cmp) = self.post_release.partial_cmp(&other.post_release) {
            if post_release_cmp != Ordering::Equal {
                return Some(post_release_cmp);
            }
        }

        if let Some(dev_release_cmp) = self.dev_release.partial_cmp(&other.dev_release) {
            if dev_release_cmp != Ordering::Equal {
                return Some(dev_release_cmp);
            }
        }

        Some(Ordering::Equal)
    }
}

impl ToString for Version {
    fn to_string(&self) -> String {
        let epoch_part = if let Some(epoch) = self.epoch {
            format!("{epoch}!")
        } else {
            "".to_string()
        };
        let version_part = self
            .versions
            .iter()
            .map(u32::to_string)
            .collect::<Vec<String>>()
            .join(".");
        let pre_release_part = if let Some(pre_release) = self.pre_release {
            pre_release.to_string()
        } else {
            "".to_string()
        };
        let post_release_part = if let Some(post_release) = self.post_release {
            format!(".post{post_release}")
        } else {
            "".to_string()
        };
        let dev_release_part = if let Some(dev_release) = self.dev_release {
            format!(".dev{dev_release}")
        } else {
            "".to_string()
        };
        let local_part = if let Some(local) = &self.local {
            format!("+{local}")
        } else {
            "".to_string()
        };

        format!("{epoch_part}{version_part}{pre_release_part}{post_release_part}{dev_release_part}{local_part}")
    }
}

impl FromStr for Version {
    type Err = String;

    fn from_str(version_str: &str) -> Result<Self, Self::Err> {
        lazy_static! {
            static ref RE: Regex = Regex::new(
        r#"^((?P<epoch>\d+)!)?(?P<version>\d+(\.\d+)*)((?P<pre_release_kind>a|alpha|b|beta|rc)(?P<pre_release_num>\d+))?(\.post(?P<post_release>\d+))?(\.dev(?P<dev_release>\d+))?(\+(?P<local>.+))?$"#,
            ).unwrap();
        }

        let captures = RE
            .captures(version_str)
            .ok_or(format!("could not match version str: `{version_str}`"))?;

        let capture_number =
            |captures: &regex::Captures, capture_name: &str| -> Result<Option<u32>, &'static str> {
                if let Some(capture) = captures.name(capture_name) {
                    u32::from_str(capture.as_str())
                        .map(Some)
                        .map_err(|_| "failed to parse named capture")
                } else {
                    Ok(None)
                }
            };

        let mut versions = vec![];
        for version in captures
            .name("version")
            .ok_or("couldn't find required version part")?
            .as_str()
            .split('.')
        {
            let version = u32::from_str(version).map_err(|_| "could not parse version part")?;
            versions.push(version);
        }

        let pre_release = if let Some(pre_release_kind) = captures.name("pre_release_kind") {
            let pre_release_kind = match pre_release_kind.as_str() {
                "a" => PreRelease::Alpha,
                "b" => PreRelease::Beta,
                "rc" => PreRelease::ReleaseCandidate,
                other => return Err(format!("unexpected pre_release_kind: `{other}`")),
            };
            let pre_release_num = capture_number(&captures, "pre_release_num")?
                .ok_or("pre_release_kind without pre_release_num")?;
            Some(pre_release_kind(pre_release_num))
        } else {
            None
        };

        Ok(Self {
            epoch: capture_number(&captures, "epoch")?,
            versions,
            pre_release,
            post_release: capture_number(&captures, "post_release")?,
            dev_release: capture_number(&captures, "dev_release")?,
            local: captures.name("local").map(|m| m.as_str().to_owned()),
        })
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Operator {
    Compatible,
    Equals,
    NotEquals,
    GreaterThanOrEqual,
    LessThanOrEqual,
    GreaterThan,
    LessThan,
}

impl ToString for Operator {
    fn to_string(&self) -> String {
        use Operator::*;
        match self {
            Compatible => "~=".to_string(),
            Equals => "==".to_string(),
            NotEquals => "!=".to_string(),
            GreaterThanOrEqual => ">=".to_string(),
            LessThanOrEqual => "<=".to_string(),
            GreaterThan => ">".to_string(),
            LessThan => "<".to_string(),
        }
    }
}

// TODO: support wildcards in specifier comparisons
// e.g. !=3.16.*
// should mean no release in that range
// but i'm not sure how we'd handle that here
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Specifier {
    operator: Operator,
    version: Version,
}

impl ToString for Specifier {
    fn to_string(&self) -> String {
        format!("{}{}", self.operator.to_string(), self.version.to_string())
    }
}

impl FromStr for Specifier {
    type Err = String;

    fn from_str(specifier_str: &str) -> Result<Self, Self::Err> {
        lazy_static! {
            static ref RE: Regex =
                Regex::new(r#"(?P<operator>~=|==|!=|>=|<=|>|<)(?P<version>.+)"#).unwrap();
        }

        let captures = RE
            .captures(specifier_str)
            .ok_or(format!("could not match version str: `{specifier_str}`"))?;

        let operator = match captures.name("operator").unwrap().as_str() {
            "~=" => Operator::Compatible,
            "==" => Operator::Equals,
            "!=" => Operator::NotEquals,
            ">=" => Operator::GreaterThanOrEqual,
            "<=" => Operator::LessThanOrEqual,
            ">" => Operator::GreaterThan,
            "<" => Operator::LessThan,
            other => return Err(format!("invalid operator: `{other}`")),
        };
        let version = Version::from_str(captures.name("version").unwrap().as_str())?;

        Ok(Self { operator, version })
    }
}

impl Specifier {
    pub fn contains(&self, version: &Version) -> bool {
        use Operator::*;

        match self.operator {
            Compatible => todo!(),
            Equals => version == &self.version,
            NotEquals => version != &self.version,
            GreaterThanOrEqual => version >= &self.version,
            LessThanOrEqual => version <= &self.version,
            GreaterThan => version > &self.version,
            LessThan => version < &self.version,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpecifierSet {
    specifiers: Vec<Specifier>,
}

impl ToString for SpecifierSet {
    fn to_string(&self) -> String {
        self.specifiers
            .iter()
            .map(Specifier::to_string)
            .collect::<Vec<String>>()
            .join(",")
    }
}

impl FromStr for SpecifierSet {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let specifiers: Vec<Specifier> = s
            .split(',')
            .map(str::trim)
            .flat_map(Specifier::from_str)
            .collect::<Vec<Specifier>>();

        Ok(Self { specifiers })
    }
}

impl SpecifierSet {
    pub fn contains(&self, version: &Version) -> bool {
        for specifier in self.specifiers.iter() {
            if !specifier.contains(version) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn test_version_from_str() {
        let version_str = "2022!1.2.3rc3.post1.dev2";
        let version = Version::from_str(version_str);
        assert_eq!(
            version,
            Ok(Version {
                epoch: Some(2022),
                versions: vec![1, 2, 3],
                pre_release: Some(PreRelease::ReleaseCandidate(3)),
                post_release: Some(1),
                dev_release: Some(2),
		local: None,
            }),
        );
    }

    const SPECIFIER_SET_STR: &'static str = ">=1.2.3,<2";

    fn make_specifier_set() -> SpecifierSet {
        SpecifierSet {
            specifiers: vec![
                Specifier {
                    operator: Operator::GreaterThanOrEqual,
                    version: Version {
                        epoch: None,
                        versions: vec![1, 2, 3],
                        pre_release: None,
                        post_release: None,
                        dev_release: None,
			local: None,
                    },
                },
                Specifier {
                    operator: Operator::LessThan,
                    version: Version {
                        epoch: None,
                        versions: vec![2],
                        pre_release: None,
                        post_release: None,
                        dev_release: None,
			local: None,
                    },
                },
            ],
        }
    }

    #[test]
    fn test_specifier_set_from_str() {
        let specifier_set = SpecifierSet::from_str(SPECIFIER_SET_STR);
        assert_eq!(specifier_set, Ok(make_specifier_set()));
    }

    #[test]
    fn test_specifier_set_to_string() {
        let specifier_set_str = make_specifier_set().to_string();
        assert_eq!(specifier_set_str, SPECIFIER_SET_STR);
    }

    #[test]
    fn test_specifier_set_pre_releases() {
	let specifier_set = SpecifierSet::from_str(">=1.0.0").unwrap();
	let version = Version::from_str("1.0.0a0").unwrap();

	assert_eq!(specifier_set.contains(&version), false);
    }
}
