use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use std::{
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
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
        output_device: Option<String>,
    },
    Speak(String),
    Stop,
    Shutdown,
}

struct SynthesisTask {
    generation: u64,
    voice: String,
    output_device: Option<String>,
    text: String,
}

enum SynthesisCommand {
    Generate(SynthesisTask),
    Shutdown,
}

enum PlaybackCommand {
    Play {
        generation: u64,
        output_device: Option<String>,
        path: PathBuf,
    },
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

    pub fn configure(
        &self,
        enabled: bool,
        language: &str,
        voice: &str,
        output_device: Option<&str>,
    ) {
        let _ = self.sender.send(SpeechCommand::Configure {
            enabled,
            language: language.to_string(),
            voice: normalize_voice_id(voice).to_string(),
            output_device: output_device.map(str::to_string),
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
    let generation = Arc::new(AtomicU64::new(1));
    let (synthesis_sender, synthesis_receiver) = mpsc::channel();
    let (playback_sender, playback_receiver) = mpsc::channel();
    let synthesis_generation = Arc::clone(&generation);
    let playback_generation = Arc::clone(&generation);
    thread::spawn(move || {
        synthesis_worker(synthesis_receiver, playback_sender, synthesis_generation)
    });
    thread::spawn(move || playback_worker(playback_receiver, playback_generation));

    let mut enabled = false;
    let mut language = String::from("en");
    let mut voice = String::from("apple-voice-1");
    let mut output_device = None;

    while let Ok(command) = receiver.recv() {
        match command {
            SpeechCommand::Configure {
                enabled: next_enabled,
                language: next_language,
                voice: next_voice,
                output_device: next_output_device,
            } => {
                generation.fetch_add(1, Ordering::SeqCst);
                enabled = next_enabled;
                language = next_language;
                voice = next_voice;
                output_device = next_output_device;
            }
            SpeechCommand::Speak(text) if enabled => {
                let Some(selected) = select_voice(&installed, &language, &voice) else {
                    log::warn!("Selected Hen Local voice is unavailable: {language} / {voice}");
                    continue;
                };
                let _ = synthesis_sender.send(SynthesisCommand::Generate(SynthesisTask {
                    generation: generation.load(Ordering::SeqCst),
                    voice: selected,
                    output_device: output_device.clone(),
                    text,
                }));
            }
            SpeechCommand::Speak(_) => {}
            SpeechCommand::Stop => {
                generation.fetch_add(1, Ordering::SeqCst);
                enabled = false;
            }
            SpeechCommand::Shutdown => {
                generation.fetch_add(1, Ordering::SeqCst);
                let _ = synthesis_sender.send(SynthesisCommand::Shutdown);
                break;
            }
        }
    }
    generation.fetch_add(1, Ordering::SeqCst);
    let _ = synthesis_sender.send(SynthesisCommand::Shutdown);
}

static SPEECH_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn synthesis_worker(
    receiver: Receiver<SynthesisCommand>,
    playback_sender: Sender<PlaybackCommand>,
    current_generation: Arc<AtomicU64>,
) {
    while let Ok(command) = receiver.recv() {
        match command {
            SynthesisCommand::Generate(task) => {
                if task.generation != current_generation.load(Ordering::SeqCst) {
                    continue;
                }
                match synthesize_wave(&task.voice, &task.text) {
                    Ok(path) if task.generation == current_generation.load(Ordering::SeqCst) => {
                        let _ = playback_sender.send(PlaybackCommand::Play {
                            generation: task.generation,
                            output_device: task.output_device,
                            path,
                        });
                    }
                    Ok(path) => {
                        let _ = fs::remove_file(path);
                    }
                    Err(error) => log::error!("Could not synthesize Apple speech: {error}"),
                }
            }
            SynthesisCommand::Shutdown => break,
        }
    }
    let _ = playback_sender.send(PlaybackCommand::Shutdown);
}

fn synthesize_wave(voice: &str, text: &str) -> Result<PathBuf, String> {
    let sequence = SPEECH_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "hen-local-speech-{}-{sequence}.wav",
        std::process::id()
    ));
    let status = Command::new("/usr/bin/say")
        .arg("-v")
        .arg(voice)
        .arg("-o")
        .arg(&path)
        .arg("--file-format=WAVE")
        .arg("--data-format=LEI16@24000")
        .arg("--channels=1")
        .arg(text)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| error.to_string())?;
    if status.success() {
        Ok(path)
    } else {
        let _ = fs::remove_file(&path);
        Err(format!("say exited with {status}"))
    }
}

