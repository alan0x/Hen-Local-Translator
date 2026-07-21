use crate::{
    account::{AccountManager, AccountStatus},
    apple_speech::{self, AppleSpeech},
    dataflow::{render_translation_dataflow, RenderOptions},
    preferences::{self, AppPreferences},
    runtime::{RuntimeEvent, TranslationRuntime},
    usage::{self, UsageSnapshot, UsageTracker},
    Args,
};
use cpal::traits::{DeviceTrait, HostTrait};
use moxin_dora_bridge::{data::SentenceUnit, AudioSource, TranslationUpdate};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Arc,
    thread,
    time::Duration,
};
use tauri::{
    Emitter, LogicalSize, Manager, Size, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};
use tauri_plugin_deep_link::DeepLinkExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationSettings {
    app_language: String,
    accent_theme: String,
    source_language: String,
    target_language: String,
    input_device: String,
    overlay_fullscreen: bool,
    subtitle_split: bool,
    overlay_opacity: f64,
    font_size_preset: String,
    anchor_position_preset: String,
    spoken_translation_enabled: bool,
    spoken_translation_voice: Option<String>,
    auto_save_transcript: bool,
    periodic_save_transcript: bool,
    transcript_file_name: String,
    transcript_save_dir: Option<String>,
}

impl From<&AppPreferences> for TranslationSettings {
    fn from(preferences: &AppPreferences) -> Self {
        Self {
            app_language: preferences.app_language.clone(),
            accent_theme: preferences.accent_theme.clone(),
            source_language: preferences.translation_source_language.clone(),
            target_language: preferences.translation_target_language.clone(),
            input_device: preferences.translation_input_device.clone(),
            overlay_fullscreen: preferences.translation_overlay_fullscreen,
            subtitle_split: preferences.translation_subtitle_split,
            overlay_opacity: preferences.translation_overlay_opacity,
            font_size_preset: preferences.translation_font_size_preset.clone(),
            anchor_position_preset: preferences.translation_anchor_position_preset.clone(),
            spoken_translation_enabled: preferences.experimental_spoken_translation_enabled,
            spoken_translation_voice: preferences.experimental_spoken_translation_voice.clone(),
            auto_save_transcript: preferences.translation_auto_save_transcript,
            periodic_save_transcript: preferences.translation_periodic_save_transcript,
            transcript_file_name: preferences.translation_transcript_file_name.clone(),
            transcript_save_dir: preferences.translation_transcript_save_dir.clone(),
        }
    }
}

