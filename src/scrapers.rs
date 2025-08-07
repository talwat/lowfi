use clap::ValueEnum;

pub mod chillhop;
pub mod lofigirl;

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub enum Sources {
    Lofigirl,
    Chillhop,
}
