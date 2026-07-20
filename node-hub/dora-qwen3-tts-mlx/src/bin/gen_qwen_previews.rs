//! Generate the branded Hen Local preview WAVs used by the desktop app.
//!
//! By default files are written to the model cache. Set
//! `QWEN3_TTS_PREVIEW_OUTPUT_DIR` to refresh another preview bundle, including
//! the repository's `previews` directory used by development and packaging.

use anyhow::{anyhow, Context, Result};
use qwen3_tts_mlx::{normalize_audio, save_wav, SynthesizeOptions, Synthesizer};
use std::path::{Path, PathBuf};

fn resolve_qwen_root() -> PathBuf {
    if let Ok(value) = std::env::var("QWEN3_TTS_MODEL_ROOT") {
        return PathBuf::from(value);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".OminiX/models/qwen3-tts-mlx")
}

fn resolve_customvoice_model_dir() -> PathBuf {
    std::env::var("QWEN3_TTS_CUSTOMVOICE_MODEL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| resolve_qwen_root().join("Qwen3-TTS-12Hz-1.7B-CustomVoice-8bit"))
}

fn resolve_base_model_dir() -> PathBuf {
    std::env::var("QWEN3_TTS_BASE_MODEL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| resolve_qwen_root().join("Qwen3-TTS-12Hz-1.7B-Base-8bit"))
}

fn resolve_output_dir() -> PathBuf {
    std::env::var("QWEN3_TTS_PREVIEW_OUTPUT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| resolve_qwen_root().join("previews"))
}

fn voice_is_selected(id: &str) -> bool {
    std::env::var("QWEN3_TTS_PREVIEW_VOICES")
        .ok()
        .map(|voices| {
            voices
                .split(',')
                .map(str::trim)
                .any(|voice| voice.eq_ignore_ascii_case(id))
        })
        .unwrap_or(true)
}

struct SpeakerSpec {
    id: &'static str,
    language: &'static str,
    text: &'static str,
}

const SPEAKERS: &[SpeakerSpec] = &[
    SpeakerSpec {
        id: "vivian",
        language: "chinese",
        text: "你好，欢迎使用很 Local，我是薇薇安。实时翻译已经准备好了。",
    },
    SpeakerSpec {
        id: "serena",
        language: "chinese",
        text: "你好，欢迎使用很 Local，我是赛琳娜。让我们开始自然地交流吧。",
    },
    SpeakerSpec {
        id: "ryan",
        language: "english",
        text: "Hello, welcome to Hen Local. I'm Ryan, and live translation is ready.",
    },
    SpeakerSpec {
        id: "aiden",
        language: "english",
        text: "Hello, welcome to Hen Local. I'm Aiden. Let's bring every conversation closer.",
    },
];

struct CloneSpec {
    id: &'static str,
    language: &'static str,
    text: &'static str,
}

const CLONES: &[CloneSpec] = &[
    CloneSpec {
        id: "baiyang",
        language: "chinese",
        text: "你好，这里是很 Local。实时翻译已经准备好了，让我们开始自然地交流吧。",
    },
    CloneSpec {
        id: "yangyang",
        language: "chinese",
        text: "你好，这里是很 Local。跨语言交流，现在就开始。",
    },
    CloneSpec {
        id: "maple",
        language: "english",
        text:
            "Hello, this is Hen Local. Live translation is ready, so let's start the conversation.",
    },
    CloneSpec {
        id: "juniper",
        language: "english",
        text: "Hello, this is Hen Local. Let's make every conversation feel closer.",
    },
];

fn load_wav_mono_24k(path: &Path) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)
        .with_context(|| format!("failed to open reference WAV: {}", path.display()))?;
    let wav_spec = reader.spec();
    let channels = wav_spec.channels.max(1) as usize;
    let mut interleaved = Vec::new();
    match wav_spec.sample_format {
        hound::SampleFormat::Float => {
            for sample in reader.samples::<f32>() {
                interleaved.push(sample.context("invalid float WAV sample")?);
            }
        }
        hound::SampleFormat::Int if wav_spec.bits_per_sample <= 16 => {
            for sample in reader.samples::<i16>() {
                interleaved
                    .push(sample.context("invalid i16 WAV sample")? as f32 / i16::MAX as f32);
            }
        }
        hound::SampleFormat::Int => {
            let scale = ((1_i64 << (wav_spec.bits_per_sample.saturating_sub(1) as u32)) - 1) as f32;
            for sample in reader.samples::<i32>() {
                interleaved
                    .push(sample.context("invalid integer WAV sample")? as f32 / scale.max(1.0));
            }
        }
    }
    if interleaved.is_empty() {
        return Err(anyhow!("reference WAV is empty: {}", path.display()));
    }
    let mono = interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect::<Vec<_>>();
    let mut audio = if wav_spec.sample_rate == 24_000 {
        mono
    } else {
        let out_len = ((mono.len() as f64) * 24_000.0 / wav_spec.sample_rate as f64)
            .round()
            .max(1.0) as usize;
        (0..out_len)
            .map(|index| {
                let position = index as f64 * wav_spec.sample_rate as f64 / 24_000.0;
                let left = (position.floor() as usize).min(mono.len() - 1);
                let right = (left + 1).min(mono.len() - 1);
                let fraction = (position - position.floor()) as f32;
                mono[left] + (mono[right] - mono[left]) * fraction
            })
            .collect()
    };
    audio.truncate(8 * 24_000);
    Ok(audio)
}

fn resolve_voice_dir(id: &str) -> Result<PathBuf> {
    let cached = resolve_qwen_root().join("voices").join(id);
    if cached.join("ref.wav").is_file() {
        return Ok(cached);
    }
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("voices")
        .join(id);
    if bundled.join("ref.wav").is_file() {
        return Ok(bundled);
    }
    Err(anyhow!("reference voice is not installed: {id}"))
}

fn save_preview(samples: Vec<f32>, sample_rate: u32, path: &Path) -> Result<()> {
    if samples.is_empty() {
        return Err(anyhow!("synthesis returned no audio"));
    }
    let samples = normalize_audio(&samples, 0.95);
    save_wav(&samples, sample_rate, path.to_string_lossy().as_ref())?;
    eprintln!("OK ({:.1}s)", samples.len() as f32 / sample_rate as f32);
    Ok(())
}

fn main() -> Result<()> {
    let custom_model_dir = resolve_customvoice_model_dir();
    let base_model_dir = resolve_base_model_dir();
    let out_dir = resolve_output_dir();
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create output directory: {}", out_dir.display()))?;

    let mut generated = 0usize;
    eprintln!(
        "Loading CustomVoice model from {} ...",
        custom_model_dir.display()
    );
    let mut custom =
        Synthesizer::load(&custom_model_dir).context("failed to load CustomVoice model")?;
    for spec in SPEAKERS {
        if !voice_is_selected(spec.id) {
            continue;
        }
        let out_path = out_dir.join(format!("{}.wav", spec.id));
        eprint!(
            "  [{}] ({}) -> {} ... ",
            spec.id,
            spec.language,
            out_path.display()
        );
        let options = SynthesizeOptions {
            speaker: spec.id,
            language: spec.language,
            seed: Some(42),
            ..Default::default()
        };
        let samples = custom.synthesize(spec.text, &options)?;
        save_preview(samples, custom.sample_rate, &out_path)?;
        generated += 1;
    }
    drop(custom);

    eprintln!("Loading Base model from {} ...", base_model_dir.display());
    let mut base = Synthesizer::load(&base_model_dir).context("failed to load Base model")?;
    for spec in CLONES {
        if !voice_is_selected(spec.id) {
            continue;
        }
        let voice_dir = resolve_voice_dir(spec.id)?;
        let reference_audio = load_wav_mono_24k(&voice_dir.join("ref.wav"))?;
        let out_path = out_dir.join(format!("{}.wav", spec.id));
        eprint!(
            "  [{}] ({}) -> {} ... ",
            spec.id,
            spec.language,
            out_path.display()
        );
        let options = SynthesizeOptions {
            language: spec.language,
            seed: Some(42),
            ..Default::default()
        };
        let samples =
            base.synthesize_voice_clone(spec.text, &reference_audio, spec.language, &options)?;
        save_preview(samples, base.sample_rate, &out_path)?;
        generated += 1;
    }

    eprintln!(
        "Generated {} Hen Local previews in {}",
        generated,
        out_dir.display()
    );
    Ok(())
}