impl TranslationSettings {
    fn apply_to(&self, preferences: &mut AppPreferences) {
        preferences.app_language = self.app_language.clone();
        preferences.accent_theme = match self.accent_theme.as_str() {
            "neon-orange" | "neon-pink" | "neon-green" => self.accent_theme.clone(),
            _ => "neon-blue".into(),
        };
        preferences.translation_source_language = self.source_language.clone();
        preferences.translation_target_language = self.target_language.clone();
        preferences.translation_input_device = self.input_device.clone();
        preferences.translation_overlay_fullscreen = self.overlay_fullscreen;
        preferences.translation_subtitle_split = self.subtitle_split;
        preferences.translation_overlay_opacity = self.overlay_opacity.clamp(0.35, 1.0);
        preferences.translation_font_size_preset = self.font_size_preset.clone();
        preferences.translation_anchor_position_preset = self.anchor_position_preset.clone();
        preferences.experimental_spoken_translation_enabled = self.spoken_translation_enabled;
        preferences.experimental_spoken_translation_voice = self.spoken_translation_voice.clone();
        preferences.translation_auto_save_transcript = self.auto_save_transcript;
        preferences.translation_periodic_save_transcript = self.periodic_save_transcript;
        preferences.translation_transcript_file_name = self.transcript_file_name.clone();
        preferences.translation_transcript_save_dir = self.transcript_save_dir.clone();
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsPayload {
    settings: TranslationSettings,
    input_devices: Vec<String>,
    subtitle_preview_visible: bool,
    running: bool,
    runtime_status: String,
    runtime_message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeState {
    running: bool,
    status: String,
    message: String,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            running: false,
            status: "idle".into(),
            message: "Local AI is ready".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Sentence {
    source_text: String,
    translation: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlayState {
    active: bool,
    status: String,
    source_language: String,
    target_language: String,
    subtitle_split: bool,
    font_size: u32,
    anchor_position: u32,
    accent_theme: String,
    history: Vec<Sentence>,
    pending_source_text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelStatus {
    core_ready: bool,
    downloading: bool,
    component: Option<String>,
    progress: f64,
    title: String,
    detail: String,
    core_download_bytes: u64,
}

struct AppState {
    preferences: Mutex<AppPreferences>,
    runtime: TranslationRuntime,
    runtime_state: Mutex<RuntimeState>,
    resource_dir: Option<PathBuf>,
    voice_preview_process: Mutex<Option<Child>>,
    apple_speech: AppleSpeech,
    spoken_completed_count: Mutex<u64>,
    model_download_process: Mutex<Option<(String, Child)>>,
    subtitle_preview_visible: Mutex<bool>,
    usage: Arc<UsageTracker>,
    account: Arc<AccountManager>,
}

impl AppState {
    fn new(resource_dir: Option<PathBuf>) -> Self {
        let preferences = preferences::load();
        let settings = TranslationSettings::from(&preferences);
        let runtime = TranslationRuntime::new();
        let usage = UsageTracker::load(preferences::preferences_dir().join("usage.json"));
        let account = Arc::new(AccountManager::new());
        let state = Self {
            preferences: Mutex::new(preferences),
            runtime,
            runtime_state: Mutex::new(RuntimeState::default()),
            resource_dir,
            voice_preview_process: Mutex::new(None),
            apple_speech: AppleSpeech::new(),
            spoken_completed_count: Mutex::new(0),
            model_download_process: Mutex::new(None),
            subtitle_preview_visible: Mutex::new(true),
            usage,
            account,
        };
        state.sync_shared_state();
        state.show_subtitle_preview(&settings);
        state
    }

    fn sync_shared_state(&self) {
        let preferences = self.preferences.lock().clone();
        let shared = self.runtime.shared_state();
        shared.translation_window_visible.set(true);
        shared
            .translation_locale_en
            .set(preferences.app_language == "en");
        shared.translation_lang_pair.set((
            preferences.translation_source_language.clone(),
            preferences.translation_target_language.clone(),
        ));
        shared
            .translation_overlay_fullscreen
            .set(preferences.translation_overlay_fullscreen);
        shared
            .translation_subtitle_split
            .set(preferences.translation_subtitle_split);
        shared
            .translation_overlay_opacity
            .set(preferences.translation_overlay_opacity);
        shared
            .translation_font_size_preset
            .set(preferences.translation_font_size_preset.clone());
        shared.translation_footer_font_size_preset.set("12".into());
        shared
            .translation_anchor_position_preset
            .set(preferences.translation_anchor_position_preset.clone());
        if preferences.translation_input_device == "__system_audio__" {
            shared
                .translation_audio_source
                .set(AudioSource::SystemAudio);
            shared.translation_input_device.set(None);
        } else {
            shared.translation_audio_source.set(AudioSource::Microphone);
            let device = (preferences.translation_input_device != "__default_microphone__")
                .then(|| preferences.translation_input_device.clone());
            shared.translation_input_device.set(device);
        }
    }

    fn sample_text(language: &str, index: usize) -> &'static str {
        match (language, index) {
            ("zh", 0) => "这是一段用于调整字幕大小和布局的测试内容。",
            ("zh", _) => "请确认每句话都清晰、易读，并适合现场屏幕。",
            ("ja", 0) => "これは字幕のサイズとレイアウトを調整するためのテストです。",
            ("ja", _) => "各文が読みやすく、会場の画面に適しているか確認してください。",
            ("fr", 0) => {
                "Ceci est un texte de test pour régler la taille et la disposition des sous-titres."
            }
            ("fr", _) => {
                "Vérifiez que chaque phrase est claire et lisible sur l’écran de la salle."
            }
            (_, 0) => "This sample helps you adjust subtitle size and layout.",
            (_, _) => {
                "Check that every sentence is clear, readable, and suitable for the venue screen."
            }
        }
    }

    fn show_subtitle_preview(&self, settings: &TranslationSettings) {
        let history = (0..2)
            .map(|index| SentenceUnit {
                source_text: Self::sample_text(&settings.source_language, index).to_string(),
                translation: if settings.target_language == "none" {
                    String::new()
                } else {
                    Self::sample_text(&settings.target_language, index).to_string()
                },
            })
            .collect();
        self.runtime
            .shared_state()
            .translation
            .set(Some(TranslationUpdate {
                history,
                pending_source_text: String::new(),
                completed_count: 2,
            }));
        *self.subtitle_preview_visible.lock() = true;
    }

    fn clear_subtitle_preview(&self) {
        self.runtime.shared_state().translation.set(None);
        *self.subtitle_preview_visible.lock() = false;
    }

    fn overlay_state(&self) -> OverlayState {
        let shared = self.runtime.shared_state();
        let active = shared.translation_overlay_active.read();
        let bridge_status = shared.status.read();
        let bridges_ready = bridge_status
            .active_bridges
            .iter()
            .any(|bridge| bridge == "moxin-mic-input")
            && bridge_status
                .active_bridges
                .iter()
                .any(|bridge| bridge == "moxin-translation-listener");
        let status = if !active {
            "idle"
        } else if bridges_ready {
            "listening"
        } else {
            "warming"
        }
        .to_string();
        if shared.translation_overlay_status.read() != status {
            shared.translation_overlay_status.set(status.clone());
        }

        let (source_language, target_language) = shared.translation_lang_pair.read();
        let update = shared.translation.read();
        let (history, pending_source_text) = update
            .map(|update| {
                (
                    update
                        .history
                        .into_iter()
                        .map(|sentence| Sentence {
                            source_text: sentence.source_text,
                            translation: sentence.translation,
                        })
                        .collect(),
                    update.pending_source_text,
                )
            })
            .unwrap_or_default();

        let accent_theme = self.preferences.lock().accent_theme.clone();
        OverlayState {
            active,
            status,
            source_language,
            target_language,
            subtitle_split: shared.translation_subtitle_split.read(),
            font_size: shared
                .translation_font_size_preset
                .read()
                .parse()
                .unwrap_or(24),
            anchor_position: shared
                .translation_anchor_position_preset
                .read()
                .parse()
                .unwrap_or(50),
            accent_theme,
            history,
            pending_source_text,
        }
    }

    fn poll_runtime_events(&self) -> RuntimeState {
        let events = self.runtime.poll_events();
        let mut state = self.runtime_state.lock();
        for event in events {
            match event {
                RuntimeEvent::Started(id) => {
                    state.running = true;
                    state.status = "listening".into();
                    state.message = format!("Local translation connected · {id}");
                }
                RuntimeEvent::Stopped => {
                    self.apple_speech.stop();
                    if state.running {
                        if let Err(error) = self.usage.stop() {
                            log::error!("Could not stop usage timer: {error}");
                        }
                    }
                    state.running = false;
                    state.status = "idle".into();
                    state.message = "Translation stopped".into();
                }
                RuntimeEvent::Error(message) => {
                    self.apple_speech.stop();
                    if state.running {
                        if let Err(error) = self.usage.stop() {
                            log::error!("Could not stop usage timer after runtime error: {error}");
                        }
                    }
                    state.running = false;
                    state.status = "error".into();
                    state.message = message;
                }
            }
        }
        state.clone()
    }

    fn queue_completed_translations_for_speech(&self) {
        if !self.runtime_state.lock().running {
            return;
        }
        let Some(update) = self.runtime.shared_state().translation.read() else {
            return;
        };
        let mut spoken = self.spoken_completed_count.lock();
        if update.completed_count < *spoken {
            *spoken = 0;
        }
        let new_sentences = update.completed_count.saturating_sub(*spoken) as usize;
        for sentence in update
            .history
            .iter()
            .skip(update.history.len().saturating_sub(new_sentences))
        {
            self.apple_speech.speak(sentence.translation.clone());
        }
        *spoken = update.completed_count;
    }

    fn save_transcript_if_needed(&self) -> Result<(), String> {
        let preferences = self.preferences.lock().clone();
        if !preferences.translation_auto_save_transcript
            && !preferences.translation_periodic_save_transcript
        {
            return Ok(());
        }
        let Some(update) = self.runtime.shared_state().translation.read() else {
            return Ok(());
        };
        if update.history.is_empty() {
            return Ok(());
        }

        let directory = preferences::transcript_dir(&preferences);
        fs::create_dir_all(&directory)
            .map_err(|error| format!("Could not create transcript directory: {error}"))?;
        let filename = if preferences
            .translation_transcript_file_name
            .trim()
            .is_empty()
        {
            "transcript.md"
        } else {
            preferences.translation_transcript_file_name.trim()
        };
        let mut markdown = String::from("# Translation transcript\n\n");
        for sentence in update.history {
            markdown.push_str(&format!(
                "**Source**\n\n{}\n\n**Translation**\n\n{}\n\n---\n\n",
                sentence.source_text, sentence.translation
            ));
        }
        fs::write(directory.join(filename), markdown)
            .map_err(|error| format!("Could not save transcript: {error}"))
    }

    fn model_status(&self) -> ModelStatus {
        let mut process = self.model_download_process.lock();
        let mut active_component = None;
        if let Some((component, child)) = process.as_mut() {
            match child.try_wait() {
                Ok(None) => active_component = Some(component.clone()),
                Ok(Some(_)) | Err(_) => *process = None,
            }
        }
        let component = active_component.clone();
        let (progress, title, detail) = component
            .as_deref()
            .and_then(read_model_download_state)
            .unwrap_or_else(|| {
                if core_models_ready() {
                    (
                        1.0,
                        "Ready".into(),
                        "Core translation models are installed".into(),
                    )
                } else {
                    (
                        0.0,
                        "Models required".into(),
                        "Download models to start translating".into(),
                    )
                }
            });
        ModelStatus {
            core_ready: core_models_ready(),
            downloading: active_component.is_some(),
            component,
            progress,
            title,
            detail,
            core_download_bytes: 4_222_472_192,
        }
    }
}

fn model_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".OminiX/models")
}

fn core_models_ready() -> bool {
    let root = model_root();
    let asr = root.join("qwen3-asr-1.7b");
    let translator = root.join("Qwen3.5-2B-MLX-4bit");
    asr.join("config.json").is_file()
        && translator.join("config.json").is_file()
        && translator.join("tokenizer.json").is_file()
        && (translator.join("model.safetensors").is_file()
            || translator.join("model.safetensors.index.json").is_file())
}

fn model_state_path(component: &str) -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/Logs/HenLocalTranslator")
        .join(format!("{component}_model_state.txt"))
}

fn read_model_download_state(component: &str) -> Option<(f64, String, String)> {
    let content = fs::read_to_string(model_state_path(component)).ok()?;
    let mut fields = content.trim().split('|');
    let _step = fields.next()?;
    let title = fields.next()?.to_string();
    let detail = fields.next()?.to_string();
    let progress = fields.next()?.parse::<f64>().ok()?.clamp(0.0, 1.0);
    Some((progress, title, detail))
}

fn resolve_model_downloader() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            candidates.push(directory.join("hen-local-init"));
        }
    }
    for variable in ["MOXIN_DORA_TARGET_DIR", "CARGO_TARGET_DIR"] {
        if let Some(directory) = std::env::var_os(variable) {
            let directory = PathBuf::from(directory);
            candidates.push(directory.join("debug/hen-local-init"));
            candidates.push(directory.join("release/hen-local-init"));
        }
    }
    candidates.extend([
        PathBuf::from("target/debug/hen-local-init"),
        PathBuf::from("target/release/hen-local-init"),
    ]);
    candidates.into_iter().find(|candidate| candidate.is_file())
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> SettingsPayload {
    let preferences = state.preferences.lock().clone();
    let runtime_state = state.poll_runtime_events();
    SettingsPayload {
        settings: TranslationSettings::from(&preferences),
        input_devices: input_devices(),
        subtitle_preview_visible: *state.subtitle_preview_visible.lock(),
        running: runtime_state.running,
        runtime_status: runtime_state.status,
        runtime_message: runtime_state.message,
    }
}

