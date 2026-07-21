use serde::Serialize;
use std::{
    collections::VecDeque,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread,
    time::Duration,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemVoice {
    pub name: String,
    pub locale: String,
    pub sample: String,
}

enum SpeechCommand {
    Configure {
        enabled: bool,
        language: String,
        voice: String,
    },
    Speak(String),
    Stop,
    Shutdown,
}

pub struct AppleSpeech {
    sender: Sender<SpeechCommand>,
}

impl AppleSpeech {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || speech_worker(receiver));
        Self { sender }
    }

    pub fn configure(&self, enabled: bool, language: &str, voice: &str) {
        let _ = self.sender.send(SpeechCommand::Configure {
            enabled,
            language: language.to_string(),
            voice: normalize_voice_id(voice).to_string(),
        });
    }

    pub fn speak(&self, text: String) {
        if !text.trim().is_empty() {
            let _ = self.sender.send(SpeechCommand::Speak(text));
        }
    }

    pub fn stop(&self) {
        let _ = self.sender.send(SpeechCommand::Stop);
    }
}

impl Drop for AppleSpeech {
    fn drop(&mut self) {
        let _ = self.sender.send(SpeechCommand::Shutdown);
    }
}

fn speech_worker(receiver: Receiver<SpeechCommand>) {
    let installed = installed_voices();
    let mut enabled = false;
    let mut language = String::from("en");
    let mut voice = String::from("apple-voice-1");
    let mut queue = VecDeque::new();
    let mut child: Option<Child> = None;

    loop {
        match receiver.recv_timeout(Duration::from_millis(40)) {
            Ok(SpeechCommand::Configure {
                enabled: next_enabled,
                language: next_language,
                voice: next_voice,
            }) => {
                stop_child(&mut child);
                queue.clear();
                enabled = next_enabled;
                language = next_language;
                voice = next_voice;
            }
            Ok(SpeechCommand::Speak(text)) if enabled => queue.push_back(text),
            Ok(SpeechCommand::Speak(_)) => {}
            Ok(SpeechCommand::Stop) => {
                stop_child(&mut child);
                queue.clear();
                enabled = false;
            }
            Ok(SpeechCommand::Shutdown) => {
                stop_child(&mut child);
                break;
            }
            Err(RecvTimeoutError::Disconnected) => {
                stop_child(&mut child);
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }

        if let Some(process) = child.as_mut() {
            match process.try_wait() {
                Ok(Some(_)) => child = None,
                Err(error) => {
                    log::error!("Could not read Apple speech process state: {error}");
                    stop_child(&mut child);
                }
                Ok(None) => {}
            }
        }
        if enabled && child.is_none() {
            if let Some(text) = queue.pop_front() {
                let Some(selected) = select_voice(&installed, &language, &voice) else {
                    log::warn!("Selected Hen Local voice is unavailable: {language} / {voice}");
                    continue;
                };
                match spawn_say(&selected, &text) {
                    Ok(process) => child = Some(process),
                    Err(error) => log::error!("Could not start Apple speech: {error}"),
                }
            }
        }
    }
}

fn stop_child(child: &mut Option<Child>) {
    if let Some(mut process) = child.take() {
        let _ = process.kill();
        let _ = process.wait();
    }
}

fn spawn_say(voice: &str, text: &str) -> Result<Child, std::io::Error> {
    let mut command = Command::new("/usr/bin/say");
    command.arg("-v").arg(voice);
    command
        .arg(text)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

pub fn preview(voice: &str, language: &str, text: &str) -> Result<Child, String> {
    let installed = installed_voices();
    let selected = select_voice(&installed, language, normalize_voice_id(voice))
        .ok_or_else(|| format!("Selected Hen Local voice is unavailable for {language}"))?;
    spawn_say(&selected, text)
        .map_err(|error| format!("Could not play Apple voice preview: {error}"))
}

pub fn preview_named(name: &str, locale: &str) -> Result<Child, String> {
    let voices = available_voices();
    let voice = voices
        .iter()
        .find(|voice| voice.name == name && voice.locale == locale)
        .ok_or_else(|| format!("Apple system voice is unavailable: {name} ({locale})"))?;
    let sample = audition_text(&voice.locale);
    spawn_say(&voice.name, sample)
        .map_err(|error| format!("Could not play Apple voice preview: {error}"))
}

fn audition_text(locale: &str) -> &'static str {
    if locale.starts_with("zh_") {
        "你好，这是很 Local 实时翻译的 Apple 系统音色试听。请听一下这个声音是否自然、清楚，适合长时间播报。"
    } else if locale.starts_with("ja_") {
        "こんにちは。Hen Local リアルタイム翻訳の Apple システム音声テストです。自然で聞きやすい声か確認してください。"
    } else if locale.starts_with("fr_") {
        "Bonjour. Voici un test de la voix système Apple pour Hen Local. Vérifiez si cette voix est naturelle et facile à comprendre."
    } else {
        "Hello. This is an Apple system voice test for Hen Local Live Translator. Please check whether this voice sounds natural, clear, and comfortable for long listening."
    }
}

fn normalize_voice_id(voice: &str) -> &str {
    match voice {
        "apple-voice-1" | "apple-voice-2" | "apple-voice-3" | "apple-voice-4" | "apple-voice-5" => {
            voice
        }
        _ => "apple-voice-1",
    }
}

fn preferred_voice(language: &str, index: usize) -> Option<&'static str> {
    // These are the higher-quality Siri voices installed from macOS
    // Accessibility > Live Speech > Voice. They are exposed to `say` using
    // these exact names even though AVSpeechSynthesizer does not enumerate
    // them on current macOS releases.
    const EN: [&str; 5] = ["Voice 1", "Voice 2", "Voice 3", "Voice 4", "Voice 5"];
    const ZH: [&str; 2] = ["Yue (Premium)", "Tingting"];
    let voices = match language {
        "zh" => ZH.as_slice(),
        "en" => EN.as_slice(),
        _ => return None,
    };
    voices.get(index).copied()
}

