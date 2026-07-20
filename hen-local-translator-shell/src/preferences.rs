use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppPreferences {
    pub app_language: String,
    pub accent_theme: String,
    pub display_name: String,
    pub avatar_letter: String,
    pub last_seen_app_version: Option<String>,
    pub default_voice_id: Option<String>,
    pub default_speed: f64,
    pub default_pitch: f64,
    pub default_volume: f64,
    pub history_retention_days: i64,
    pub inference_backend: String,
    pub zero_shot_backend: String,
    pub training_backend: String,
    pub preferred_output_device: Option<String>,
    pub preferred_input_device: Option<String>,
    pub tts_download_format: String,
    pub translation_auto_save_transcript: bool,
    pub translation_periodic_save_transcript: bool,
    pub translation_transcript_file_name: String,
    pub translation_transcript_save_dir: Option<String>,
    pub translation_source_language: String,
    pub translation_target_language: String,
    pub translation_input_device: String,
    pub translation_overlay_fullscreen: bool,
    pub translation_subtitle_split: bool,
    pub translation_overlay_opacity: f64,
    pub translation_font_size_preset: String,
    pub translation_anchor_position_preset: String,
    pub experimental_spoken_translation_enabled: bool,
    pub experimental_spoken_translation_output_device: Option<String>,
    pub experimental_spoken_translation_voice: Option<String>,
    pub debug_logs_enabled: bool,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            app_language: "zh".into(),
            accent_theme: "neon-blue".into(),
            display_name: "User".into(),
            avatar_letter: "U".into(),
            last_seen_app_version: None,
            default_voice_id: Some("vivian".into()),
            default_speed: 1.0,
            default_pitch: 0.0,
            default_volume: 100.0,
            history_retention_days: -1,
            inference_backend: "qwen3_tts_mlx".into(),
            zero_shot_backend: "qwen3_tts_mlx".into(),
            training_backend: "option_c".into(),
            preferred_output_device: None,
            preferred_input_device: None,
            tts_download_format: "mp3".into(),
            translation_auto_save_transcript: false,
            translation_periodic_save_transcript: false,
            translation_transcript_file_name: "transcript.md".into(),
            translation_transcript_save_dir: None,
            translation_source_language: "zh".into(),
            translation_target_language: "en".into(),
            translation_input_device: "__system_audio__".into(),
            translation_overlay_fullscreen: true,
            translation_subtitle_split: true,
            translation_overlay_opacity: 1.0,
            translation_font_size_preset: "24".into(),
            translation_anchor_position_preset: "50".into(),
            experimental_spoken_translation_enabled: false,
            experimental_spoken_translation_output_device: None,
            experimental_spoken_translation_voice: Some("vivian".into()),
            debug_logs_enabled: false,
        }
    }
}

fn preferences_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Hen Local Translator")
}

pub fn preferences_path() -> PathBuf {
    preferences_dir().join("app_preferences.json")
}

pub fn transcript_dir(preferences: &AppPreferences) -> PathBuf {
    preferences
        .translation_transcript_save_dir
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| preferences_dir().join("transcripts"))
}

pub fn load() -> AppPreferences {
    let mut preferences = fs::read_to_string(preferences_path())
        .ok()
        .and_then(|content| serde_json::from_str::<AppPreferences>(&content).ok())
        .unwrap_or_default();
    sanitize(&mut preferences);
    preferences
}

pub fn save(preferences: &AppPreferences) -> Result<(), String> {
    let directory = preferences_dir();
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create preferences directory: {error}"))?;
    let json = serde_json::to_string_pretty(preferences)
        .map_err(|error| format!("Could not serialize preferences: {error}"))?;
    fs::write(preferences_path(), json)
        .map_err(|error| format!("Could not save preferences: {error}"))
}

fn sanitize(preferences: &mut AppPreferences) {
    if !matches!(
        preferences.translation_source_language.as_str(),
        "zh" | "en" | "ja" | "fr"
    ) {
        preferences.translation_source_language = "zh".into();
    }
    if !matches!(
        preferences.translation_target_language.as_str(),
        "zh" | "en" | "ja" | "fr" | "none"
    ) {
        preferences.translation_target_language = "en".into();
    }
    if !matches!(
        preferences.translation_font_size_preset.as_str(),
        "16" | "20" | "24" | "30" | "36" | "44" | "52" | "64" | "80" | "96" | "120" | "160"
    ) {
        preferences.translation_font_size_preset = "24".into();
    }
    if !matches!(
        preferences.translation_anchor_position_preset.as_str(),
        "35" | "50" | "70" | "100"
    ) {
        preferences.translation_anchor_position_preset = "50".into();
    }
    preferences.translation_overlay_opacity =
        preferences.translation_overlay_opacity.clamp(0.35, 1.0);
    if !matches!(preferences.app_language.as_str(), "zh" | "en") {
        preferences.app_language = "zh".into();
    }
    if !matches!(
        preferences.accent_theme.as_str(),
        "neon-blue" | "neon-orange" | "neon-pink" | "neon-green"
    ) {
        preferences.accent_theme = "neon-blue".into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_invalid_translation_values() {
        let mut preferences = AppPreferences {
            translation_source_language: "xx".into(),
            translation_target_language: "yy".into(),
            translation_font_size_preset: "999".into(),
            translation_anchor_position_preset: "42".into(),
            translation_overlay_opacity: 0.1,
            accent_theme: "unknown".into(),
            ..AppPreferences::default()
        };
        sanitize(&mut preferences);
        assert_eq!(preferences.translation_source_language, "zh");
        assert_eq!(preferences.translation_target_language, "en");
        assert_eq!(preferences.translation_font_size_preset, "24");
        assert_eq!(preferences.translation_anchor_position_preset, "50");
        assert_eq!(preferences.translation_overlay_opacity, 0.35);
        assert_eq!(preferences.accent_theme, "neon-blue");
    }

    #[test]
    fn preserves_supported_accent_themes() {
        for theme in ["neon-blue", "neon-orange", "neon-pink", "neon-green"] {
            let mut preferences = AppPreferences {
                accent_theme: theme.into(),
                ..AppPreferences::default()
            };
            sanitize(&mut preferences);
            assert_eq!(preferences.accent_theme, theme);
        }
    }
}
