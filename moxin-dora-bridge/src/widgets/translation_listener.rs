//! Translation Listener Bridge
//!
//! Listens to translator node output (source_text + translation) and writes
//! streaming/final results to SharedDoraState for consumption by the UI overlay.

use crate::bridge::{BridgeState, DoraBridge};
use crate::data::{DoraData, SentenceUnit, StreamingTranslation, TranslationUpdate};
use crate::error::{BridgeError, BridgeResult};
use crate::shared_state::SharedDoraState;
use crossbeam_channel::{bounded, Receiver, Sender};
use dora_node_api::{
    dora_core::config::{DataId, NodeId},
    DoraNode, Event, Parameter,
};
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::thread;
use tracing::{debug, error, info, warn};

/// Translation Listener Bridge - monitors translator node output
pub struct TranslationListenerBridge {
    /// Node ID in the dataflow (should be "moxin-translation-listener")
    node_id: String,
    /// Connection state
    state: Arc<RwLock<BridgeState>>,
    /// Shared state for writing translation results
    shared_state: Option<Arc<SharedDoraState>>,
    /// Stop signal
    stop_sender: Option<Sender<()>>,
    /// Worker thread handle
    worker_handle: Option<thread::JoinHandle<()>>,
}

#[derive(Debug, Default)]
struct TranslationDisplayState {
    history: Vec<SentenceUnit>,
    pending_source_text: String,
    current_source_text: String,
    pending_completed_sources: HashMap<i64, String>,
    pending_completed_translations: HashMap<i64, String>,
    finalized_commit_ids: HashSet<i64>,
    completed_count: u64,
    active_translation: Option<StreamingTranslation>,
}

impl TranslationDisplayState {
    fn strip_active_source_prefix(&self, text: String) -> String {
        let Some(active) = self.active_translation.as_ref() else {
            return text;
        };
        text.strip_prefix(&active.source_text)
            .map(|tail| tail.trim_start().to_string())
            .unwrap_or(text)
    }

    fn handle_source_text(
        &mut self,
        session_status: &str,
        text: String,
        commit_id: Option<i64>,
        max_history: usize,
    ) -> bool {
        match session_status {
            "streaming" => {
                let text = self.strip_active_source_prefix(text);
                self.current_source_text = text.clone();
                self.pending_source_text = text;
                false
            }
            "translating" => {
                let Some(commit_id) = commit_id else {
                    return false;
                };
                self.pending_source_text = self
                    .pending_source_text
                    .strip_prefix(&text)
                    .map(|tail| tail.trim_start().to_string())
                    .unwrap_or_else(|| self.pending_source_text.clone());
                self.current_source_text = self
                    .current_source_text
                    .strip_prefix(&text)
                    .map(|tail| tail.trim_start().to_string())
                    .unwrap_or_else(|| self.current_source_text.clone());
                self.active_translation = Some(StreamingTranslation {
                    commit_id,
                    source_text: text,
                    translation: String::new(),
                    complete: false,
                });
                false
            }
            "complete" => {
                if let Some(commit_id) = commit_id {
                    if self.finalized_commit_ids.contains(&commit_id) {
                        return false;
                    }
                    self.pending_completed_sources.insert(commit_id, text);
                    self.try_finalize_complete_pair(commit_id, max_history)
                } else {
                    // Metadata-free safety path for malformed upstream events.
                    self.current_source_text = text;
                    false
                }
            }
            _ => {
                self.current_source_text = text.clone();
                self.pending_source_text = text;
                false
            }
        }
    }

    fn handle_translation_streaming(
        &mut self,
        text: String,
        commit_id: Option<i64>,
        source_text: Option<String>,
    ) -> bool {
        let Some(commit_id) = commit_id else {
            return false;
        };

        if self
            .active_translation
            .as_ref()
            .map(|active| active.commit_id)
            != Some(commit_id)
        {
            let Some(source_text) = source_text else {
                return false;
            };
            self.active_translation = Some(StreamingTranslation {
                commit_id,
                source_text,
                translation: String::new(),
                complete: false,
            });
        }

        let Some(active) = self.active_translation.as_mut() else {
            return false;
        };
        if active.translation == text && !active.complete {
            return false;
        }
        active.translation = text;
        active.complete = false;
        true
    }

