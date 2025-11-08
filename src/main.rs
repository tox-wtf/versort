use core::fmt;

use std::env::args;
use std::io::{self, BufRead};
use std::process::exit;
use std::sync::LazyLock;

use regex::Regex;

static RECOGNIZED_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"[0-9][-_\.]?(dev|pre|next|alpha|[^a-z]a|beta|[^a-z]b|rc|patch|[^a-z]p)"#).expect("Invalid regex"));
static COUNT_IS_CHAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"[^a-z]([a-z])$"#).expect("Invalid regex"));

static RKIND_DEV: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"dev"#).expect("Invalid regex"));
static RKIND_PRE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"pre"#).expect("Invalid regex"));
static RKIND_NEXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"next"#).expect("Invalid regex"));
static RKIND_ALPHA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^(alpha|a)([0-9]+)?$"#).expect("Invalid regex"));
static RKIND_BETA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^(beta|b)([0-9]+)?$"#).expect("Invalid regex"));
static RKIND_RC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^rc([0-9]+)?$"#).expect("Invalid regex"));
static RKIND_PATCH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^(patch|p)([0-9]+)?$"#).expect("Invalid regex"));

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub enum ReleaseKind {
    Dev,
    Pre,
    Next,
    Alpha,
    Beta,
    ReleaseCandidate,
    #[default]
    Stable,
    Patch,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct Semver {
    pub major: u64,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
    pub ident: Option<u64>,
    pub rkind: ReleaseKind,
    pub count: Option<u64>,
}

impl PartialOrd for Semver {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Semver {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.major.cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
            .then_with(|| self.ident.cmp(&other.ident))
            .then_with(|| self.rkind.cmp(&other.rkind))
            .then_with(|| self.count.cmp(&other.count))
    }
}

// FIXME: should account for count_is_char fuckery
impl fmt::Display for Semver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.major)?;
        if let Some(part) = self.minor { write!(f, ".{part}")?; }
        if let Some(part) = self.patch { write!(f, ".{part}")?; }
        if let Some(part) = self.ident { write!(f, ".{part}")?; }

        match self.rkind {
            ReleaseKind::Dev => write!(f, "-dev")?,
            ReleaseKind::Pre => write!(f, "-pre")?,
            ReleaseKind::Next => write!(f, "-next")?,
            ReleaseKind::Alpha => write!(f, "-alpha")?,
            ReleaseKind::Beta => write!(f, "-beta")?,
            ReleaseKind::ReleaseCandidate => write!(f, "-rc")?,
            ReleaseKind::Stable => return Ok(()),
            ReleaseKind::Patch => write!(f, "p")?,
        };

        if let Some(count) = self.count { write!(f, "{count}")?; }
        Ok(())
    }
}

#[derive(Debug)]
pub enum ParseSemverError {
    UnrecognizedText,
    MissingMajor,
}

impl fmt::Display for ParseSemverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnrecognizedText => write!(f, "Unrecognized text"),
            Self::MissingMajor => write!(f, "Missing major"),
        }
    }
}

fn recognized(s: &str, count_is_char: bool, lenient: bool) -> bool {
    if lenient {
        true
    } else if count_is_char {
        COUNT_IS_CHAR.is_match(s)
    } else {
        RECOGNIZED_RE.is_match(s)
    }
}

