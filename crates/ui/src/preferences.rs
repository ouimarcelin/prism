//! Preferences kept between sessions: recent projects, output screen,
//! crossfade time, audio input.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Recent projects remembered.
pub const MAX_RECENT: usize = 8;

/// Preferences of the user.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// Projects opened or saved, most recent first.
    pub recent_projects: Vec<PathBuf>,
    /// Names of the screens last used for the output.
    pub output_screens: Vec<String>,
    /// Duration of the automatic crossfade, in seconds.
    pub crossfade_seconds: f32,
    /// Duration of the automatic crossfade, in beats.
    pub crossfade_beats: f32,
    /// Whether the automatic crossfade lasts [`Preferences::crossfade_beats`]
    /// beats at the tempo of the composition rather than a time in seconds.
    pub crossfade_on_beats: bool,
    /// Whether the sound of an input is measured, for settings that follow
    /// it.
    pub audio_input: bool,
    /// Name of the audio input; the default input of the system when none.
    pub audio_device: Option<String>,
    /// Gain applied to the sound before measuring, in decibels.
    pub audio_gain_db: f32,
    /// Whether the sound of the clips is played.
    pub clip_sound: bool,
    /// Volume of the sound of the clips, from 0 to 1.
    pub clip_sound_volume: f32,
    /// MIDI inputs not listened to; every other input is.
    pub midi_disabled: Vec<String>,
    /// Whether MIDI controllers are sent the state of their controls
    /// (lit pads, faders).
    pub midi_feedback: bool,
    /// Whether the tempo follows the MIDI clock received on the inputs.
    pub midi_clock: bool,
    /// Whether DMX from a lighting console is listened to.
    pub dmx_input: bool,
    /// Protocol of the DMX listened to.
    pub dmx_input_protocol: prism_core::DmxProtocol,
    /// Whether the program image is published over Syphon, for other
    /// applications of the Mac.
    pub syphon_output: bool,
    /// Whether the program image is published as an NDI® source of the
    /// network.
    pub ndi_output: bool,
    /// Whether Prism listens for OSC.
    pub osc_input: bool,
    /// UDP port Prism listens on for OSC.
    pub osc_port: u16,
    /// Whether Prism sends its values over OSC.
    pub osc_output: bool,
    /// Where values are sent, as `IP:port`.
    pub osc_target: String,
    /// Whether the network API (HTTP and WebSocket) is on.
    pub api_enabled: bool,
    /// TCP port of the network API.
    pub api_port: u16,
    /// Code clients give to use the network API; made when the API is
    /// first turned on.
    pub api_code: String,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            recent_projects: Vec::new(),
            output_screens: Vec::new(),
            crossfade_seconds: 2.0,
            crossfade_beats: 4.0,
            crossfade_on_beats: false,
            audio_input: false,
            audio_device: None,
            audio_gain_db: 0.0,
            clip_sound: true,
            clip_sound_volume: 1.0,
            midi_disabled: Vec::new(),
            midi_feedback: true,
            midi_clock: false,
            dmx_input: false,
            dmx_input_protocol: prism_core::DmxProtocol::ArtNet,
            syphon_output: false,
            ndi_output: false,
            osc_input: false,
            osc_port: 7000,
            osc_output: false,
            osc_target: "127.0.0.1:7001".to_owned(),
            api_enabled: false,
            api_port: prism_control::DEFAULT_API_PORT,
            api_code: String::new(),
        }
    }
}

