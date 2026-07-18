//! Local user preferences for Hen Local Translator.
//!
//! Stored at: ~/.dora/primespeech/app_preferences.json
//! (path kept for backward compatibility with existing installations)

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppPreferences {
    pub app_language: String, // "en" | "zh"
    pub display_name: String,
    pub avatar_letter: String,
    pub last_seen_app_version: Option<String>,
    pub default_voice_id: Option<String>,
    pub default_speed: f64,
    pub default_pitch: f64,
    pub default_volume: f64,
    pub history_retention_days: i64, // -1 = forever
    pub inference_backend: String,   // primespeech_mlx | qwen3_tts_mlx
    pub zero_shot_backend: String,   // primespeech_mlx | qwen3_tts_mlx
    pub training_backend: String,    // option_a | option_b
    pub preferred_output_device: Option<String>,
    pub preferred_input_device: Option<String>,
    pub tts_download_format: String, // "mp3" | "wav"
    pub translation_auto_save_transcript: bool,
    pub translation_periodic_save_transcript: bool,
    pub translation_transcript_file_name: String,
    pub translation_transcript_save_dir: Option<String>,
    pub translation_source_language: String,
    pub translation_target_language: String,
    /// `__system_audio__`, `__default_microphone__`, or a concrete CPAL device name.
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
            app_language: "en".to_string(),
            display_name: "User".to_string(),
            avatar_letter: "U".to_string(),
            last_seen_app_version: None,
            default_voice_id: Some("vivian".to_string()),
            default_speed: 1.0,
            default_pitch: 0.0,
            default_volume: 100.0,
            history_retention_days: -1,
            inference_backend: "qwen3_tts_mlx".to_string(),
            zero_shot_backend: "qwen3_tts_mlx".to_string(),
            training_backend: "option_c".to_string(), // Qwen3 mode (ICL only)
            preferred_output_device: None,
            preferred_input_device: None,
            tts_download_format: "mp3".to_string(),
            translation_auto_save_transcript: false,
            translation_periodic_save_transcript: false,
            translation_transcript_file_name: "transcript.md".to_string(),
            translation_transcript_save_dir: None,
            translation_source_language: "zh".to_string(),
            translation_target_language: "en".to_string(),
            translation_input_device: "__system_audio__".to_string(),
            translation_overlay_fullscreen: true,
            translation_subtitle_split: true,
            translation_overlay_opacity: 1.0,
            translation_font_size_preset: "24".to_string(),
            translation_anchor_position_preset: "50".to_string(),
            experimental_spoken_translation_enabled: false,
            experimental_spoken_translation_output_device: None,
            experimental_spoken_translation_voice: None,
            debug_logs_enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppVersionTransition {
    FirstLaunch,
    SameVersion,
    Updated {
        previous: Option<String>,
        current: String,
    },
}

pub fn record_app_version_transition(
    prefs: &mut AppPreferences,
    current_version: &str,
    had_existing_preferences: bool,
) -> AppVersionTransition {
    let current = current_version.trim().to_string();
    let previous = prefs
        .last_seen_app_version
        .as_deref()
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(str::to_string);

    prefs.last_seen_app_version = Some(current.clone());

    match previous {
        Some(previous) if previous == current => AppVersionTransition::SameVersion,
        Some(previous) => AppVersionTransition::Updated {
            previous: Some(previous),
            current,
        },
        None if had_existing_preferences => AppVersionTransition::Updated {
            previous: None,
            current,
        },
        None => AppVersionTransition::FirstLaunch,
    }
}

fn primespeech_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".dora").join("primespeech")
}

pub fn preferences_path() -> PathBuf {
    primespeech_dir().join("app_preferences.json")
}

pub fn load_preferences() -> AppPreferences {
    let path = preferences_path();
    if !path.exists() {
        return AppPreferences::default();
    }
    match fs::read_to_string(&path) {
        Ok(content) => sanitize_loaded_preferences(
            serde_json::from_str::<AppPreferences>(&content).unwrap_or_default(),
        ),
        Err(_) => AppPreferences::default(),
    }
}

fn sanitize_loaded_preferences(mut prefs: AppPreferences) -> AppPreferences {
    if !matches!(
        prefs.translation_source_language.as_str(),
        "zh" | "en" | "ja" | "fr"
    ) {
        prefs.translation_source_language = "zh".to_string();
    }
    if !matches!(
        prefs.translation_target_language.as_str(),
        "en" | "zh" | "ja" | "fr" | "none"
    ) {
        prefs.translation_target_language = "en".to_string();
    }
    if !matches!(
        prefs.translation_font_size_preset.as_str(),
        "16" | "20" | "24" | "30" | "36" | "44" | "52" | "64" | "80" | "96" | "120" | "160"
    ) {
        prefs.translation_font_size_preset = "24".to_string();
    }
    if !matches!(
        prefs.translation_anchor_position_preset.as_str(),
        "35" | "50" | "70" | "100"
    ) {
        prefs.translation_anchor_position_preset = "50".to_string();
    }
    if !(0.35..=1.0).contains(&prefs.translation_overlay_opacity) {
        prefs.translation_overlay_opacity = 1.0;
    }
    prefs
}

