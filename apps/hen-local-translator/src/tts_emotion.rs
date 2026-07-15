pub const NEUTRAL_EMOTION_ID: &str = "neutral";
pub const CUSTOM_EMOTION_ID: &str = "custom";
pub const TTS_INSTRUCT_MAX_CHARS: usize = 200;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TtsInstructSelection {
    #[default]
    Neutral,
    Preset(&'static str),
    Custom,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TtsInstructState {
    selection: TtsInstructSelection,
    text: String,
    editor_expanded: bool,
}

impl TtsInstructState {
    pub fn from_preset(id: &str, english: bool) -> Self {
        let mut state = Self::default();
        state.select_preset(id, english);
        state
    }

    pub fn from_history(instruct: Option<&str>) -> Self {
        let Some(instruct) = instruct else {
            return Self::default();
        };
        if instruct.is_empty() {
            return Self::default();
        }

        if let Some(preset) = EMOTION_PRESETS
            .iter()
            .find(|preset| preset.matches_instruct(instruct))
        {
            return Self {
                selection: TtsInstructSelection::Preset(preset.id),
                text: instruct.to_string(),
                editor_expanded: false,
            };
        }

        Self {
            selection: TtsInstructSelection::Custom,
            text: instruct.to_string(),
            editor_expanded: false,
        }
    }

    pub fn select_preset(&mut self, id: &str, english: bool) {
        let preset = emotion_preset_or_neutral(id);
        self.text = preset.instruct(english).unwrap_or_default().to_string();
        self.selection = if preset.id == NEUTRAL_EMOTION_ID {
            TtsInstructSelection::Neutral
        } else {
            TtsInstructSelection::Preset(preset.id)
        };
    }

    pub fn edit(&mut self, text: String) {
        self.selection = if text.is_empty() {
            TtsInstructSelection::Neutral
        } else {
            TtsInstructSelection::Custom
        };
        self.text = text;
    }

    pub fn collapse(&mut self) {
        self.editor_expanded = false;
    }

    pub fn toggle_editor(&mut self) {
        self.editor_expanded = !self.editor_expanded;
    }

    pub fn selection(&self) -> TtsInstructSelection {
        self.selection
    }

    pub fn selection_id(&self) -> &'static str {
        match self.selection {
            TtsInstructSelection::Neutral => NEUTRAL_EMOTION_ID,
            TtsInstructSelection::Preset(id) => id,
            TtsInstructSelection::Custom => CUSTOM_EMOTION_ID,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn editor_expanded(&self) -> bool {
        self.editor_expanded
    }

    pub fn effective_instruct(&self) -> Option<&str> {
        (!self.text.is_empty()).then_some(self.text.as_str())
    }

    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    pub fn is_over_limit(&self) -> bool {
        self.char_count() > TTS_INSTRUCT_MAX_CHARS
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TtsEmotionPreset {
    pub id: &'static str,
    label_zh: &'static str,
    label_en: &'static str,
    instruct_zh: Option<&'static str>,
    instruct_en: Option<&'static str>,
}

impl TtsEmotionPreset {
    pub fn label(&self, english: bool) -> &'static str {
        if english {
            self.label_en
        } else {
            self.label_zh
        }
    }

    pub fn instruct(&self, english: bool) -> Option<&'static str> {
        if english {
            self.instruct_en.or(self.instruct_zh)
        } else {
            self.instruct_zh
        }
    }

    fn matches_instruct(&self, instruct: &str) -> bool {
        self.instruct_zh == Some(instruct)
            || self.instruct_en == Some(instruct)
            || legacy_preset_instructs(self.id).contains(&instruct)
    }
}

pub const EMOTION_PRESETS: &[TtsEmotionPreset] = &[
    TtsEmotionPreset {
        id: NEUTRAL_EMOTION_ID,
        label_zh: "自然",
        label_en: "Neutral",
        instruct_zh: None,
        instruct_en: None,
    },
    TtsEmotionPreset {
        id: "happy",
        label_zh: "开心",
        label_en: "Happy",
        instruct_zh: Some("体现非常开心兴奋的语气，声音明亮有笑意，音调明显上扬，语速轻快，像终于见到很想念的人时忍不住笑着说话。"),
        instruct_en: Some("Deliver it with an unmistakably happy and excited performance: bright smiling voice, clearly rising pitch, brisk pace, as if happily greeting someone you have missed."),
    },
    TtsEmotionPreset {
        id: "angry",
        label_zh: "愤怒",
        label_en: "Angry",
        instruct_zh: Some("体现非常愤怒压抑的语气，音量偏大，咬字很重，语速偏快，重音明显，语气短促有压迫感，像在强烈质问。"),
        instruct_en: Some("Deliver it with an unmistakably angry and forceful performance: louder voice, hard articulation, faster pace, strong stress, short pressuring phrasing, as if strongly questioning someone."),
    },
    TtsEmotionPreset {
        id: "sad",
        label_zh: "难过",
        label_en: "Sad",
        instruct_zh: Some("体现非常悲伤委屈的语气，声音低落发颤，语速缓慢，有明显哭腔和哽咽，句子中带停顿，像快要哭出来。"),
        instruct_en: Some("Deliver it with an unmistakably sad and hurt performance: low trembling voice, slow pace, audible tearful tone and catches in the throat, with pauses as if about to cry."),
    },
    TtsEmotionPreset {
        id: "gentle",
        label_zh: "温柔",
        label_en: "Gentle",
        instruct_zh: Some("体现温柔安抚的语气，声音柔和靠前，音量适中，语速平稳偏慢，重音轻，像在耐心安慰对方。"),
        instruct_en: Some("Deliver it with a gentle reassuring performance: soft close voice, moderate volume, steady slightly slow pace, light stress, as if patiently comforting someone."),
    },
    TtsEmotionPreset {
        id: "excited",
        label_zh: "兴奋",
        label_en: "Excited",
        instruct_zh: Some("体现非常兴奋期待的语气，声音明亮有冲劲，音调起伏更大，语速偏快，重音积极，像迫不及待分享好消息。"),
        instruct_en: Some("Deliver it with a very excited and anticipatory performance: bright energetic voice, wider pitch movement, faster pace, upbeat stress, as if eager to share good news."),
    },
];

fn legacy_preset_instructs(id: &str) -> &'static [&'static str] {
    match id {
        "happy" => &["用开心、轻快的语气说", "Say it in a happy and lively tone"],
        "angry" => &[
            "用极其愤怒、严厉斥责、情绪爆发的语气说，语调强烈，咬字有力",
            "Use an extremely angry, stern, emotionally explosive tone, with forceful intonation and articulation.",
        ],
        "sad" => &["用难过、低落的语气说", "Say it in a sad and subdued tone"],
        "gentle" => &["用温柔、平静的语气说", "Say it in a gentle and calm tone"],
        "excited" => &[
            "用兴奋、充满活力的语气说",
            "Say it in an excited and energetic tone",
        ],
        _ => &[],
    }
}

pub fn emotion_preset(id: &str) -> Option<&'static TtsEmotionPreset> {
    EMOTION_PRESETS.iter().find(|preset| preset.id == id)
}

pub fn emotion_preset_or_neutral(id: &str) -> &'static TtsEmotionPreset {
    emotion_preset(id).unwrap_or(&EMOTION_PRESETS[0])
}

