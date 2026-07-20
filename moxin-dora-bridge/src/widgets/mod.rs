//! Widget-specific bridge implementations
//!
//! Each widget type has its own bridge that connects to dora as a dynamic node:
//! - `moxin-audio-player`: Receives audio, forwards to UI for playback
//! - `moxin-aec-input`: Captures mic audio with AEC, sends to ASR
//!
//! Note: LED visualization is calculated in screen.rs from output waveform
//! (more accurate since it reflects what's actually being played)

mod aec_input;
mod audio_player;
#[cfg(target_os = "macos")]
mod screencapture_input;
mod translation_listener;

pub use aec_input::{AecControlCommand, AecInputBridge, AudioSource};
pub use audio_player::AudioPlayerBridge;
#[cfg(target_os = "macos")]
pub use screencapture_input::{permission_granted, probe_permission_async};
pub use translation_listener::TranslationListenerBridge;