#[tauri::command]
fn get_model_status(state: State<'_, AppState>) -> ModelStatus {
    state.model_status()
}

#[tauri::command]
fn start_model_download(
    state: State<'_, AppState>,
    component: String,
) -> Result<ModelStatus, String> {
    let component = match component.as_str() {
        "core" => "core",
        _ => return Err("Unknown model component".into()),
    };
    if component == "core" && core_models_ready() {
        return Ok(state.model_status());
    }
    let mut running = state.model_download_process.lock();
    if let Some((_, child)) = running.as_mut() {
        if child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_none()
        {
            return Err("A model download is already running".into());
        }
        *running = None;
    }

    let downloader = resolve_model_downloader()
        .ok_or_else(|| "The bundled model downloader is missing. Reinstall the app.".to_string())?;
    let state_path = model_state_path(component);
    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let _ = fs::remove_file(&state_path);
    let log_path = state_path.with_extension("log");
    let log = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&log_path)
        .map_err(|error| format!("Could not create model download log: {error}"))?;
    let child = Command::new(downloader)
        .env("HEN_LOCAL_MODEL_COMPONENT", component)
        .env("HEN_LOCAL_BOOTSTRAP_STATE_PATH", &state_path)
        .stdout(Stdio::from(log.try_clone().map_err(|error| {
            format!("Could not open model log: {error}")
        })?))
        .stderr(Stdio::from(log))
        .spawn()
        .map_err(|error| format!("Could not start model download: {error}"))?;
    *running = Some((component.to_string(), child));
    drop(running);
    Ok(state.model_status())
}