pub fn save_preferences(prefs: &AppPreferences) -> Result<(), String> {
    let dir = primespeech_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create preferences directory {:?}: {}", dir, e))?;
    }
    let json = serde_json::to_string_pretty(prefs)
        .map_err(|e| format!("Failed to serialize preferences: {}", e))?;
    fs::write(preferences_path(), json).map_err(|e| format!("Failed to write preferences: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_version_transition_marks_first_launch_without_reset() {
        let mut prefs = AppPreferences::default();

        let transition = record_app_version_transition(&mut prefs, "0.0.7", false);

        assert_eq!(transition, AppVersionTransition::FirstLaunch);
        assert_eq!(prefs.last_seen_app_version.as_deref(), Some("0.0.7"));
    }

    #[test]
    fn app_version_transition_ignores_same_version() {
        let mut prefs = AppPreferences {
            last_seen_app_version: Some("0.0.7".to_string()),
            ..AppPreferences::default()
        };

        let transition = record_app_version_transition(&mut prefs, "0.0.7", true);

        assert_eq!(transition, AppVersionTransition::SameVersion);
        assert_eq!(prefs.last_seen_app_version.as_deref(), Some("0.0.7"));
    }

    #[test]
    fn app_version_transition_treats_legacy_preferences_as_update() {
        let mut prefs = AppPreferences::default();

        let transition = record_app_version_transition(&mut prefs, "0.0.7", true);

        assert_eq!(
            transition,
            AppVersionTransition::Updated {
                previous: None,
                current: "0.0.7".to_string()
            }
        );
        assert_eq!(prefs.last_seen_app_version.as_deref(), Some("0.0.7"));
    }

    #[test]
    fn app_version_transition_detects_update_once() {
        let mut prefs = AppPreferences {
            last_seen_app_version: Some("0.0.6".to_string()),
            ..AppPreferences::default()
        };

        let transition = record_app_version_transition(&mut prefs, "0.0.7", true);

        assert_eq!(
            transition,
            AppVersionTransition::Updated {
                previous: Some("0.0.6".to_string()),
                current: "0.0.7".to_string()
            }
        );
        assert_eq!(prefs.last_seen_app_version.as_deref(), Some("0.0.7"));

        let transition = record_app_version_transition(&mut prefs, "0.0.7", true);
        assert_eq!(transition, AppVersionTransition::SameVersion);
    }

    #[test]
    fn legacy_preferences_default_tts_download_format_to_mp3() {
        let prefs: AppPreferences = serde_json::from_str(
            r#"{
                "app_language": "zh",
                "display_name": "Alan",
                "avatar_letter": "A"
            }"#,
        )
        .unwrap();

        assert_eq!(prefs.tts_download_format, "mp3");
        assert!(!prefs.experimental_spoken_translation_enabled);
        assert!(prefs
            .experimental_spoken_translation_output_device
            .is_none());
        assert!(prefs.experimental_spoken_translation_voice.is_none());
    }

    #[test]
    fn loaded_preferences_restore_spoken_translation_enabled() {
        let prefs = AppPreferences {
            experimental_spoken_translation_enabled: true,
            experimental_spoken_translation_output_device: Some("Headsets".to_string()),
            experimental_spoken_translation_voice: Some("Samantha".to_string()),
            ..AppPreferences::default()
        };

        let prefs = sanitize_loaded_preferences(prefs);

        assert!(prefs.experimental_spoken_translation_enabled);
        assert_eq!(
            prefs
                .experimental_spoken_translation_output_device
                .as_deref(),
            Some("Headsets")
        );
        assert_eq!(
            prefs.experimental_spoken_translation_voice.as_deref(),
            Some("Samantha")
        );
    }

    #[test]
    fn invalid_translation_preferences_fall_back_to_safe_defaults() {
        let prefs = AppPreferences {
            translation_source_language: "xx".to_string(),
            translation_target_language: "yy".to_string(),
            translation_overlay_opacity: 0.1,
            translation_font_size_preset: "999".to_string(),
            translation_anchor_position_preset: "42".to_string(),
            ..AppPreferences::default()
        };

        let prefs = sanitize_loaded_preferences(prefs);

        assert_eq!(prefs.translation_source_language, "zh");
        assert_eq!(prefs.translation_target_language, "en");
        assert_eq!(prefs.translation_overlay_opacity, 1.0);
        assert_eq!(prefs.translation_font_size_preset, "24");
        assert_eq!(prefs.translation_anchor_position_preset, "50");
    }
}
