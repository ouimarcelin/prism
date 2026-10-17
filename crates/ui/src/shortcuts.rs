//! Keyboard shortcuts, shared by the control window and the output window.

/// An action bound to a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    /// Starts every clip of the column, numbered from 0.
    TriggerColumn(usize),
    /// Stops every layer.
    StopAll,
    /// Opens or closes the full-screen output.
    ToggleOutput,
    /// Closes the full-screen output.
    CloseOutput,
    /// Quits, after asking about unsaved changes.
    Quit,
    /// One tap of the tap tempo.
    TapTempo,
    /// Starts a bar now.
    Resync,
}

impl Shortcut {
    /// Shortcut of a key that types `text`, without modifiers.
    pub fn from_text(text: &str) -> Option<Self> {
        match text {
            "f" | "F" => Some(Self::ToggleOutput),
            "t" | "T" => Some(Self::TapTempo),
            "r" | "R" => Some(Self::Resync),
            " " => Some(Self::StopAll),
            digit => digit
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=9).contains(n))
                .map(|n| Self::TriggerColumn(n - 1)),
        }
    }
}

/// Shortcuts listed in the help window: key, then action. Any key without
/// modifier can also be learned (Contrôle > Apprentissage MIDI), and then
/// does what it was learned for instead.
pub const SHORTCUT_HELP: [(&str, &str); 12] = [
    ("1 à 9", "Déclencher la colonne"),
    ("Espace", "Arrêter toutes les couches"),
    ("T", "Tap tempo : frapper en rythme"),
    ("R", "Resync : la mesure commence maintenant"),
    ("F", "Ouvrir ou fermer la sortie plein écran"),
    ("Échap", "Fermer la sortie plein écran"),
    ("Cmd + N", "Nouveau projet"),
    ("Cmd + O", "Ouvrir un projet"),
    ("Cmd + S", "Enregistrer"),
    ("Cmd + Maj + S", "Enregistrer sous"),
    ("Cmd + Q", "Quitter"),
    (
        "Toute touche",
        "Apprise : Contrôle > Apprentissage MIDI, cliquer une commande, appuyer",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_map_to_shortcuts() {
        assert_eq!(Shortcut::from_text("1"), Some(Shortcut::TriggerColumn(0)));
        assert_eq!(Shortcut::from_text("9"), Some(Shortcut::TriggerColumn(8)));
        assert_eq!(Shortcut::from_text("0"), None);
        assert_eq!(Shortcut::from_text("f"), Some(Shortcut::ToggleOutput));
        assert_eq!(Shortcut::from_text(" "), Some(Shortcut::StopAll));
        assert_eq!(Shortcut::from_text("t"), Some(Shortcut::TapTempo));
        assert_eq!(Shortcut::from_text("R"), Some(Shortcut::Resync));
        assert_eq!(Shortcut::from_text("x"), None);
    }
}