#[tauri::command]
fn update_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    settings: TranslationSettings,
) -> Result<(), String> {
    let (overlay_mode_changed, speech_settings_changed) = {
        let mut preferences = state.preferences.lock();
        let overlay_changed =
            preferences.translation_overlay_fullscreen != settings.overlay_fullscreen;
        let speech_changed = preferences.experimental_spoken_translation_enabled
            != settings.spoken_translation_enabled
            || preferences.experimental_spoken_translation_voice
                != settings.spoken_translation_voice;
        settings.apply_to(&mut preferences);
        preferences::save(&preferences)?;
        (overlay_changed, speech_changed)
    };
    state.sync_shared_state();
    if speech_settings_changed && state.runtime_state.lock().running {
        *state.spoken_completed_count.lock() = state
            .runtime
            .shared_state()
            .translation
            .read()
            .map(|update| update.completed_count)
            .unwrap_or(0);
        state.apple_speech.configure(
            settings.spoken_translation_enabled,
            &settings.target_language,
            settings
                .spoken_translation_voice
                .as_deref()
                .unwrap_or("apple-voice-1"),
        );
    }
    apply_native_identity(&app, &settings)?;
    apply_overlay_window(&app, &settings, overlay_mode_changed)?;
    if *state.subtitle_preview_visible.lock() {
        state.show_subtitle_preview(&settings);
    }
    Ok(())
}

