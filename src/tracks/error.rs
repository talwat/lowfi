pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Kind {
    #[error("unable to decode: {0}")]
    Decode(#[from] rodio::decoder::DecoderError),

    #[error("invalid name")]
    InvalidName,

    #[error("invalid file path")]
    InvalidPath,

    #[error("unknown target track length")]
    UnknownLength,

    #[error("unable to read file: {0}")]
    File(#[from] std::io::Error),

    #[error("unable to fetch data: {0}")]
    Request(#[from] reqwest::Error),

    #[error("couldn't handle integer track length: {0}")]
    Integer(#[from] std::num::TryFromIntError),
}

#[derive(Debug, thiserror::Error)]
#[error("{kind}{}", self.track.as_ref().map_or(String::new(), |t| format!(" (track: {t:?}) ")))]
pub struct Error {
    pub track: Option<String>,
    pub kind: Kind,
}

impl Error {
    pub fn timeout(&self) -> bool {
        if let Kind::Request(x) = &self.kind {
            x.is_timeout()
        } else {
            false
        }
    }
}

impl<T, E> From<(T, E)> for Error
where
    T: Into<String>,
    Kind: From<E>,
{
    fn from((track, err): (T, E)) -> Self {
        Self {
            track: Some(track.into()),
            kind: Kind::from(err),
        }
    }
}

impl<E> From<E> for Error
where
    Kind: From<E>,
{
    fn from(err: E) -> Self {
        Self {
            track: None,
            kind: Kind::from(err),
        }
    }
}

pub trait WithTrackContext<T> {
    fn track(self, name: impl Into<String>) -> Result<T>;
}

impl<T, E> WithTrackContext<T> for std::result::Result<T, E>
where
    (String, E): Into<Error>,
    E: Into<Kind>,
{
    fn track(self, name: impl Into<String>) -> std::result::Result<T, Error> {
        self.map_err(|e| {
            let error = match e.into() {
                Kind::Request(e) => Kind::Request(e.without_url()),
                e => e,
            };

            (name.into(), error).into()
        })
    }
}
