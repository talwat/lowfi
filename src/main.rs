//! An extremely simple lofi player.

#![warn(clippy::all, clippy::restriction, clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::single_call_fn,
    clippy::struct_excessive_bools,
    clippy::implicit_return,
    clippy::question_mark_used,
    clippy::shadow_reuse,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::std_instead_of_core,
    clippy::print_stdout,
    clippy::float_arithmetic,
    clippy::integer_division_remainder_used,
    clippy::used_underscore_binding,
    clippy::print_stderr,
    clippy::semicolon_outside_block,
    clippy::non_send_fields_in_send_ty,
    clippy::non_ascii_literal,
    clippy::let_underscore_untyped,
    clippy::let_underscore_must_use,
    clippy::shadow_unrelated,
    clippy::std_instead_of_alloc,
    clippy::partial_pub_fields,
    clippy::unseparated_literal_suffix,
    clippy::self_named_module_files,
    // TODO: Disallow these lints later.
    clippy::unwrap_used,
    clippy::pattern_type_mismatch,
    clippy::tuple_array_conversions,
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::wildcard_enum_match_arm,
    clippy::integer_division,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
)]

use clap::{Parser, Subcommand};

mod play;
mod player;
mod tracks;

#[allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
mod scrape;

/// An extremely simple lofi player.
#[derive(Parser)]
#[command(about, version)]
struct Args {
    /// Whether to use an alternate terminal screen.
    #[clap(long, short)]
    alternate: bool,

    /// Whether to hide the bottom control bar.
    #[clap(long, short)]
    minimalist: bool,

    /// Whether to start lowfi paused.
    #[clap(long, short)]
    paused: bool,

    /// Whether to include ALSA & other logs.
    #[clap(long, short)]
    debug: bool,

    /// The width of the player, from 0 to 32.
    #[clap(long, short, default_value_t = 3)]
    width: usize,

    /// This is either a path, or a name of a file in the data directory (eg. ~/.local/share/lowfi).
    #[clap(long, short, alias = "list", short_alias = 'l')]
    tracks: Option<String>,

    /// The command that was ran.
    /// This is [None] if no command was specified.
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Defines all of the extra commands lowfi can run.
#[derive(Subcommand)]
enum Commands {
    /// Scrapes the lofi girl website file server for files.
    Scrape {
        /// The file extension to search for, defaults to mp3.
        #[clap(long, short, default_value = "mp3")]
        extension: String,

        /// Whether to include the full HTTP URL or just the distinguishing part.
        #[clap(long, short)]
        include_full: bool,
    },
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Args::parse();

    if let Some(command) = cli.command {
        match command {
            Commands::Scrape {
                extension,
                include_full,
            } => scrape::scrape(extension, include_full).await,
        }
    } else {
        play::play(cli).await
    }
}
