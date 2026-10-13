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
}

impl Shortcut {
    /// Shortcut of a key that types `text`, without modifiers.
    pub fn from_text(text: &str) -> Option<Self> {
        match text {
            "f" | "F" => Some(Self::ToggleOutput),
            " " => Some(Self::StopAll),
            digit => digit
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=9).contains(n))
                .map(|n| Self::TriggerColumn(n - 1)),
        }
    }
}

/// Shortcuts listed in the help window: key, then action.
pub const SHORTCUT_HELP: [(&str, &str); 8] = [
    ("1 à 9", "Déclencher la colonne"),
    ("Espace", "Arrêter toutes les couches"),
    ("F", "Ouvrir ou fermer la sortie plein écran"),
    ("Échap", "Fermer la sortie plein écran"),
    ("Cmd + N", "Nouveau projet"),
    ("Cmd + O", "Ouvrir un projet"),
    ("Cmd + S", "Enregistrer"),
    ("Cmd + Maj + S", "Enregistrer sous"),
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
        assert_eq!(Shortcut::from_text("x"), None);
    }
}
