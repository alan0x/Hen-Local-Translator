//! Dora runtime integration for the active Hen Local translation dataflow.
//!
//! The crate owns the microphone/system-audio input bridge, translated-text
//! listener, optional spoken-output bridge, dataflow lifecycle, and the shared
//! state consumed by the Tauri desktop shell.

pub mod bridge;
pub mod controller;
pub mod data;
pub mod dispatcher;
pub mod error;
pub mod parser;
pub mod shared_state;

// Widget-specific bridges
pub mod widgets;

// Re-exports
pub use bridge::{BridgeState, DoraBridge};
pub use controller::{DataflowController, DataflowState};
pub use data::{AudioData, DoraData, TranslationUpdate};
pub use dispatcher::{DynamicNodeDispatcher, WidgetBinding};
pub use error::{BridgeError, BridgeResult};
pub use parser::{DataflowParser, EnvRequirement, ParsedDataflow, ParsedNode};
pub use shared_state::{AudioState, DirtyValue, DoraStatus, MicState, SharedDoraState};
pub use widgets::{AecControlCommand, AudioSource, TranslationListenerBridge};

/// Prefix for Moxin built-in dynamic nodes in dataflow YAML
pub const MOFA_NODE_PREFIX: &str = "moxin-";

/// Known Moxin widget node types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MoxinNodeType {
    /// Audio player widget - receives audio, plays through speaker
    AudioPlayer,
    /// Mic input widget - captures audio from microphone
    MicInput,
    /// Translation listener widget - receives source_text + translation from translator node
    TranslationListener,
}

impl MoxinNodeType {
    /// Get the node ID for this widget type
    pub fn node_id(&self) -> &'static str {
        match self {
            MoxinNodeType::AudioPlayer => "moxin-audio-player",
            MoxinNodeType::MicInput => "moxin-mic-input",
            MoxinNodeType::TranslationListener => "moxin-translation-listener",
        }
    }

    /// Parse node type from node ID
    pub fn from_node_id(node_id: &str) -> Option<Self> {
        match node_id {
            "moxin-audio-player" => Some(MoxinNodeType::AudioPlayer),
            "moxin-mic-input" => Some(MoxinNodeType::MicInput),
            "moxin-translation-listener" => Some(MoxinNodeType::TranslationListener),
            _ => None,
        }
    }

    /// Check if a node ID is a Moxin widget node
    pub fn is_moxin_node(node_id: &str) -> bool {
        node_id.starts_with(MOFA_NODE_PREFIX)
    }
}