#[tauri::command]
fn start_translation(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    settings: TranslationSettings,
) -> Result<RuntimeState, String> {
    stop_preview_process(&state);
    state.account.translation_allowed()?;
    if !core_models_ready() {
        return Err("Download the core translation models before starting live translation".into());
    }
    {
        let mut preferences = state.preferences.lock();
        settings.apply_to(&mut preferences);
        preferences::save(&preferences)?;
    }
    state.sync_shared_state();

    let dataflow = render_translation_dataflow(
        state.resource_dir.as_deref(),
        RenderOptions {
            source_language: &settings.source_language,
            target_language: &settings.target_language,
            system_audio: settings.input_device == "__system_audio__",
        },
    )?;

    state.clear_subtitle_preview();
    let shared = state.runtime.shared_state();
    shared.translation.set(None);
    shared.translation_window_visible.set(true);
    shared.translation_overlay_active.set(true);
    shared.translation_overlay_status.set("warming".into());
    *state.spoken_completed_count.lock() = 0;
    state.apple_speech.configure(
        settings.spoken_translation_enabled,
        &settings.target_language,
        settings
            .spoken_translation_voice
            .as_deref()
            .unwrap_or("apple-voice-1"),
    );
    state.runtime.start(dataflow)?;
    if let Err(error) = state.usage.start() {
        let _ = state.runtime.stop();
        return Err(error);
    }
    apply_overlay_window(&app, &settings, false)?;
    if let Some(window) = app.get_webview_window("overlay") {
        window.show().map_err(|error| error.to_string())?;
    }

    let next = RuntimeState {
        running: true,
        status: "warming".into(),
        message: "Starting local translation…".into(),
    };
    *state.runtime_state.lock() = next.clone();
    Ok(next)
}

#[tauri::command]
fn get_account_status(state: State<'_, AppState>) -> AccountStatus {
    state.account.status()
}

#[tauri::command]
fn begin_account_sign_in(state: State<'_, AppState>) -> Result<AccountStatus, String> {
    state.account.begin_sign_in()
}