impl Preferences {
    /// Where preferences live on this system, if a home folder is known.
    pub fn default_path() -> Option<PathBuf> {
        let folder = if cfg!(target_os = "macos") {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join("Library/Application Support/Prism"))
        } else if cfg!(target_os = "windows") {
            std::env::var_os("APPDATA").map(|appdata| PathBuf::from(appdata).join("Prism"))
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"))
                })
                .map(|config| config.join("prism"))
        };
        folder.map(|folder| folder.join("preferences.json"))
    }

    /// Reads preferences; a missing or unreadable file gives the defaults.
    pub fn load(path: &Path) -> Self {
        let Some(value) = std::fs::read_to_string(path)
            .ok()
            .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
        else {
            return Self::default();
        };
        // Earlier versions kept a single output screen.
        let legacy_screen = value
            .get("output_screen")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let mut preferences: Self = serde_json::from_value(value).unwrap_or_default();
        if let Some(screen) = legacy_screen
            && preferences.output_screens.is_empty()
        {
            preferences.output_screens.push(screen);
        }
        preferences
    }

    /// Writes preferences, creating their folder if needed.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Duration of the automatic crossfade at `bpm` beats per minute, in
    /// seconds.
    pub fn crossfade_duration(&self, bpm: f64) -> f32 {
        if self.crossfade_on_beats {
            (f64::from(self.crossfade_beats) * 60.0 / bpm) as f32
        } else {
            self.crossfade_seconds
        }
    }

    /// Puts `path` first in the recent projects.
    pub fn add_recent(&mut self, path: &Path) {
        self.recent_projects.retain(|recent| recent != path);
        self.recent_projects.insert(0, path.to_path_buf());
        self.recent_projects.truncate(MAX_RECENT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_projects_are_unique_and_bounded() {
        let mut preferences = Preferences::default();
        for index in 0..10 {
            preferences.add_recent(Path::new(&format!("/shows/{index}.prism")));
        }
        preferences.add_recent(Path::new("/shows/5.prism"));
        assert_eq!(preferences.recent_projects.len(), MAX_RECENT);
        assert_eq!(preferences.recent_projects[0], Path::new("/shows/5.prism"));
        assert_eq!(
            preferences
                .recent_projects
                .iter()
                .filter(|path| path.ends_with("5.prism"))
                .count(),
            1
        );
    }

    #[test]
    fn saved_preferences_load_back_and_bad_files_give_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("preferences.json");
        assert_eq!(Preferences::load(&path), Preferences::default());

        let mut preferences = Preferences {
            output_screens: vec!["Projecteur".into(), "Écran LED".into()],
            crossfade_seconds: 4.5,
            ..Preferences::default()
        };
        preferences.add_recent(Path::new("/shows/a.prism"));
        preferences.save(&path).unwrap();
        assert_eq!(Preferences::load(&path), preferences);

        std::fs::write(&path, b"{ not json").unwrap();
        assert_eq!(Preferences::load(&path), Preferences::default());
        // Unknown and missing fields are tolerated.
        std::fs::write(&path, br#"{"crossfade_seconds": 1.0, "future": true}"#).unwrap();
        let loaded = Preferences::load(&path);
        assert_eq!(loaded.crossfade_seconds, 1.0);
        assert!(loaded.recent_projects.is_empty());
    }

    #[test]
    fn the_single_output_screen_of_earlier_versions_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preferences.json");
        std::fs::write(&path, br#"{"output_screen": "Projecteur"}"#).unwrap();
        let loaded = Preferences::load(&path);
        assert_eq!(loaded.output_screens, ["Projecteur"]);
        loaded.save(&path).unwrap();
        let json = std::fs::read_to_string(&path).unwrap();
        assert!(!json.contains("\"output_screen\""));
        assert_eq!(Preferences::load(&path), loaded);
    }

    #[test]
    fn crossfades_last_seconds_or_beats() {
        let mut preferences = Preferences::default();
        assert_eq!(preferences.crossfade_duration(120.0), 2.0);
        preferences.crossfade_on_beats = true;
        preferences.crossfade_beats = 8.0;
        assert_eq!(preferences.crossfade_duration(120.0), 4.0);
        assert_eq!(preferences.crossfade_duration(60.0), 8.0);
    }

    #[test]
    fn preferences_live_in_the_user_folder() {
        let path = Preferences::default_path();
        assert!(path.is_none_or(|path| path.ends_with("preferences.json")));
    }
}