pub fn supports_custom_voice_instruct(backend: &str, is_builtin_voice: bool) -> bool {
    backend == "qwen3_tts_mlx" && is_builtin_voice
}

pub fn instruct_for_request<'a>(
    state: &'a TtsInstructState,
    backend: &str,
    is_builtin_voice: bool,
    text: &str,
) -> Option<&'a str> {
    if !supports_custom_voice_instruct(backend, is_builtin_voice) {
        return None;
    }

    if text_prefers_chinese_instruct(text) {
        if let TtsInstructSelection::Preset(id) = state.selection() {
            if let Some(instruct) = emotion_preset(id).and_then(|preset| preset.instruct(false)) {
                return Some(instruct);
            }
        }
    }

    state.effective_instruct()
}

fn text_prefers_chinese_instruct(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(
            ch,
            '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}'
        )
    })
}

pub fn selection_for_request(
    state: &TtsInstructState,
    backend: &str,
    is_builtin_voice: bool,
) -> TtsInstructSelection {
    if supports_custom_voice_instruct(backend, is_builtin_voice) {
        state.selection()
    } else {
        TtsInstructSelection::Neutral
    }
}

pub fn instruct_is_over_limit_for_request(
    state: &TtsInstructState,
    backend: &str,
    is_builtin_voice: bool,
) -> bool {
    supports_custom_voice_instruct(backend, is_builtin_voice) && state.is_over_limit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_emotion_has_no_native_instruct() {
        let preset = emotion_preset("neutral").unwrap();

        assert_eq!(preset.instruct(false), None);
        assert_eq!(preset.instruct(true), None);
    }

    #[test]
    fn happy_emotion_maps_to_native_instruct() {
        let preset = emotion_preset("happy").unwrap();

        assert_eq!(preset.label(false), "开心");
        assert_eq!(preset.label(true), "Happy");
        assert_eq!(
            preset.instruct(false),
            Some("体现非常开心兴奋的语气，声音明亮有笑意，音调明显上扬，语速轻快，像终于见到很想念的人时忍不住笑着说话。")
        );
        assert_eq!(
            preset.instruct(true),
            Some("Deliver it with an unmistakably happy and excited performance: bright smiling voice, clearly rising pitch, brisk pace, as if happily greeting someone you have missed.")
        );
    }

    #[test]
    fn angry_uses_approved_strong_instruct_in_both_languages() {
        let angry = emotion_preset("angry").unwrap();

        assert_eq!(
            angry.instruct(false),
            Some("体现非常愤怒压抑的语气，音量偏大，咬字很重，语速偏快，重音明显，语气短促有压迫感，像在强烈质问。")
        );
        assert_eq!(
            angry.instruct(true),
            Some(
                "Deliver it with an unmistakably angry and forceful performance: louder voice, hard articulation, faster pace, strong stress, short pressuring phrasing, as if strongly questioning someone."
            )
        );
    }

    #[test]
    fn strong_preset_instructs_match_the_approved_performance_descriptions() {
        assert_eq!(
            emotion_preset("sad").unwrap().instruct(false),
            Some("体现非常悲伤委屈的语气，声音低落发颤，语速缓慢，有明显哭腔和哽咽，句子中带停顿，像快要哭出来。")
        );
        assert_eq!(
            emotion_preset("sad").unwrap().instruct(true),
            Some("Deliver it with an unmistakably sad and hurt performance: low trembling voice, slow pace, audible tearful tone and catches in the throat, with pauses as if about to cry.")
        );
        assert_eq!(
            emotion_preset("gentle").unwrap().instruct(false),
            Some("体现温柔安抚的语气，声音柔和靠前，音量适中，语速平稳偏慢，重音轻，像在耐心安慰对方。")
        );
        assert_eq!(
            emotion_preset("gentle").unwrap().instruct(true),
            Some("Deliver it with a gentle reassuring performance: soft close voice, moderate volume, steady slightly slow pace, light stress, as if patiently comforting someone.")
        );
        assert_eq!(
            emotion_preset("excited").unwrap().instruct(false),
            Some("体现非常兴奋期待的语气，声音明亮有冲劲，音调起伏更大，语速偏快，重音积极，像迫不及待分享好消息。")
        );
        assert_eq!(
            emotion_preset("excited").unwrap().instruct(true),
            Some("Deliver it with a very excited and anticipatory performance: bright energetic voice, wider pitch movement, faster pace, upbeat stress, as if eager to share good news.")
        );
    }

    #[test]
    fn preset_edit_and_clear_follow_custom_state_rules() {
        let mut state = TtsInstructState::default();

        state.select_preset("happy", false);
        assert_eq!(state.selection(), TtsInstructSelection::Preset("happy"));
        assert_eq!(
            state.text(),
            "体现非常开心兴奋的语气，声音明亮有笑意，音调明显上扬，语速轻快，像终于见到很想念的人时忍不住笑着说话。"
        );

        state.edit("请带着克制但明显的喜悦说".to_string());
        assert_eq!(state.selection(), TtsInstructSelection::Custom);

        state.edit(String::new());
        assert_eq!(state.selection(), TtsInstructSelection::Neutral);
        assert_eq!(state.effective_instruct(), None);
    }

    #[test]
    fn whitespace_only_text_remains_custom_and_is_not_trimmed() {
        let mut state = TtsInstructState::default();

        state.edit("  \n".to_string());

        assert_eq!(state.selection(), TtsInstructSelection::Custom);
        assert_eq!(state.effective_instruct(), Some("  \n"));
    }

    #[test]
    fn history_matches_any_localized_preset_variant() {
        assert_eq!(
            TtsInstructState::from_history(Some("Deliver it with an unmistakably happy and excited performance: bright smiling voice, clearly rising pitch, brisk pace, as if happily greeting someone you have missed.")),
            TtsInstructState::from_preset("happy", true)
        );
        assert_eq!(
            TtsInstructState::from_history(Some("体现非常开心兴奋的语气，声音明亮有笑意，音调明显上扬，语速轻快，像终于见到很想念的人时忍不住笑着说话。")).selection(),
            TtsInstructSelection::Preset("happy")
        );
    }

    #[test]
    fn history_still_matches_legacy_short_preset_instructs() {
        assert_eq!(
            TtsInstructState::from_history(Some("Say it in a happy and lively tone")).selection(),
            TtsInstructSelection::Preset("happy")
        );
        assert_eq!(
            TtsInstructState::from_history(Some("用难过、低落的语气说")).selection(),
            TtsInstructSelection::Preset("sad")
        );
    }

    #[test]
    fn unsupported_request_omits_preserved_instruct() {
        let state = TtsInstructState::from_preset("happy", false);

        assert_eq!(
            instruct_for_request(&state, "qwen3_tts_mlx", true, "Hello there"),
            Some("体现非常开心兴奋的语气，声音明亮有笑意，音调明显上扬，语速轻快，像终于见到很想念的人时忍不住笑着说话。")
        );
        assert_eq!(
            instruct_for_request(&state, "primespeech", true, "Hello there"),
            None
        );
        assert_eq!(
            instruct_for_request(&state, "qwen3_tts_mlx", false, "Hello there"),
            None
        );
    }

    #[test]
    fn preset_request_prefers_chinese_instruct_for_chinese_text() {
        let state = TtsInstructState::from_preset("happy", true);

        assert_eq!(
            state.text(),
            "Deliver it with an unmistakably happy and excited performance: bright smiling voice, clearly rising pitch, brisk pace, as if happily greeting someone you have missed."
        );
        assert_eq!(
            instruct_for_request(
                &state,
                "qwen3_tts_mlx",
                true,
                "复杂的问题背后也许没有统一的答案"
            ),
            Some("体现非常开心兴奋的语气，声音明亮有笑意，音调明显上扬，语速轻快，像终于见到很想念的人时忍不住笑着说话。")
        );
    }

    #[test]
    fn custom_request_keeps_custom_instruct_for_chinese_text() {
        let mut state = TtsInstructState::default();
        state.edit("Say this with restrained but clear delight.".to_string());

        assert_eq!(
            instruct_for_request(
                &state,
                "qwen3_tts_mlx",
                true,
                "复杂的问题背后也许没有统一的答案"
            ),
            Some("Say this with restrained but clear delight.")
        );
    }

    #[test]
    fn unsupported_request_ignores_hidden_validation_and_uses_neutral_metadata() {
        let mut state = TtsInstructState::default();
        state.edit("情".repeat(TTS_INSTRUCT_MAX_CHARS + 1));

        assert!(!instruct_is_over_limit_for_request(
            &state,
            "qwen3_tts_mlx",
            false
        ));
        assert_eq!(
            selection_for_request(&state, "qwen3_tts_mlx", false),
            TtsInstructSelection::Neutral
        );
        assert!(instruct_is_over_limit_for_request(
            &state,
            "qwen3_tts_mlx",
            true
        ));
        assert_eq!(
            selection_for_request(&state, "qwen3_tts_mlx", true),
            TtsInstructSelection::Custom
        );
    }

    #[test]
    fn instruct_limit_counts_unicode_scalars() {
        let mut state = TtsInstructState::default();

        state.edit("情".repeat(TTS_INSTRUCT_MAX_CHARS));
        assert_eq!(state.char_count(), 200);
        assert!(!state.is_over_limit());

        state.edit("情".repeat(TTS_INSTRUCT_MAX_CHARS + 1));
        assert_eq!(state.char_count(), 201);
        assert!(state.is_over_limit());
    }

    #[test]
    fn history_restore_classifies_neutral_preset_and_custom() {
        assert_eq!(
            TtsInstructState::from_history(None).selection(),
            TtsInstructSelection::Neutral
        );
        assert_eq!(
            TtsInstructState::from_history(Some("")).selection(),
            TtsInstructSelection::Neutral
        );
        assert_eq!(
            TtsInstructState::from_history(Some("Deliver it with an unmistakably sad and hurt performance: low trembling voice, slow pace, audible tearful tone and catches in the throat, with pauses as if about to cry.")).selection(),
            TtsInstructSelection::Preset("sad")
        );
        assert_eq!(
            TtsInstructState::from_history(Some("Very restrained, but clearly hopeful."))
                .selection(),
            TtsInstructSelection::Custom
        );
    }
}
