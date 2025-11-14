use tokio::sync::mpsc;

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
pub enum Kind {
    #[error("unable to fetch data: {0}")]
    Request(#[from] reqwest::Error),

    #[error("C string null error: {0}")]
    FfiNull(#[from] std::ffi::NulError),

    #[error("audio playing error: {0}")]
    Rodio(#[from] rodio::StreamError),

    #[error("couldn't send internal message: {0}")]
    Send(#[from] mpsc::error::SendError<crate::Message>),
}

#[derive(Debug, Default)]
pub struct Context {
    track: Option<String>,
}

impl std::fmt::Display for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(track) = &self.track {
            write!(f, " ")?;
            write!(f, "(track: {track})")?;
        }
        
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{kind}{context}")]
pub struct Error {
    pub context: Context,

    #[source]
    pub kind: Kind,
}

impl<T, E> From<(T, E)> for Error
where
    T: Into<String>,
    Kind: From<E>,
{
    fn from((track, err): (T, E)) -> Self {
        Self {
            context: Context { track: Some(track.into()) },
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
            context: Context::default(),
            kind: Kind::from(err),
        }
    }
}

pub trait WithContextExt<T> {
    fn context(self, name: impl Into<String>) -> std::result::Result<T, Error>;
}

impl<T, E> WithContextExt<T> for std::result::Result<T, E>
where
    (String, E): Into<Error>,
    E: Into<Kind>,
{
    fn context(self, name: impl Into<String>) -> std::result::Result<T, Error> {
        self.map_err(|e| {
            let error = match e.into() {
                Kind::Request(error) => Kind::Request(error.without_url()),
                kind => kind,
            };

            (name.into(), error).into()
        })
    }
}