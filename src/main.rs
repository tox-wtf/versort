use core::fmt;

use std::env::args;
use std::io::{self, BufRead};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed as Lax};
use std::sync::LazyLock;

use regex::Regex;

static RECOGNIZED_RE:   LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"[0-9][-_\.]?(dev|pre|next|alpha|[^a-z]a|beta|[^a-z]b|r?c|patch|[^a-z]p)"#).expect("Invalid regex"));
static COUNT_IS_CHAR:   LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"[^a-z]([a-z])$"#).expect("Invalid regex"));

static RKIND_DEV:       LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"dev"#).expect("Invalid regex"));
static RKIND_PRE:       LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"pre"#).expect("Invalid regex"));
static RKIND_NEXT:      LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"next"#).expect("Invalid regex"));
static RKIND_ALPHA:     LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^(alpha|a)([0-9]+)?$"#).expect("Invalid regex"));
static RKIND_BETA:      LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^(beta|b)([0-9]+)?$"#).expect("Invalid regex"));
static RKIND_RC:        LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^r?c([0-9]+)?$"#).expect("Invalid regex"));
static RKIND_PATCH:     LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^(patch|p)([0-9]+)?$"#).expect("Invalid regex"));

static VERBOSE:         AtomicBool      = AtomicBool::new(false);
static FORMAT:          AtomicBool      = AtomicBool::new(false);
static LENIENT:         AtomicBool      = AtomicBool::new(false);
static IGNORE:          AtomicBool      = AtomicBool::new(false);
static CHARCOUNT:       AtomicBool      = AtomicBool::new(false);

macro_rules! die        { ($($arg:tt)*) => {{ eprintln!($($arg)*); std::process::exit(1); }}; }
macro_rules! quit       { ($($arg:tt)*) => {{ println!($($arg)*); std::process::exit(0); }}; }
macro_rules! vprint     { ($($arg:tt)*) => {{ if VERBOSE.load(Lax) { eprint!($($arg)*); } }}; }
macro_rules! vprintln   { ($($arg:tt)*) => {{ if VERBOSE.load(Lax) { eprintln!($($arg)*); } }}; }

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub enum ReleaseKind {
    Dev,
    Pre,
    Next,
    Alpha,
    Beta,
    Rc,
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

impl fmt::Display for Semver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.major)?;
        if let Some(part) = self.minor { write!(f, ".{part}")?; }
        if let Some(part) = self.patch { write!(f, ".{part}")?; }
        if let Some(part) = self.ident { write!(f, ".{part}")?; }

        match self.rkind {
            ReleaseKind::Dev    => write!(f, "-dev")?,
            ReleaseKind::Pre    => write!(f, "-pre")?,
            ReleaseKind::Next   => write!(f, "-next")?,
            ReleaseKind::Alpha  => write!(f, "-alpha")?,
            ReleaseKind::Beta   => write!(f, "-beta")?,
            ReleaseKind::Rc     => write!(f, "-rc")?,
            ReleaseKind::Patch  => write!(f, "p")?,
            ReleaseKind::Stable => {},
        };

        if let Some(count) = self.count {
            if CHARCOUNT.load(Lax) {
                // SAFETY: `count` is derived from an ASCII alphabetic character
                write!(f, "{}", unsafe { char::from_u32_unchecked(count as u32) })?;
            } else {
                write!(f, "{count}")?;
            }
        }

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

fn recognized(s: &str) -> bool {
    if CHARCOUNT.load(Lax) {
        COUNT_IS_CHAR.is_match(s)
    } else {
        RECOGNIZED_RE.is_match(s)
    }
}

impl FromStr for Semver {
    type Err = ParseSemverError;

