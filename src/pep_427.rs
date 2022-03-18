// reference: https://peps.python.org/pep-0427/#file-name-convention

use lazy_static::lazy_static;
use regex::Regex;
use std::str::FromStr;

#[derive(Eq, Debug, PartialEq)]
pub struct WheelInfo {
    pub distribution: String,
    pub version: String,
    pub build_tag: Option<String>,
    pub python_tag: String,
    pub abi_tag: String,
    pub platform_tag: String,
}

impl ToString for WheelInfo {
    fn to_string(&self) -> String {
        let mut components = vec![&self.distribution, &self.version];
        if let Some(build_tag) = &self.build_tag {
            components.push(build_tag);
        }
        components.extend(vec![&self.python_tag, &self.abi_tag, &self.platform_tag]);

        format!(
            "{}.whl",
            components
                .into_iter()
                .map(String::as_str)
                .collect::<Vec<&str>>()
                .join("-"),
        )
    }
}

impl FromStr for WheelInfo {
    type Err = &'static str;

    fn from_str(wheel_name: &str) -> Result<Self, Self::Err> {
        lazy_static! {
            static ref RE: Regex = Regex::new(
        r#"^(?P<distribution>.+)-(?P<version>.+)(-(?P<build_tag>.+))?-(?P<python_tag>.+)-(?P<abi_tag>.+)-(?P<platform_tag>.+)\.whl$"#
            ).unwrap();
        }

        let captures = RE
            .captures(wheel_name.as_ref())
            .ok_or("could not match wheel name")?;

        let unwrap_capture = |captures: &regex::Captures, capture_name: &str| -> String {
            captures.name(capture_name).unwrap().as_str().to_owned()
        };
        Ok(WheelInfo {
            distribution: unwrap_capture(&captures, "distribution"),
            version: unwrap_capture(&captures, "version"),
            build_tag: captures.name("build_tag").map(|m| m.as_str().to_owned()),
            python_tag: unwrap_capture(&captures, "python_tag"),
            abi_tag: unwrap_capture(&captures, "abi_tag"),
            platform_tag: unwrap_capture(&captures, "platform_tag"),
        })
    }
}