    fn handle_translation_failed(&mut self, commit_id: Option<i64>) -> bool {
        if self
            .active_translation
            .as_ref()
            .map(|active| active.commit_id)
            == commit_id
        {
            let Some(active) = self.active_translation.take() else {
                return false;
            };

            // The translator only consumes source text after a successful
            // completion. Put the failed source back into the visible pending
            // text so a retry does not make the original sentence disappear.
            if !self.pending_source_text.starts_with(&active.source_text) {
                self.pending_source_text = if self.pending_source_text.is_empty() {
                    active.source_text
                } else {
                    format!("{} {}", active.source_text, self.pending_source_text)
                };
            }
            self.current_source_text = self.pending_source_text.clone();
            return true;
        }
        false
    }

    fn handle_translation_complete(
        &mut self,
        text: String,
        commit_id: Option<i64>,
        source_text: Option<String>,
        max_history: usize,
    ) -> bool {
        if let Some(commit_id) = commit_id {
            if self.finalized_commit_ids.contains(&commit_id) {
                return false;
            }
            if let Some(source_text) = source_text {
                self.pending_completed_sources.remove(&commit_id);
                self.pending_completed_translations.remove(&commit_id);
                self.finalized_commit_ids.insert(commit_id);
                self.active_translation = Some(StreamingTranslation {
                    commit_id,
                    source_text: source_text.clone(),
                    translation: text.clone(),
                    complete: true,
                });
                self.push_completed_sentence(source_text, text, max_history);
                return true;
            }
            self.pending_completed_translations.insert(commit_id, text);
            self.try_finalize_complete_pair(commit_id, max_history)
        } else {
            self.push_completed_sentence(self.current_source_text.clone(), text, max_history);
            true
        }
    }

    fn try_finalize_complete_pair(&mut self, commit_id: i64, max_history: usize) -> bool {
        let Some(source_text) = self.pending_completed_sources.remove(&commit_id) else {
            return false;
        };
        let Some(translation) = self.pending_completed_translations.remove(&commit_id) else {
            self.pending_completed_sources
                .insert(commit_id, source_text);
            return false;
        };

        self.finalized_commit_ids.insert(commit_id);
        self.push_completed_sentence(source_text, translation, max_history);
        true
    }

    fn push_completed_sentence(
        &mut self,
        source_text: String,
        translation: String,
        max_history: usize,
    ) {
        self.history.push(SentenceUnit {
            source_text: source_text.clone(),
            translation,
        });
        self.completed_count = self.completed_count.saturating_add(1);
        if self.history.len() > max_history {
            self.history.remove(0);
        }

        if self.pending_source_text == source_text {
            self.pending_source_text.clear();
        }
        if self.current_source_text == source_text {
            self.current_source_text.clear();
        }
    }
}

impl TranslationListenerBridge {
    /// Create a new translation listener bridge
    pub fn new(node_id: &str) -> Self {
        Self::with_shared_state(node_id, None)
    }

    /// Create with shared state
    pub fn with_shared_state(node_id: &str, shared_state: Option<Arc<SharedDoraState>>) -> Self {
        Self {
            node_id: node_id.to_string(),
            state: Arc::new(RwLock::new(BridgeState::Disconnected)),
            shared_state,
            stop_sender: None,
            worker_handle: None,
        }
    }

