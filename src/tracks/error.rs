#[derive(Debug, thiserror::Error)]
pub enum Kind {
    #[error("timeout")]
    Timeout,

    #[error("unable to decode: {0}")]
    Decode(#[from] rodio::decoder::DecoderError),

    #[error("invalid name")]
    InvalidName,

    #[error("invalid file path")]
    InvalidPath,

    #[error("unable to read file: {0}")]
    File(#[from] std::io::Error),

    #[error("unable to fetch data: {0}")]
    Request(#[from] reqwest::Error),
}

#[derive(Debug, thiserror::Error)]
#[error("{kind}\ntrack: {track}")]
pub struct Error {
    pub track: String,

    #[source]
    pub kind: Kind,
}

impl Error {
    pub const fn is_timeout(&self) -> bool {
        matches!(self.kind, Kind::Timeout)
    }
}

impl<T, E> From<(T, E)> for Error
where
    T: Into<String>,
    Kind: From<E>,
{
    fn from((track, err): (T, E)) -> Self {
        Error {
            track: track.into(),
            kind: Kind::from(err),
        }
    }
}

pub trait Context<T> {
    fn track(self, name: impl Into<String>) -> Result<T, Error>;
}

impl<T, E> Context<T> for Result<T, E>
where
    (String, E): Into<Error>,
{
    fn track(self, name: impl Into<String>) -> Result<T, Error> {
        self.map_err(|e| (name.into(), e).into())
    }
}