    fn from_str(naive: &str) -> Result<Self, Self::Err> {
        let mut s = naive.to_ascii_lowercase();

        if let Some(idx) = s.find(|c: char| c.is_ascii_alphabetic()) {
            if !recognized(&s) || !LENIENT.load(Lax) {
                return Err(ParseSemverError::UnrecognizedText)
            }

            // remove dot following the final character (e.g. 1.0.0-rc.1 -> 1.0.0-rc1)
            if let Some(letter_idx) = s.rfind(|c: char| c.is_ascii_alphabetic())
                && let Some(dot_idx) = s.rfind('.')
                && dot_idx == letter_idx + 1
            {
                s.remove(dot_idx);
            }

            s.insert(idx, '.');
        }

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
            if CHARCOUNT.load(Lax) && let Some(caps) = COUNT_IS_CHAR.captures(&s) {
                let m = caps.get(1).unwrap();
                let ct = m.as_str().chars().next().unwrap() as u64;
                semver.count = Some(ct);
            } else {
                vprint!("Matched {last_bit} to ");
                semver.rkind = match &last_bit {
                    s if RKIND_DEV.is_match(s) => ReleaseKind::Dev,
                    s if RKIND_PRE.is_match(s) => ReleaseKind::Pre,
                    s if RKIND_NEXT.is_match(s) => ReleaseKind::Next,
                    s if RKIND_ALPHA.is_match(s) => ReleaseKind::Alpha,
                    s if RKIND_BETA.is_match(s) => ReleaseKind::Beta,
                    s if RKIND_RC.is_match(s) => ReleaseKind::Rc,
                    s if RKIND_PATCH.is_match(s) => ReleaseKind::Patch,
                    _ => ReleaseKind::Stable,
                };
                vprintln!("{:?}", semver.rkind);
            }
        }

        if !matches!(semver.rkind, ReleaseKind::Stable)
        && let Some(count) = s.rsplit_once(|c: char| c.is_ascii_alphabetic()).and_then(|ct| {
            let ct = ct.1;
            if ct.is_empty() { Some(1) } else { ct.parse::<u64>().ok() }
        }) {
            semver.count = Some(count);
        }

        vprintln!("Parsed semver '{semver}' from '{naive}'");
        Ok(semver)
    }
}

fn help() {
    quit! {
"\
\x1b[4;1mUsage:\x1b[0;1m versort \x1b[0m[OPTIONS]

\x1b[4;1mOptions:\x1b[0m
    \x1b[1m-i | --ignore\x1b[0m       ignore versions that could not be parsed
    \x1b[1m-f | --format\x1b[0m       format versions in output
    \x1b[1m-l | --lenient\x1b[0m      parse versions more leniently
    \x1b[1m-c | --charcount\x1b[0m    treat a single trailing character as a counter

    \x1b[1m-v | --verbose\x1b[0m      print verbose messages to stderr
    \x1b[1m-h | --help\x1b[0m         display help
    \x1b[1m-V | --version\x1b[0m      display version

\x1b[4;1mExamples:\x1b[0m
    \x1b[1mversort\x1b[0m < data.txt
    sed 's/^v//' data.txt | \x1b[1mversort\x1b[0m -lif
"
    }
}

fn version() {
    quit!("versort {}", env!("CARGO_PKG_VERSION"));
}

fn main() {
    for arg in args().skip(1) {
        if arg.starts_with("--") {
            match arg.as_str() {
                "--ignore" => IGNORE.store(true, Lax),
                "--format" => FORMAT.store(true, Lax),
                "--lenient" => LENIENT.store(true, Lax),
                "--charcount" => CHARCOUNT.store(true, Lax),
                "--verbose" => VERBOSE.store(true, Lax),
                "--help" => help(),
                "--version" => version(),
                _ => die!("Unrecognized flag: {arg}"),
            }
        } else if arg.starts_with('-') && arg.len() > 1 {
            for ch in arg.chars().skip(1) {
                match ch {
                    'i' => IGNORE.store(true, Lax),
                    'f' => FORMAT.store(true, Lax),
                    'l' => LENIENT.store(true, Lax),
                    'c' => CHARCOUNT.store(true, Lax),
                    'v' => VERBOSE.store(true, Lax),
                    'h' => help(),
                    'V' => version(),
                    _ => die!("Unrecognized flag: {arg}")
                }
            }
        } else {
            die!("Unrecognized argument: {arg}")
        }
    }

    let stdin = io::stdin();
    let reader = stdin.lock();

    let mut semvers = reader.lines()
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty())
        .filter_map(|v| {
            match v.parse::<Semver>() {
                Ok(s) => Some((v, s)),
                Err(e) => {
                    if IGNORE.load(Lax) { None }
                    else { die!("Failed to parse {v} into a semver: {e}"); }
                }
            }
        })
        .collect::<Vec<_>>();
    semvers.sort_by(|a, b| a.1.cmp(&b.1));

    if FORMAT.load(Lax) {
        println! { "{}", semvers.iter().map(|t| t.1.to_string()).collect::<Vec<_>>().join("\n") }
    } else {
        println! { "{}", semvers.iter().map(|t| t.0.clone()).collect::<Vec<_>>().join("\n") }
    }
}