#[tauri::command]
async fn refresh_account(state: State<'_, AppState>) -> Result<AccountStatus, String> {
    state.account.refresh().await
}

#[tauri::command]
async fn open_account_checkout(state: State<'_, AppState>) -> Result<(), String> {
    state.account.checkout().await
}

#[tauri::command]
async fn open_account_portal(state: State<'_, AppState>) -> Result<(), String> {
    state.account.portal().await
}

#[tauri::command]
async fn deactivate_account_device(
    state: State<'_, AppState>,
    device_id: String,
) -> Result<AccountStatus, String> {
    state.account.deactivate_device(&device_id).await
}

#[tauri::command]
fn sign_out_account(state: State<'_, AppState>) -> Result<AccountStatus, String> {
    state.account.sign_out()
}

#[tauri::command]
fn stop_translation(state: State<'_, AppState>) -> Result<RuntimeState, String> {
    state.apple_speech.stop();
    *state.spoken_completed_count.lock() = 0;
    state.save_transcript_if_needed()?;
    state.runtime.stop()?;
    state.usage.stop()?;
    let shared = state.runtime.shared_state();
    shared.translation.set(None);
    shared.translation_window_visible.set(true);
    shared.translation_overlay_active.set(false);
    shared.translation_overlay_status.set("idle".into());
    *state.subtitle_preview_visible.lock() = false;

    let next = RuntimeState {
        running: false,
        status: "idle".into(),
        message: "Translation stopped".into(),
    };
    *state.runtime_state.lock() = next.clone();
    Ok(next)
}

#[tauri::command]
fn get_usage(state: State<'_, AppState>) -> UsageSnapshot {
    state.usage.snapshot()
}

#[tauri::command]
fn set_usage_comparison_rate(
    state: State<'_, AppState>,
    rate: f64,
) -> Result<UsageSnapshot, String> {
    state.usage.set_comparison_rate(rate)
}

#[tauri::command]
fn get_overlay_state(state: State<'_, AppState>) -> OverlayState {
    state.overlay_state()
}

#[tauri::command]
fn open_transcript_history(state: State<'_, AppState>) -> Result<(), String> {
    let directory = preferences::transcript_dir(&state.preferences.lock());
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create transcript directory: {error}"))?;
    open_directory(&directory)
}

#[tauri::command]
fn toggle_subtitle_preview(state: State<'_, AppState>) -> Result<bool, String> {
    if state.runtime_state.lock().running {
        return Err("Stop live translation before changing the test subtitles".into());
    }

    let next = !*state.subtitle_preview_visible.lock();
    if next {
        let preferences = state.preferences.lock();
        let settings = TranslationSettings::from(&*preferences);
        drop(preferences);
        state.show_subtitle_preview(&settings);
    } else {
        state.clear_subtitle_preview();
    }
    Ok(next)
}

