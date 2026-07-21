use std::{
    collections::VecDeque,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread,
    time::Duration,
};

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
                let selected = select_voice(&installed, &language, &voice);
                match spawn_say(selected.as_deref(), &text) {
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

fn spawn_say(voice: Option<&str>, text: &str) -> Result<Child, std::io::Error> {
    let mut command = Command::new("/usr/bin/say");
    if let Some(voice) = voice {
        command.arg("-v").arg(voice);
    }
    command
        .arg(text)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

pub fn preview(voice: &str, language: &str, text: &str) -> Result<Child, String> {
    let installed = installed_voices();
    let selected = select_voice(&installed, language, normalize_voice_id(voice));
    spawn_say(selected.as_deref(), text)
        .map_err(|error| format!("Could not play Apple voice preview: {error}"))
}

fn normalize_voice_id(voice: &str) -> &str {
    match voice {
        "apple-voice-1" | "apple-voice-2" | "apple-voice-3" | "apple-voice-4" | "apple-voice-5" => {
            voice
        }
        _ => "apple-voice-1",
    }
}

fn preferred_voice(language: &str, index: usize) -> &'static str {
    const EN: [&str; 5] = ["Samantha", "Daniel", "Karen", "Moira", "Rishi"];
    const ZH: [&str; 5] = [
        "Tingting",
        "Eddy (Chinese (China mainland))",
        "Flo (Chinese (China mainland))",
        "Reed (Chinese (China mainland))",
        "Shelley (Chinese (China mainland))",
    ];
    const JA: [&str; 5] = [
        "Kyoko",
        "Otoya",
        "Eddy (Japanese (Japan))",
        "Flo (Japanese (Japan))",
        "Reed (Japanese (Japan))",
    ];
    const FR: [&str; 5] = [
        "Thomas",
        "Amelie",
        "Jacques",
        "Eddy (French (France))",
        "Flo (French (France))",
    ];
    let voices = match language {
        "zh" => &ZH,
        "ja" => &JA,
        "fr" => &FR,
        _ => &EN,
    };
    voices[index.min(4)]
}

fn locale_prefix(language: &str) -> &'static str {
    match language {
        "zh" => "zh_",
        "ja" => "ja_",
        "fr" => "fr_",
        _ => "en_",
    }
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
    let preferred = preferred_voice(language, voice_index(voice));
    installed
        .iter()
        .find(|(name, _)| name == preferred)
        .or_else(|| {
            let prefix = locale_prefix(language);
            installed
                .iter()
                .find(|(_, locale)| locale.starts_with(prefix))
        })
        .map(|(name, _)| name.clone())
}

fn installed_voices() -> Vec<(String, String)> {
    let Ok(output) = Command::new("/usr/bin/say").arg("-v").arg("?").output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_voice_line)
        .collect()
}

fn parse_voice_line(line: &str) -> Option<(String, String)> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    let locale_index = fields
        .iter()
        .position(|field| field.len() == 5 && field.as_bytes().get(2) == Some(&b'_'))?;
    (locale_index > 0).then(|| {
        (
            fields[..locale_index].join(" "),
            fields[locale_index].to_string(),
        )
    })
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
    fn falls_back_to_an_installed_voice_for_the_language() {
        let installed = vec![("Fallback".into(), "fr_FR".into())];
        assert_eq!(
            select_voice(&installed, "fr", "apple-voice-2"),
            Some("Fallback".into())
        );
    }
}
