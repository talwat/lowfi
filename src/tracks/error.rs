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
}

#[derive(Debug, thiserror::Error)]
#[error("{kind} (track: {track})")]
pub struct Error {
    pub track: String,

    #[source]
    pub kind: Kind,
}

impl Error {
    pub fn is_timeout(&self) -> bool {
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
    E: Into<Kind>,
{
    fn track(self, name: impl Into<String>) -> Result<T, Error> {
        self.map_err(|e| {
            let error = match e.into() {
                Kind::Request(e) => Kind::Request(e.without_url()),
                e => e,
            };

            (name.into(), error).into()
        })
    }
}