fn stop_preview_process(state: &AppState) {
    if let Some(mut child) = state.voice_preview_process.lock().take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[tauri::command]
fn preview_spoken_voice(
    state: State<'_, AppState>,
    voice: String,
    language: String,
) -> Result<(), String> {
    stop_preview_process(&state);
    let sample = match language.as_str() {
        "zh" => "欢迎使用很 Local 实时翻译，这是苹果系统音色试听。",
        "ja" => "Hen Local リアルタイム翻訳のシステム音声プレビューです。",
        "fr" => "Bienvenue dans Hen Local, voici un aperçu de la voix système Apple.",
        _ => "Welcome to Hen Local Live Translator. This is an Apple system voice preview.",
    };
    let child = apple_speech::preview(&voice, &language, sample)?;

    *state.voice_preview_process.lock() = Some(child);
    Ok(())
}

#[tauri::command]
fn stop_spoken_voice_preview(state: State<'_, AppState>) {
    stop_preview_process(&state);
}

#[tauri::command]
fn list_apple_voices() -> Vec<apple_speech::SystemVoice> {
    apple_speech::available_voices()
}

#[tauri::command]
fn preview_apple_voice(
    state: State<'_, AppState>,
    name: String,
    locale: String,
) -> Result<(), String> {
    stop_preview_process(&state);
    let child = apple_speech::preview_named(&name, &locale)?;
    *state.voice_preview_process.lock() = Some(child);
    Ok(())
}

#[tauri::command]
fn open_voice_lab(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("voice-lab") {
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
        return Ok(());
    }
    WebviewWindowBuilder::new(&app, "voice-lab", WebviewUrl::App("voice-lab.html".into()))
        .title("Apple Voice Lab")
        .inner_size(1080.0, 760.0)
        .min_inner_size(760.0, 560.0)
        .center()
        .build()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn input_devices() -> Vec<String> {
    let mut devices = vec!["__system_audio__".into(), "__default_microphone__".into()];
    if let Ok(discovered) = cpal::default_host().input_devices() {
        devices.extend(discovered.filter_map(|device| device.name().ok()));
    }
    devices.sort_by(|left, right| {
        let rank = |value: &str| match value {
            "__system_audio__" => 0,
            "__default_microphone__" => 1,
            _ => 2,
        };
        rank(left).cmp(&rank(right)).then_with(|| left.cmp(right))
    });
    devices.dedup();
    devices
}

fn create_overlay(app: &tauri::App) -> tauri::Result<WebviewWindow> {
    let overlay = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("overlay.html".into()))
        .title("Hen Local Live Translator")
        .inner_size(960.0, 640.0)
        .min_inner_size(560.0, 260.0)
        .resizable(true)
        .decorations(false)
        .always_on_top(true)
        .visible(true)
        .center()
        .build()?;

    let app_handle = app.handle().clone();
    overlay.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let state = app_handle.state::<AppState>();
            state
                .runtime
                .shared_state()
                .translation_window_visible
                .set(true);
        }
    });
    Ok(overlay)
}

fn localized_app_name(language: &str) -> &'static str {
    if language == "en" {
        "Hen Local Live Translator"
    } else {
        "很 Local 实时翻译"
    }
}

fn apply_native_identity(
    app: &tauri::AppHandle,
    settings: &TranslationSettings,
) -> Result<(), String> {
    let name = localized_app_name(&settings.app_language);
    if let Some(window) = app.get_webview_window("main") {
        window.set_title(name).map_err(|error| error.to_string())?;
    }
    if let Some(window) = app.get_webview_window("overlay") {
        window.set_title(name).map_err(|error| error.to_string())?;
    }
    apply_macos_dock_icon(app, &settings.accent_theme)
}

#[cfg(target_os = "macos")]
fn apply_macos_dock_icon(app: &tauri::AppHandle, accent_theme: &str) -> Result<(), String> {
    let icon: &'static [u8] = match accent_theme {
        "neon-orange" => include_bytes!("../icons/icon-neon-orange.png"),
        "neon-pink" => include_bytes!("../icons/icon-neon-pink.png"),
        "neon-green" => include_bytes!("../icons/icon-neon-green.png"),
        _ => include_bytes!("../icons/icon-neon-blue.png"),
    };

    app.run_on_main_thread(move || {
        use objc2::{AllocAnyThread, MainThreadMarker};
        use objc2_app_kit::{NSApplication, NSImage};
        use objc2_foundation::NSData;

        let marker = unsafe { MainThreadMarker::new_unchecked() };
        let application = NSApplication::sharedApplication(marker);
        let data = NSData::with_bytes(icon);
        if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
            unsafe { application.setApplicationIconImage(Some(&image)) };
        }
    })
    .map_err(|error| error.to_string())
}

#[cfg(not(target_os = "macos"))]
fn apply_macos_dock_icon(_app: &tauri::AppHandle, _accent_theme: &str) -> Result<(), String> {
    Ok(())
}

fn apply_overlay_window(
    app: &tauri::AppHandle,
    settings: &TranslationSettings,
    resize_for_mode: bool,
) -> Result<(), String> {
    let Some(window) = app.get_webview_window("overlay") else {
        return Ok(());
    };
    if resize_for_mode {
        let size = if settings.overlay_fullscreen {
            LogicalSize::new(960.0, 640.0)
        } else {
            LogicalSize::new(680.0, 340.0)
        };
        window
            .set_size(Size::Logical(size))
            .map_err(|error| error.to_string())?;
    }
    set_window_opacity(&window, settings.overlay_opacity)
}