fn playback_worker(receiver: Receiver<PlaybackCommand>, current_generation: Arc<AtomicU64>) {
    while let Ok(command) = receiver.recv() {
        match command {
            PlaybackCommand::Play {
                generation,
                output_device,
                path,
            } => {
                if generation == current_generation.load(Ordering::SeqCst) {
                    if let Err(error) = play_wave(
                        &path,
                        output_device.as_deref(),
                        generation,
                        Arc::clone(&current_generation),
                    ) {
                        log::error!("Could not play Apple speech: {error}");
                    }
                }
                let _ = fs::remove_file(path);
            }
            PlaybackCommand::Shutdown => break,
        }
    }
}

fn play_wave(
    path: &PathBuf,
    selected_device: Option<&str>,
    generation: u64,
    current_generation: Arc<AtomicU64>,
) -> Result<(), String> {
    let mut reader = hound::WavReader::open(path).map_err(|error| error.to_string())?;
    let specification = reader.spec();
    let source_channels = usize::from(specification.channels.max(1));
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|sample| sample.map(|value| f32::from(value) / f32::from(i16::MAX)))
        .collect::<Result<_, _>>()
        .map_err(|error| error.to_string())?;

    let host = cpal::default_host();
    let device = selected_device
        .and_then(|selected| {
            host.output_devices().ok()?.find(|device| {
                device
                    .name()
                    .is_ok_and(|device_name| device_name == selected)
            })
        })
        .or_else(|| host.default_output_device())
        .ok_or_else(|| "No audio output device is available".to_string())?;
    if let Some(selected) = selected_device {
        if device.name().ok().as_deref() != Some(selected) {
            log::warn!("Speech output device disconnected; using the system default: {selected}");
        }
    }

    let supported = device
        .default_output_config()
        .map_err(|error| error.to_string())?;
    let config = supported.config();
    let output = Arc::new(resample_for_output(
        &samples,
        source_channels,
        specification.sample_rate,
        usize::from(config.channels),
        config.sample_rate.0,
    ));
    let finished = Arc::new(AtomicBool::new(false));
    let error_callback = |error| log::error!("Apple speech audio stream error: {error}");

    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            let output = Arc::clone(&output);
            let finished = Arc::clone(&finished);
            let active_generation = Arc::clone(&current_generation);
            let mut cursor = 0usize;
            let mut draining = false;
            device.build_output_stream(
                &config,
                move |buffer: &mut [f32], _| {
                    if draining || generation != active_generation.load(Ordering::Relaxed) {
                        buffer.fill(0.0);
                        finished.store(true, Ordering::Release);
                        return;
                    }
                    for sample in buffer.iter_mut() {
                        *sample = output.get(cursor).copied().unwrap_or(0.0);
                        cursor += usize::from(cursor < output.len());
                    }
                    draining = cursor >= output.len();
                },
                error_callback,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let output = Arc::clone(&output);
            let finished = Arc::clone(&finished);
            let active_generation = Arc::clone(&current_generation);
            let mut cursor = 0usize;
            let mut draining = false;
            device.build_output_stream(
                &config,
                move |buffer: &mut [i16], _| {
                    if draining || generation != active_generation.load(Ordering::Relaxed) {
                        buffer.fill(0);
                        finished.store(true, Ordering::Release);
                        return;
                    }
                    for sample in buffer.iter_mut() {
                        let value = output.get(cursor).copied().unwrap_or(0.0);
                        *sample = (value.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
                        cursor += usize::from(cursor < output.len());
                    }
                    draining = cursor >= output.len();
                },
                error_callback,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let output = Arc::clone(&output);
            let finished = Arc::clone(&finished);
            let active_generation = Arc::clone(&current_generation);
            let mut cursor = 0usize;
            let mut draining = false;
            device.build_output_stream(
                &config,
                move |buffer: &mut [u16], _| {
                    if draining || generation != active_generation.load(Ordering::Relaxed) {
                        buffer.fill(u16::MAX / 2);
                        finished.store(true, Ordering::Release);
                        return;
                    }
                    for sample in buffer.iter_mut() {
                        let value = output.get(cursor).copied().unwrap_or(0.0);
                        *sample =
                            ((value.clamp(-1.0, 1.0) * 0.5 + 0.5) * f32::from(u16::MAX)) as u16;
                        cursor += usize::from(cursor < output.len());
                    }
                    draining = cursor >= output.len();
                },
                error_callback,
                None,
            )
        }
        format => return Err(format!("Unsupported output sample format: {format:?}")),
    }
    .map_err(|error| error.to_string())?;

    stream.play().map_err(|error| error.to_string())?;
    while !finished.load(Ordering::Acquire)
        && generation == current_generation.load(Ordering::SeqCst)
    {
        thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

fn resample_for_output(
    samples: &[f32],
    source_channels: usize,
    source_rate: u32,
    output_channels: usize,
    output_rate: u32,
) -> Vec<f32> {
    if samples.is_empty() || source_rate == 0 || output_channels == 0 {
        return Vec::new();
    }
    let mono: Vec<f32> = samples
        .chunks(source_channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect();
    let output_frames =
        ((mono.len() as u64 * u64::from(output_rate)) / u64::from(source_rate)) as usize;
    let ratio = f64::from(source_rate) / f64::from(output_rate);
    let mut output = Vec::with_capacity(output_frames * output_channels);
    for frame in 0..output_frames {
        let position = frame as f64 * ratio;
        let left = position.floor() as usize;
        let right = (left + 1).min(mono.len() - 1);
        let fraction = (position - left as f64) as f32;
        let value = mono[left] + (mono[right] - mono[left]) * fraction;
        output.extend(std::iter::repeat_n(value, output_channels));
    }
    output
}

fn spawn_say(
    voice: &str,
    output_device: Option<&str>,
    text: &str,
) -> Result<Child, std::io::Error> {
    let mut command = Command::new("/usr/bin/say");
    command.arg("-v").arg(voice);
    if let Some(device_id) = available_output_device_id(output_device) {
        command.arg("-a").arg(device_id);
    }
    command
        .arg(text)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

pub fn preview(
    voice: &str,
    language: &str,
    output_device: Option<&str>,
    text: &str,
) -> Result<Child, String> {
    let installed = installed_voices();
    let selected = select_voice(&installed, language, normalize_voice_id(voice))
        .ok_or_else(|| format!("Selected Hen Local voice is unavailable for {language}"))?;
    spawn_say(&selected, output_device, text)
        .map_err(|error| format!("Could not play Apple voice preview: {error}"))
}

pub fn preview_named(name: &str, locale: &str) -> Result<Child, String> {
    let voices = available_voices();
    let voice = voices
        .iter()
        .find(|voice| voice.name == name && voice.locale == locale)
        .ok_or_else(|| format!("Apple system voice is unavailable: {name} ({locale})"))?;
    let sample = audition_text(&voice.locale);
    spawn_say(&voice.name, None, sample)
        .map_err(|error| format!("Could not play Apple voice preview: {error}"))
}

fn available_output_device_id(selected: Option<&str>) -> Option<String> {
    let selected = selected?.trim();
    if selected.is_empty() {
        return None;
    }
    let device_id = output_devices()
        .into_iter()
        .find_map(|(id, name)| (name == selected).then_some(id));
    if device_id.is_none() {
        log::warn!("Apple speech output device is unavailable; using system default: {selected}");
    }
    device_id
}

pub fn output_device_names() -> Vec<String> {
    let mut devices: Vec<String> = output_devices().into_iter().map(|(_, name)| name).collect();
    devices.sort();
    devices.dedup();
    devices
}

fn output_devices() -> Vec<(String, String)> {
    let Ok(output) = Command::new("/usr/bin/say").arg("-a").arg("?").output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_output_device_line)
        .collect()
}

fn parse_output_device_line(line: &str) -> Option<(String, String)> {
    let (id, name) = line.trim().split_once(char::is_whitespace)?;
    let name = name.trim();
    (!id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit()) && !name.is_empty())
        .then(|| (id.to_string(), name.to_string()))
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

fn preferred_voice(language: &str, index: usize) -> Option<(&'static str, &'static str)> {
    // These are the higher-quality Siri voices installed from macOS
    // Accessibility > Live Speech > Voice. They are exposed to `say` using
    // these exact names even though AVSpeechSynthesizer does not enumerate
    // them on current macOS releases.
    const EN: [(&str, &str); 5] = [
        ("Voice 1", "en_US"),
        ("Voice 2", "en_US"),
        ("Voice 3", "en_US"),
        ("Voice 4", "en_US"),
        ("Voice 5", "en_US"),
    ];
    const ZH: [(&str, &str); 2] = [("Yue (Premium)", "zh_CN"), ("Tingting", "zh_CN")];
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
    let (preferred_name, preferred_locale) = preferred_voice(language, voice_index(voice))?;
    installed
        .iter()
        .find(|(name, locale)| name == preferred_name && locale == preferred_locale)
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

pub fn ensure_voice_available(language: &str, voice: &str) -> Result<(), String> {
    let installed = installed_voices();
    if select_voice(&installed, language, normalize_voice_id(voice)).is_some() {
        return Ok(());
    }
    let (name, locale) = preferred_voice(language, voice_index(normalize_voice_id(voice)))
        .ok_or_else(|| format!("Spoken translation is not available for {language}"))?;
    Err(format!(
        "Selected Apple voice is not installed: {name} ({locale}). Download it in macOS Accessibility settings before enabling spoken translation."
    ))
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
    fn parses_say_output_devices() {
        assert_eq!(
            parse_output_device_line("  94 MacBook Pro Speakers"),
            Some(("94".into(), "MacBook Pro Speakers".into()))
        );
        assert_eq!(parse_output_device_line("not a device"), None);
    }

    #[test]
    fn resamples_mono_audio_and_expands_output_channels() {
        let output = resample_for_output(&[0.0, 1.0], 1, 2, 2, 4);
        assert_eq!(output.len(), 8);
        assert_eq!(&output[..4], &[0.0, 0.0, 0.5, 0.5]);
    }

    #[test]
    fn maps_english_choices_to_siri_live_speech_voices() {
        assert_eq!(preferred_voice("en", 0), Some(("Voice 1", "en_US")));
        assert_eq!(preferred_voice("en", 4), Some(("Voice 5", "en_US")));
    }

    #[test]
    fn maps_first_chinese_choice_to_yue_premium() {
        assert_eq!(preferred_voice("zh", 0), Some(("Yue (Premium)", "zh_CN")));
        assert_eq!(preferred_voice("zh", 1), Some(("Tingting", "zh_CN")));
        assert_eq!(preferred_voice("zh", 2), None);
    }

    #[test]
    fn rejects_same_named_voice_from_wrong_locale() {
        let installed = vec![
            ("Voice 4".into(), "fr_FR".into()),
            ("Voice 4".into(), "en_GB".into()),
        ];
        assert_eq!(
            select_voice(&installed, "en", "apple-voice-4"),
            None,
            "Voice 4 must be the US English variant"
        );
    }

    #[test]
    fn has_no_unapproved_japanese_or_french_voice() {
        assert_eq!(preferred_voice("ja", 0), None);
        assert_eq!(preferred_voice("fr", 0), None);
    }
}
