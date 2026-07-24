//! Runtime data exchanged between the active Hen Local Dora bridges.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum DoraData {
    Json(serde_json::Value),
}

#[derive(Debug, Clone)]
pub struct AudioData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub participant_id: Option<String>,
    pub question_id: Option<String>,
}

impl AudioData {
    pub fn duration_secs(&self) -> f32 {
        self.samples.len() as f32 / (self.sample_rate as f32 * self.channels as f32)
    }

    pub fn to_mono(&self) -> Vec<f32> {
        if self.channels == 1 {
            return self.samples.clone();
        }
        self.samples
            .chunks(self.channels as usize)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect()
    }
}

#[derive(Debug, Clone, Default)]
pub struct EventMetadata {
    pub values: HashMap<String, String>,
}

impl EventMetadata {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn session_status(&self) -> Option<&str> {
        self.get("session_status")
    }

    pub fn question_id(&self) -> Option<&str> {
        self.get("question_id")
    }

    pub fn participant_id(&self) -> Option<&str> {
        self.get("participant_id")
    }
}

#[derive(Debug, Clone)]
pub struct SentenceUnit {
    pub source_text: String,
    pub translation: String,
}

#[derive(Debug, Clone, Default)]
pub struct TranslationUpdate {
    pub history: Vec<SentenceUnit>,
    pub pending_source_text: String,
    pub completed_count: u64,
}

#[derive(Debug, Clone, Default)]
pub struct StreamingTranslation {
    pub commit_id: i64,
    pub source_text: String,
    pub translation: String,
    pub complete: bool,
}
