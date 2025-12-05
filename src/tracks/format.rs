use std::path::Path;

use super::error::WithTrackContext as _;
use url::form_urlencoded;

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

    let name = decode_url(name);

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
        Ok(name.trim().to_owned())
    } else {
        // We've already checked before that the bound is at an ASCII digit.
        #[allow(clippy::string_slice)]
        Ok(String::from(name[skip..].trim()))
    }
}
