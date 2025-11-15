use convert_case::{Case, Casing};
use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;
use url::form_urlencoded;

use super::error::WithTrackContext;

lazy_static! {
    static ref MASTER_PATTERNS: [Regex; 5] = [
        // (master), (master v2)
        Regex::new(r"\s*\(.*?master(?:\s*v?\d+)?\)$").unwrap(),
        // mstr or - mstr or (mstr) — now also matches "mstr v3", "mstr2", etc.
        Regex::new(r"\s*[-(]?\s*mstr(?:\s*v?\d+)?\s*\)?$").unwrap(),
        // - master, master at end without parentheses
        Regex::new(r"\s*[-]?\s*master(?:\s*v?\d+)?$").unwrap(),
        // kupla master1, kupla master v2 (without parentheses or separator)
        Regex::new(r"\s+kupla\s+master(?:\s*v?\d+|\d+)?$").unwrap(),
        // (kupla master) followed by trailing parenthetical numbers, e.g. "... (kupla master) (1)"
        Regex::new(r"\s*\(.*?master(?:\s*v?\d+)?\)(?:\s*\(\d+\))+$").unwrap(),
    ];
    static ref ID_PATTERN: Regex = Regex::new(r"^[a-z]\d[ .]").unwrap();
}

/// Decodes a URL string into normal UTF-8.
fn decode_url(text: &str) -> String {
    // The tuple contains smart pointers, so it's not really practical to use `into()`.
    #[allow(clippy::tuple_array_conversions)]
    form_urlencoded::parse(text.as_bytes())
        .map(|(key, val)| [key, val].concat())
        .collect()
}

/// Formats a name with [`convert_case`].
///
/// This will also strip the first few numbers that are
/// usually present on most lofi tracks and do some other
/// formatting operations.
pub fn name(name: &str) -> super::Result<String> {
    let path = Path::new(name);

    let name = path
        .file_stem()
        .and_then(|x| x.to_str())
        .ok_or(super::error::Kind::InvalidName)
        .track(name)?;

    let name = decode_url(name).to_lowercase();
    let mut name = name
        .replace("masster", "master")
        .replace("(online-audio-converter.com)", "") // Some of these names, man...
        .replace('_', " ");

    // Get rid of "master" suffix with a few regex patterns.
    for regex in MASTER_PATTERNS.iter() {
        name = regex.replace(&name, "").to_string();
    }

    name = ID_PATTERN.replace(&name, "").to_string();

    let name = name
        .replace("13lufs", "")
        .to_case(Case::Title)
        .replace(" .", "")
        .replace(" Ft ", " ft. ")
        .replace("Ft.", "ft.")
        .replace("Feat.", "ft.")
        .replace(" W ", " w/ ");

    // This is incremented for each digit in front of the song name.
    let mut skip = 0;

    for character in name.as_bytes() {
        if character.is_ascii_digit()
            || *character == b'.'
            || *character == b')'
            || *character == b'('
        {
            skip += 1;
        } else {
            break;
        }
    }

    // If the entire name of the track is a number, then just return it.
    if skip == name.len() {
        Ok(name.trim().to_string())
    } else {
        // We've already checked before that the bound is at an ASCII digit.
        #[allow(clippy::string_slice)]
        Ok(String::from(name[skip..].trim()))
    }
}