fn voice_index(voice: &str) -> usize {
    voice
        .strip_prefix("apple-voice-")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1)
        .saturating_sub(1)
        .min(4)
}

fn select_voice(installed: &[(String, String)], language: &str, voice: &str) -> Option<String> {
    let preferred = preferred_voice(language, voice_index(voice))?;
    installed
        .iter()
        .find(|(name, _)| name == preferred)
        .map(|(name, _)| name.clone())
}

fn installed_voices() -> Vec<(String, String)> {
    available_voices()
        .into_iter()
        .map(|voice| (voice.name, voice.locale))
        .collect()
}

pub fn available_voices() -> Vec<SystemVoice> {
    let Ok(output) = Command::new("/usr/bin/say").arg("-v").arg("?").output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_system_voice_line)
        .collect()
}

#[cfg(test)]
fn parse_voice_line(line: &str) -> Option<(String, String)> {
    parse_system_voice_line(line).map(|voice| (voice.name, voice.locale))
}

fn parse_system_voice_line(line: &str) -> Option<SystemVoice> {
    let (identity, sample) = line.split_once('#').unwrap_or((line, ""));
    let fields: Vec<&str> = identity.split_whitespace().collect();
    let locale_index = fields.iter().position(|field| is_locale(field))?;
    (locale_index > 0).then(|| SystemVoice {
        name: fields[..locale_index].join(" "),
        locale: fields[locale_index].to_string(),
        sample: sample.trim().to_string(),
    })
}

fn is_locale(value: &str) -> bool {
    let Some((language, region)) = value.split_once('_') else {
        return false;
    };
    (2..=3).contains(&language.len())
        && !region.is_empty()
        && language.bytes().all(|byte| byte.is_ascii_alphabetic())
        && region.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiword_apple_voice_names() {
        assert_eq!(
            parse_voice_line("Eddy (Chinese (China mainland)) zh_CN # hello"),
            Some(("Eddy (Chinese (China mainland))".into(), "zh_CN".into()))
        );
    }

    #[test]
    fn never_falls_back_to_an_unselected_voice() {
        let installed = vec![("Fallback".into(), "fr_FR".into())];
        assert_eq!(select_voice(&installed, "fr", "apple-voice-2"), None);
    }

    #[test]
    fn parses_three_digit_regions() {
        assert_eq!(
            parse_voice_line("Majed ar_001 # hello"),
            Some(("Majed".into(), "ar_001".into()))
        );
    }

    #[test]
    fn maps_english_choices_to_siri_live_speech_voices() {
        assert_eq!(preferred_voice("en", 0), Some("Voice 1"));
        assert_eq!(preferred_voice("en", 4), Some("Voice 5"));
    }

    #[test]
    fn maps_first_chinese_choice_to_yue_premium() {
        assert_eq!(preferred_voice("zh", 0), Some("Yue (Premium)"));
        assert_eq!(preferred_voice("zh", 1), Some("Tingting"));
        assert_eq!(preferred_voice("zh", 2), None);
    }

    #[test]
    fn has_no_unapproved_japanese_or_french_voice() {
        assert_eq!(preferred_voice("ja", 0), None);
        assert_eq!(preferred_voice("fr", 0), None);
    }
}
