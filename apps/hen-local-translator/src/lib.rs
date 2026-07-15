//! Hen Local Translator App - live speech translation.

/// User-facing app version shown in the About panel and release artifacts.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

// Local modules
pub mod app_preferences;
pub mod app_update;
pub mod audio_player; // Legacy audio preview support retained until the screen is split.
pub mod bounded_drop_down;
pub mod dora_integration;
pub mod i18n;

pub mod screen;

pub mod task_persistence;
pub mod training_executor;
pub mod training_manager;
pub mod transcript_export;
pub mod tts_emotion;
pub mod tts_history;
pub mod voice_clone_modal;
pub mod voice_data;
pub mod voice_persistence;
pub mod voice_selector;

// Re-export shared components from moxin-ui
pub use moxin_ui::log_bridge;
pub use moxin_ui::system_monitor;
pub use moxin_ui::widgets::moxin_hero::{self, ConnectionStatus, MoxinHero, MoxinHeroAction};

pub use screen::TTSScreenRef;
pub use screen::TTSScreenWidgetRefExt;

use makepad_widgets::Cx;
use moxin_widgets::{AppInfo, MoxinApp};

/// Hen Local Translator app descriptor.
pub struct HenLocalTranslatorApp;

impl MoxinApp for HenLocalTranslatorApp {
    fn info() -> AppInfo {
        AppInfo {
            name: "Live Translation",
            id: "hen-local-translator",
            description: "Real-time speech translation with floating subtitles",
            ..Default::default()
        }
    }

    fn live_design(cx: &mut Cx) {
        // Note: Shared components (moxin_hero, system_monitor, log_bridge) are already
        // registered by moxin_ui::live_design(cx) in the shell, so we only register
        // app-specific components here.

        voice_selector::live_design(cx);
        voice_clone_modal::live_design(cx);
        bounded_drop_down::live_design(cx);
        screen::live_design(cx);
    }
}

/// Initialize the translator app - must be called before using the app
pub fn init() {
    // Initialize i18n translations
    i18n::init_translations();
}

/// Register translator widgets with Makepad.
pub fn live_design(cx: &mut Cx) {
    HenLocalTranslatorApp::live_design(cx);
}