#[cfg(target_os = "macos")]
fn set_window_opacity(window: &WebviewWindow, opacity: f64) -> Result<(), String> {
    use objc2::{msg_send, runtime::AnyObject};
    let ns_window = window.ns_window().map_err(|error| error.to_string())? as *mut AnyObject;
    unsafe {
        let _: () = msg_send![ns_window, setAlphaValue: opacity.clamp(0.35, 1.0)];
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn set_window_opacity(_window: &WebviewWindow, _opacity: f64) -> Result<(), String> {
    Ok(())
}

fn open_directory(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer");
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = Command::new("xdg-open");

    command
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open transcript history: {error}"))
}

fn start_event_bridge(app_handle: tauri::AppHandle) {
    thread::spawn(move || loop {
        if app_handle.get_webview_window("main").is_none() {
            break;
        }
        let state = app_handle.state::<AppState>();
        let runtime_state = state.poll_runtime_events();
        state.queue_completed_translations_for_speech();
        let overlay_state = state.overlay_state();
        let _ = app_handle.emit_to("main", "runtime-state", &runtime_state);
        let _ = app_handle.emit_to("overlay", "overlay-state", &overlay_state);
        thread::sleep(Duration::from_millis(80));
    });
}

pub fn run(args: Args) {
    let cli_dataflow = args.dataflow.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
            let resource_dir = app.path().resource_dir().ok();
            app.manage(AppState::new(resource_dir));
            {
                let app_handle = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    for url in event.urls() {
                        let callback = url.to_string();
                        let handle = app_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            let account = {
                                let state = handle.state::<AppState>();
                                state.account.clone()
                            };
                            let result = account.complete_sign_in(&callback).await;
                            match result {
                                Ok(status) => {
                                    let _ = handle.emit_to("main", "account-status", status);
                                    if let Some(window) = handle.get_webview_window("main") {
                                        let _ = window.show();
                                        let _ = window.set_focus();
                                    }
                                }
                                Err(error) => {
                                    let _ = handle.emit_to("main", "account-error", error);
                                }
                            }
                        });
                    }
                });
            }
            {
                let state = app.state::<AppState>();
                usage::start_checkpoint_loop(state.usage.clone());
            }
            create_overlay(app)?;
            let initial_settings = {
                let state = app.state::<AppState>();
                let preferences = state.preferences.lock();
                TranslationSettings::from(&*preferences)
            };
            apply_native_identity(app.handle(), &initial_settings).map_err(anyhow::Error::msg)?;
            apply_overlay_window(app.handle(), &initial_settings, true)
                .map_err(anyhow::Error::msg)?;

            if let Some(dataflow) = cli_dataflow.clone() {
                let state = app.state::<AppState>();
                state
                    .runtime
                    .start(dataflow.into())
                    .map_err(|error| anyhow::anyhow!(error))?;
                state.usage.start().map_err(anyhow::Error::msg)?;
            }
            start_event_bridge(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            get_model_status,
            start_model_download,
            update_settings,
            start_translation,
            stop_translation,
            get_usage,
            set_usage_comparison_rate,
            get_account_status,
            begin_account_sign_in,
            refresh_account,
            open_account_checkout,
            open_account_portal,
            deactivate_account_device,
            sign_out_account,
            get_overlay_state,
            open_transcript_history,
            toggle_subtitle_preview,
            preview_spoken_voice,
            stop_spoken_voice_preview,
            list_apple_voices,
            preview_apple_voice,
            open_voice_lab
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Hen Local Translator")
        .run(|app, event| {
            if let tauri::RunEvent::Ready = event {
                let settings = {
                    let state = app.state::<AppState>();
                    let preferences = state.preferences.lock();
                    TranslationSettings::from(&*preferences)
                };
                if let Err(error) = apply_native_identity(app, &settings) {
                    log::error!("Could not apply native application identity: {error}");
                }
            }
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let state = app.state::<AppState>();
                if state.usage.snapshot().running {
                    if let Err(error) = state.usage.stop() {
                        log::error!("Could not save usage before exit: {error}");
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_preserves_translation_preferences() {
        let original = AppPreferences::default();
        let settings = TranslationSettings::from(&original);
        let mut updated = AppPreferences::default();
        settings.apply_to(&mut updated);
        assert_eq!(
            original.translation_source_language,
            updated.translation_source_language
        );
        assert_eq!(
            original.translation_target_language,
            updated.translation_target_language
        );
        assert_eq!(
            original.translation_input_device,
            updated.translation_input_device
        );
        assert_eq!(original.accent_theme, updated.accent_theme);
    }
}