impl Semver {
    pub fn parse(naive: &str, lenient: bool, count_is_char: bool) -> Result<Self, ParseSemverError> {
        let s = naive.to_ascii_lowercase();
        let s = s.as_str();
        let s = if let Some(idx) = s.find(|c: char| c.is_ascii_alphabetic()) {
            if !recognized(s, count_is_char, lenient) {
                return Err(ParseSemverError::UnrecognizedText)
            }

            let mut str = s.to_string();

            // remove dot following the final character (e.g. 1.0.0-rc.1 -> 1.0.0-rc1)
            if let Some(letter_idx) = s.rfind(|c: char| c.is_ascii_alphabetic())
                && let Some(dot_idx) = s.rfind('.')
                && dot_idx == letter_idx + 1
            {
                str.remove(dot_idx);
            }

            str.insert(idx, '.');
            str
        } else {
            s.into()
        };

        // remove dashes or underscores (e.g. 1.0.0-rc1 -> 1.0.0rc1)
        let s = s.replace(['-',  '_'], "");

        let mut parts = s.split('.');
        let mut num_parts = parts.clone().filter_map(|p| p.parse::<u64>().ok());
        let mut semver = Self {
            major: num_parts.next().ok_or(ParseSemverError::MissingMajor)?,
            minor: num_parts.next(),
            patch: num_parts.next(),
            ident: num_parts.next(),
            ..Default::default()
        };

        if let Some(last_bit) = parts.next_back().filter(|p| p.parse::<u64>().is_err()) {
            if count_is_char && let Some(caps) = COUNT_IS_CHAR.captures(&s) {
                let m = caps.get(1).unwrap();
                let ct = m.as_str().chars().next().unwrap() as u64;
                semver.count = Some(ct);
            } else {
                // dbg!(&last_bit);
                // eprint!("Matched {last_bit} to ");
                semver.rkind = match &last_bit {
                    s if RKIND_DEV.is_match(s) => ReleaseKind::Dev,
                    s if RKIND_PRE.is_match(s) => ReleaseKind::Pre,
                    s if RKIND_NEXT.is_match(s) => ReleaseKind::Next,
                    s if RKIND_ALPHA.is_match(s) => ReleaseKind::Alpha,
                    s if RKIND_BETA.is_match(s) => ReleaseKind::Beta,
                    s if RKIND_RC.is_match(s) => ReleaseKind::ReleaseCandidate,
                    s if RKIND_PATCH.is_match(s) => ReleaseKind::Patch,
                    _ => ReleaseKind::Stable,
                };
                // eprintln!("{:?}", semver.rkind);
            }
        }

        if matches!(semver.rkind, ReleaseKind::Stable) {
            return Ok(semver)
        }

        if let Some(count) = s.rsplit_once(|c: char| c.is_ascii_alphabetic()).and_then(|ct| {
            let ct = ct.1;
            if ct.is_empty() { Some(1) } else { ct.parse::<u64>().ok() }
        }) {
            semver.count = Some(count);
        }

        // eprintln!("Parsed semver '{semver}' from '{naive}'");
        Ok(semver)
    }
}

fn main() {
    let mut ignore = false;
    let mut format = false;
    let mut lenient = false;
    let mut count_is_char = false;
    for arg in args().skip(1) {
        if arg.starts_with("--") {
            match arg.as_str() {
                "--ignore" => ignore = true,
                "--format" => format = true,
                "--lenient" => lenient = true,
                "--count-is-char" => count_is_char = true,
                _ => eprintln!("Unrecognized flag: {arg}"),
            }
        } else if arg.starts_with('-') && arg.len() > 1 {
            for ch in arg.chars().skip(1) {
                match ch {
                    'i' => ignore = true,
                    'f' => format = true,
                    'l' => lenient = true,
                    'c' => count_is_char = true,
                    _ => eprintln!("Unrecognized flag: {arg}")
                }
            }
        } else {
            eprintln!("Unrecognized argument: {arg}")
        }
    }

    let stdin = io::stdin();
    let reader = stdin.lock();

    let mut semvers = reader.lines()
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty())
        .filter_map(|v| {
            match Semver::parse(&v, lenient, count_is_char) {
                Ok(s) => Some((v, s)),
                Err(e) => {
                    if ignore { None }
                    else {
                        eprintln!("Failed to parse {v} into a semver: {e}");
                        exit(1);
                    }
                }
            }
        })
        .collect::<Vec<_>>();
    semvers.sort_by(|a, b| a.1.cmp(&b.1));

    if format {
        println! { "{}", semvers.iter().map(|t| t.1.to_string()).collect::<Vec<_>>().join("\n") }
    } else {
        println! { "{}", semvers.iter().map(|t| t.0.clone()).collect::<Vec<_>>().join("\n") }
    }
}