    /// Worker thread: listens for source_text and translation events from the translator node.
    ///
    /// The translator node sends:
    /// - `source_text`: progressive ASR text plus `translating`/`complete` sentence states
    /// - `translation`: cumulative snapshots with metadata
    ///   `session_status = "streaming" | "complete" | "failed"`
    ///
    /// We keep the currently translating sentence separate from finalized history so
    /// frequent snapshots do not clone the entire subtitle history.
    fn run_event_loop(
        node_id: String,
        state: Arc<RwLock<BridgeState>>,
        shared_state: Option<Arc<SharedDoraState>>,
        stop_receiver: Receiver<()>,
    ) {
        info!("[TranslationListener] Worker started for node: {}", node_id);

        let (mut _node, mut events) =
            match DoraNode::init_from_node_id(NodeId::from(node_id.clone())) {
                Ok(n) => n,
                Err(e) => {
                    error!("[TranslationListener] Failed to init dora node: {}", e);
                    *state.write() = BridgeState::Error;
                    return;
                }
            };

        info!("[TranslationListener] Connected to dora as: {}", node_id);
        *state.write() = BridgeState::Connected;

        if let Some(ref shared) = shared_state {
            shared.add_bridge(node_id.clone());
            shared.translation_stream.set(None);
        }

        const MAX_HISTORY: usize = 10_000;
        let mut display = TranslationDisplayState::default();

        loop {
            if stop_receiver.try_recv().is_ok() {
                info!("[TranslationListener] Received stop signal");
                break;
            }

            if let Some(event) = events.recv_timeout(std::time::Duration::from_millis(100)) {
                match event {
                    Event::Input { id, metadata, data } => {
                        // Extract text value from Arrow StringArray
                        let text_value: Option<String> = {
                            use arrow::array::Array;
                            data.as_any()
                                .downcast_ref::<arrow::array::StringArray>()
                                .and_then(|arr| {
                                    if arr.len() > 0 {
                                        Some(arr.value(0).to_string())
                                    } else {
                                        None
                                    }
                                })
                        };

                        let text = match text_value {
                            Some(t) => t,
                            None => continue,
                        };

                        if id == DataId::from("log".to_owned()) {
                            eprintln!("[TranslatorLog] {}", text);
                        } else if id == DataId::from("source_text".to_owned()) {
                            let session_status = metadata
                                .parameters
                                .iter()
                                .find(|(k, _)| k.as_str() == "session_status")
                                .and_then(|(_, v)| {
                                    if let Parameter::String(s) = v {
                                        Some(s.clone())
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or_else(|| "streaming".to_string());
                            let commit_id = metadata
                                .parameters
                                .iter()
                                .find(|(k, _)| k.as_str() == "commit_id")
                                .and_then(|(_, v)| {
                                    if let Parameter::Integer(v) = v {
                                        Some(*v)
                                    } else {
                                        None
                                    }
                                });

                            debug!(
                                "[TranslationListener] source_text ({}, commit_id={:?}): {}",
                                session_status, commit_id, &text
                            );

                            display.handle_source_text(
                                &session_status,
                                text,
                                commit_id,
                                MAX_HISTORY,
                            );
                            if let Some(ref shared) = shared_state {
                                shared.translation.set(Some(TranslationUpdate {
                                    history: display.history.clone(),
                                    pending_source_text: display.pending_source_text.clone(),
                                    completed_count: display.completed_count,
                                }));
                                if session_status == "translating" {
                                    shared
                                        .translation_stream
                                        .set(display.active_translation.clone());
                                }
                            }
                        } else if id == DataId::from("translation".to_owned()) {
                            let session_status = metadata
                                .parameters
                                .iter()
                                .find(|(k, _)| k.as_str() == "session_status")
                                .and_then(|(_, v)| {
                                    if let Parameter::String(s) = v {
                                        Some(s.as_str())
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or("complete");
                            let commit_id = metadata
                                .parameters
                                .iter()
                                .find(|(k, _)| k.as_str() == "commit_id")
                                .and_then(|(_, v)| {
                                    if let Parameter::Integer(v) = v {
                                        Some(*v)
                                    } else {
                                        None
                                    }
                                });
                            let source_text_meta = metadata
                                .parameters
                                .iter()
                                .find(|(k, _)| k.as_str() == "source_text")
                                .and_then(|(_, v)| {
                                    if let Parameter::String(v) = v {
                                        Some(v.clone())
                                    } else {
                                        None
                                    }
                                });

                            debug!(
                                "[TranslationListener] translation ({}, commit_id={:?}, source_text_meta_present={}): {}",
                                session_status,
                                commit_id,
                                source_text_meta.is_some(),
                                &text
                            );

                            if session_status == "streaming" {
                                if display.handle_translation_streaming(
                                    text,
                                    commit_id,
                                    source_text_meta,
                                ) {
                                    if let Some(ref shared) = shared_state {
                                        shared
                                            .translation_stream
                                            .set(display.active_translation.clone());
                                    }
                                }
                            } else if session_status == "complete" {
                                let completed = display.handle_translation_complete(
                                    text,
                                    commit_id,
                                    source_text_meta,
                                    MAX_HISTORY,
                                );
                                if completed {
                                    if let Some(latest) = display.history.last() {
                                        info!(
                                            "[TranslationListener] Translation complete: [{}] -> [{}]",
                                            latest.source_text,
                                            latest.translation
                                        );
                                    }
                                }

                                if let Some(ref shared) = shared_state {
                                    shared
                                        .translation_stream
                                        .set(display.active_translation.clone());
                                    shared.translation.set(Some(TranslationUpdate {
                                        history: display.history.clone(),
                                        pending_source_text: display.pending_source_text.clone(),
                                        completed_count: display.completed_count,
                                    }));
                                }
                            } else if session_status == "failed"
                                && display.handle_translation_failed(commit_id)
                            {
                                if let Some(ref shared) = shared_state {
                                    shared.translation_stream.set(None);
                                    shared.translation.set(Some(TranslationUpdate {
                                        history: display.history.clone(),
                                        pending_source_text: display.pending_source_text.clone(),
                                        completed_count: display.completed_count,
                                    }));
                                }
                            }
                        }
                    }
                    Event::Stop(_) => {
                        info!("[TranslationListener] Received stop event from dora");
                        break;
                    }
                    Event::InputClosed { id } => {
                        debug!("[TranslationListener] Input closed: {:?}", id);
                    }
                    Event::Error(e) => {
                        error!("[TranslationListener] Dora error: {}", e);
                    }
                    _ => {}
                }
            }
        }

        info!("[TranslationListener] Worker stopped");
        *state.write() = BridgeState::Disconnected;

        if let Some(ref shared) = shared_state {
            shared.translation_stream.set(None);
            shared.remove_bridge(&node_id);
        }
    }
}

impl DoraBridge for TranslationListenerBridge {
    fn node_id(&self) -> &str {
        &self.node_id
    }

    fn state(&self) -> BridgeState {
        *self.state.read()
    }

    fn connect(&mut self) -> BridgeResult<()> {
        if self.is_connected() {
            return Ok(());
        }

        *self.state.write() = BridgeState::Connecting;

        let (stop_tx, stop_rx) = bounded(1);
        self.stop_sender = Some(stop_tx);

        let node_id = self.node_id.clone();
        let state = Arc::clone(&self.state);
        let shared_state = self.shared_state.clone();

        let handle = thread::Builder::new()
            .name(format!("translation-listener-{}", node_id))
            .spawn(move || {
                Self::run_event_loop(node_id, state, shared_state, stop_rx);
            })
            .map_err(|e| BridgeError::ThreadSpawnFailed(e.to_string()))?;

        self.worker_handle = Some(handle);

        // Wait for connection — macOS Unix socket init is slower
        #[cfg(target_os = "macos")]
        let max_wait_iterations = 100; // 10 seconds
        #[cfg(not(target_os = "macos"))]
        let max_wait_iterations = 50; // 5 seconds

        for i in 0..max_wait_iterations {
            std::thread::sleep(std::time::Duration::from_millis(100));
            match *self.state.read() {
                BridgeState::Connected => {
                    info!(
                        "[TranslationListener] Connection verified after {} ms",
                        i * 100
                    );
                    return Ok(());
                }
                BridgeState::Error => {
                    error!(
                        "[TranslationListener] Bridge failed to connect: {}",
                        self.node_id
                    );
                    return Err(BridgeError::ConnectionFailed(format!(
                        "TranslationListener {} failed to init dora node",
                        self.node_id
                    )));
                }
                _ => {}
            }
        }

        warn!(
            "[TranslationListener] Bridge connection timeout for: {}",
            self.node_id
        );
        Err(BridgeError::ConnectionFailed(format!(
            "TranslationListener {} connection timeout",
            self.node_id
        )))
    }

    fn disconnect(&mut self) -> BridgeResult<()> {
        if let Some(stop_tx) = self.stop_sender.take() {
            let _ = stop_tx.send(());
        }

        if let Some(handle) = self.worker_handle.take() {
            handle.join().map_err(|_| BridgeError::ThreadJoinFailed)?;
        }

        *self.state.write() = BridgeState::Disconnected;
        Ok(())
    }

    fn send(&self, _output: &str, _data: DoraData) -> BridgeResult<()> {
        // This bridge only listens, doesn't send
        Err(BridgeError::NotSupported(
            "TranslationListenerBridge does not support sending data".to_string(),
        ))
    }

    fn expected_inputs(&self) -> Vec<String> {
        vec![
            "source_text".to_string(),
            "translation".to_string(),
            "log".to_string(),
        ]
    }

    fn expected_outputs(&self) -> Vec<String> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::TranslationDisplayState;

    #[test]
    fn complete_pairing_uses_commit_id_instead_of_latest_streaming_source() {
        let mut state = TranslationDisplayState::default();

        state.handle_source_text("streaming", "旧句".to_string(), None, 50);
        state.handle_source_text("complete", "旧句".to_string(), Some(1), 50);
        state.handle_source_text("streaming", "新句，后半段".to_string(), None, 50);

        let completed =
            state.handle_translation_complete("old translation".to_string(), Some(1), None, 50);
        assert!(completed);

        assert_eq!(state.history.len(), 1);
        assert_eq!(state.history[0].source_text, "旧句");
        assert_eq!(state.history[0].translation, "old translation");
        assert_eq!(state.pending_source_text, "新句，后半段");
        assert_eq!(state.completed_count, 1);
    }

    #[test]
    fn finalized_commit_clears_pending_only_when_pending_matches_same_source() {
        let mut state = TranslationDisplayState::default();

        state.handle_source_text("streaming", "同一句".to_string(), None, 50);
        state.handle_source_text("complete", "同一句".to_string(), Some(7), 50);

        let completed =
            state.handle_translation_complete("same translation".to_string(), Some(7), None, 50);
        assert!(completed);

        assert!(state.pending_source_text.is_empty());
        assert_eq!(state.history.len(), 1);
        assert_eq!(state.history[0].source_text, "同一句");
    }

    #[test]
    fn translation_complete_with_embedded_source_finalizes_without_separate_source_event() {
        let mut state = TranslationDisplayState::default();

        state.handle_source_text("streaming", "新句，后半段".to_string(), None, 50);

        let completed = state.handle_translation_complete(
            "old translation".to_string(),
            Some(11),
            Some("旧句".to_string()),
            50,
        );
        assert!(completed);

        assert_eq!(state.history.len(), 1);
        assert_eq!(state.history[0].source_text, "旧句");
        assert_eq!(state.history[0].translation, "old translation");
        assert_eq!(state.pending_source_text, "新句，后半段");

        let completed_again =
            state.handle_source_text("complete", "旧句".to_string(), Some(11), 50);
        assert!(!completed_again);
        assert_eq!(state.history.len(), 1);
        assert_eq!(state.completed_count, 1);
    }

    #[test]
    fn translating_state_separates_committed_source_from_new_asr_tail() {
        let mut state = TranslationDisplayState::default();

        state.handle_source_text(
            "streaming",
            "The first sentence. The next".to_string(),
            None,
            50,
        );
        state.handle_source_text(
            "translating",
            "The first sentence.".to_string(),
            Some(3),
            50,
        );

        assert_eq!(
            state.active_translation.as_ref().map(|active| (
                active.commit_id,
                active.source_text.as_str(),
                active.translation.as_str(),
                active.complete
            )),
            Some((3, "The first sentence.", "", false))
        );
        assert_eq!(state.pending_source_text, "The next");
    }

    #[test]
    fn cumulative_translation_snapshots_replace_active_text() {
        let mut state = TranslationDisplayState::default();
        state.handle_source_text(
            "translating",
            "A complete sentence.".to_string(),
            Some(8),
            50,
        );

        assert!(state.handle_translation_streaming(
            "一个".to_string(),
            Some(8),
            Some("A complete sentence.".to_string())
        ));
        assert!(state.handle_translation_streaming(
            "一个完整的句子。".to_string(),
            Some(8),
            Some("A complete sentence.".to_string())
        ));

        let active = state.active_translation.expect("active translation");
        assert_eq!(active.translation, "一个完整的句子。");
        assert!(!active.complete);
    }

    #[test]
    fn failed_translation_restores_source_for_retry() {
        let mut state = TranslationDisplayState::default();
        state.handle_source_text(
            "streaming",
            "First sentence. Next words".to_string(),
            None,
            50,
        );
        state.handle_source_text("translating", "First sentence.".to_string(), Some(9), 50);

        assert!(state.handle_translation_failed(Some(9)));
        assert!(state.active_translation.is_none());
        assert_eq!(state.pending_source_text, "First sentence. Next words");
        assert_eq!(state.current_source_text, state.pending_source_text);
    }

    #[test]
    fn final_snapshot_marks_active_translation_complete_and_adds_history() {
        let mut state = TranslationDisplayState::default();
        state.handle_source_text(
            "translating",
            "A complete sentence.".to_string(),
            Some(12),
            50,
        );
        state.handle_translation_streaming(
            "一个完整".to_string(),
            Some(12),
            Some("A complete sentence.".to_string()),
        );

        assert!(state.handle_translation_complete(
            "一个完整的句子。".to_string(),
            Some(12),
            Some("A complete sentence.".to_string()),
            50
        ));

        let active = state
            .active_translation
            .as_ref()
            .expect("active translation");
        assert!(active.complete);
        assert_eq!(active.translation, "一个完整的句子。");
        assert_eq!(state.history.len(), 1);
        assert_eq!(state.completed_count, 1);
    }
}
