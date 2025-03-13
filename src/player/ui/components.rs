//! Various different individual components that
//! appear in lowfi's UI, like the progress bar.

use std::{ops::Deref as _, sync::Arc, time::Duration};

use crossterm::style::Stylize as _;
use unicode_segmentation::UnicodeSegmentation as _;
use strip_ansi_escapes::strip as strip_ansi;

use crate::{player::Player, tracks::Info};

/// Petite fonction utilitaire pour formater un `Duration` en mm:ss.
pub fn format_duration(duration: &Duration) -> String {
    let seconds = duration.as_secs() % 60;
    let minutes = duration.as_secs() / 60;
    format!("{minutes:02}:{seconds:02}")
}

/// Calcule la « vraie » largeur visible d'une chaîne, en enlevant les codes ANSI.
fn visible_width(styled: &str) -> usize {
    match strip_ansi(styled.as_bytes()) {
        Ok(bytes) => bytes.len(),
        Err(_) => styled.len(),
    }
}

/// Crée la barre de progression, ainsi que tous les espacements nécessaires.
pub fn progress_bar(player: &Player, current: Option<&Arc<Info>>, width: usize) -> String {
    let mut duration = Duration::new(0, 0);
    let elapsed = if current.is_some() {
        player.sink.get_pos()
    } else {
        Duration::new(0, 0)
    };

    let mut filled = 0;
    if let Some(current) = current {
        if let Some(x) = current.duration {
            duration = x;
            let ratio = elapsed.as_secs() as f32 / duration.as_secs() as f32;
            filled = (ratio * width as f32).round() as usize;
        }
    };

    format!(
        " [{}{}] {}/{} ",
        "/".repeat(filled),
        " ".repeat(width.saturating_sub(filled)),
        format_duration(&elapsed),
        format_duration(&duration),
    )
}

/// Crée la barre de volume, avec l'affichage du pourcentage et le padding nécessaire.
pub fn audio_bar(volume: f32, percentage: &str, width: usize) -> String {
    let audio = (volume * width as f32).round() as usize;

    format!(
        " volume: [{}{}] {}{} ",
        "/".repeat(audio),
        " ".repeat(width.saturating_sub(audio)),
        " ".repeat(4usize.saturating_sub(percentage.len())),
        percentage,
    )
}

/// Représente l'état de la barre d'action en haut de l'UI.
enum ActionBar {
    /// L'application est en pause.
    Paused(Info),
    /// L'application est en lecture.
    Playing(Info),
    /// L'application est en chargement.
    Loading,
}

impl ActionBar {
    /// Retourne (la_chaîne_à_afficher, longueur_visible_de_cette_chaîne).
    fn format(&self) -> (String, usize) {
        let (word, subject) = match self {
            Self::Playing(x) => ("playing", Some((x.name.clone(), x.width))),
            Self::Paused(x) => ("paused", Some((x.name.clone(), x.width))),
            Self::Loading => ("loading", None),
        };

        subject.map_or_else(
            || {
                // Cas : "loading"
                (word.to_owned(), visible_width(word))
            },
            |(subject, raw_len)| {
                // Cas : "playing [titre]" ou "paused [titre]"
                // `raw_len` = longueur "réelle" du titre sans style (stockée dans Info)
                let styled = format!("{} {}", word, subject.bold());
                // On calcule la longueur comme : longueur(word) + 1 (espace) + raw_len
                (styled, visible_width(word) + 1 + raw_len)
            },
        )
    }
}

/// Crée la barre d'action du haut, avec le nom du morceau et son statut.
/// Gère aussi l'espace pour atteindre `width`.
pub fn action(player: &Player, current: Option<&Arc<Info>>, width: usize) -> String {
    let (main, len) = current
        .map_or(ActionBar::Loading, |info| {
            let info = info.deref().clone();
            if player.sink.is_paused() {
                ActionBar::Paused(info)
            } else {
                ActionBar::Playing(info)
            }
        })
        .format();

    if len > width {
        // On tronque si la longueur dépasse la largeur voulue
        let chopped: String = main.graphemes(true).take(width + 1).collect();
        format!("{chopped}...")
    } else {
        // Sinon on complète avec des espaces
        format!("{}{}", main, " ".repeat(width.saturating_sub(len)))
    }
}

/// Crée la barre de contrôles du bas, et espace correctement chaque contrôle.
pub fn controls(width: usize) -> String {
    // Les textes bruts
    let controls_raw = [
        ["[s]", "kip"],
        ["[b]", "ack"],
        ["[p]", "ause"],
        ["[q]", "uit"],
    ];

    // Version stylisée (avec codes ANSI pour le gras)
    let controls_styled: Vec<String> = controls_raw
        .iter()
        .map(|x| format!("{}{}", x[0].bold(), x[1]))
        .collect();

    // Version « brute » (pas de gras) pour mesurer la longueur réelle
    let controls_plain: Vec<String> = controls_raw
        .iter()
        .map(|x| format!("{}{}", x[0], x[1]))
        .collect();

    // On joint les contrôles avec 5 espaces entre chacun
    let with_spacing_styled = controls_styled.join("     ");
    let with_spacing_plain  = controls_plain.join("     ");

    // On calcule la longueur visible, +1 pour la marge de gauche
    let total_len = visible_width(&with_spacing_plain) + 1;
    let padding   = width.saturating_sub(total_len);

    // On affiche la version stylisée, en paddant en fonction de la longueur réelle
    format!(" {}{}", with_spacing_styled, " ".repeat(padding))
}
