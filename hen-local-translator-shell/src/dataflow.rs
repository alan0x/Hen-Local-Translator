use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub struct RenderOptions<'a> {
    pub source_language: &'a str,
    pub target_language: &'a str,
    pub system_audio: bool,
    pub spoken_translation: bool,
    pub spoken_voice: &'a str,
}

pub fn render_translation_dataflow(
    resource_dir: Option<&Path>,
    options: RenderOptions<'_>,
) -> Result<PathBuf, String> {
    ensure_executable_directory_on_path();

    let template_path = resolve_template(resource_dir)
        .ok_or_else(|| "translation_qwen35.yml was not found".to_string())?;
    let template = fs::read_to_string(&template_path)
        .map_err(|error| format!("Could not read translation dataflow: {error}"))?;

    let asr_path = resolve_binary("dora-qwen3-asr").ok_or_else(|| {
        "dora-qwen3-asr is missing; build it before starting translation".to_string()
    })?;
    let translator_path = resolve_binary("dora-qwen35-translator").ok_or_else(|| {
        "dora-qwen35-translator is missing; build it before starting translation".to_string()
    })?;

    let (start_frames, end_frames, end_ms, question_end_ms, min_segment_ms, start_rms, end_rms) =
        match options.source_language {
            "zh" => (5, 10, 420, 1200, 420, 0.018_f32, 0.010_f32),
            "en" => (4, 30, 300, 900, 300, 0.015_f32, 0.009_f32),
            "fr" => (4, 10, 320, 1000, 320, 0.016_f32, 0.009_f32),
            _ => (4, 10, 320, 1000, 320, 0.016_f32, 0.009_f32),
        };
    let (start_rms, end_rms) = if options.system_audio {
        (start_rms * 0.3, end_rms * 0.3)
    } else {
        (start_rms, end_rms)
    };

    let passthrough = if options.target_language == "none" {
        "1"
    } else {
        "0"
    };
    let mut rendered = template
        .replace("__TRANSLATION_SRC_LANG__", options.source_language)
        .replace("__TRANSLATION_TGT_LANG__", options.target_language)
        .replace("__TRANSLATOR_PASSTHROUGH__", passthrough)
        .replace("__ASR_BIN_PATH__", &asr_path.to_string_lossy())
        .replace(
            "__TRANSLATOR_BIN_PATH__",
            &translator_path.to_string_lossy(),
        )
        .replace("__SPEECH_START_FRAMES__", &start_frames.to_string())
        .replace("__SPEECH_END_FRAMES__", &end_frames.to_string())
        .replace("__SPEECH_END_MS__", &end_ms.to_string())
        .replace("__QUESTION_END_SILENCE_MS__", &question_end_ms.to_string())
        .replace("__MIN_SEGMENT_MS__", &min_segment_ms.to_string())
        .replace("__MAX_SEGMENT_MS__", "8000")
        .replace("__START_RMS_THRESHOLD__", &format!("{start_rms:.4}"))
        .replace("__END_RMS_THRESHOLD__", &format!("{end_rms:.4}"));

    let tts_path = options
        .spoken_translation
        .then(|| resolve_binary("qwen-tts-node"))
        .flatten()
        .filter(|_| qwen_tts_models_ready());
    rendered = if let Some(tts_path) = tts_path {
        rendered
            .replace("__TTS_BIN_PATH__", &tts_path.to_string_lossy())
            .replace("__SPOKEN_VOICE__", options.spoken_voice)
    } else {
        strip_optional_tts(&rendered)
    };

    let rendered = absolutize_dataflow_paths(&template_path, &rendered);
    let output = env::temp_dir().join("hen_local_translation_dataflow.yml");
    fs::write(&output, rendered)
        .map_err(|error| format!("Could not write rendered translation dataflow: {error}"))?;
    Ok(output)
}

fn resolve_template(resource_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = env::var_os("MOXIN_TRANSLATION_DATAFLOW_PATH")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
    {
        return Some(path);
    }

    let mut candidates = Vec::new();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest_dir.join("dataflow/translation_qwen35.yml"));
    if let Some(resource_dir) = resource_dir {
        candidates.extend([
            resource_dir.join("translation_qwen35.yml"),
            resource_dir.join("dataflow/translation_qwen35.yml"),
            resource_dir.join("_up_/hen-local-translator-shell/dataflow/translation_qwen35.yml"),
        ]);
    }
    if let Some(resources) = env::var_os("HEN_LOCAL_APP_RESOURCES") {
        candidates.push(PathBuf::from(resources).join("dataflow/translation_qwen35.yml"));
    }
    candidates.push(PathBuf::from(
        "hen-local-translator-shell/dataflow/translation_qwen35.yml",
    ));
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".OminiX/dataflows/translation_qwen35.yml"));
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn resolve_binary(name: &str) -> Option<PathBuf> {
    let executable_name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let mut candidates = Vec::new();
    if let Some(directory) = env::var_os("MOXIN_DORA_BIN_DIR") {
        candidates.push(PathBuf::from(directory).join(&executable_name));
    }
    for variable in ["MOXIN_DORA_TARGET_DIR", "CARGO_TARGET_DIR"] {
        if let Some(directory) = env::var_os(variable) {
            let directory = PathBuf::from(directory);
            candidates.push(directory.join("debug").join(&executable_name));
            candidates.push(directory.join("release").join(&executable_name));
            candidates.push(directory.join(&executable_name));
        }
    }
    if let Ok(executable) = env::current_exe() {
        if let Some(directory) = executable.parent() {
            candidates.push(directory.join(&executable_name));
        }
    }
    candidates.extend([
        PathBuf::from("target/debug").join(&executable_name),
        PathBuf::from("target/release").join(&executable_name),
    ]);

    candidates.into_iter().find_map(|candidate| {
        candidate
            .is_file()
            .then(|| candidate.canonicalize().unwrap_or(candidate))
    })
}

fn qwen_tts_models_ready() -> bool {
    if let Some(directory) = env::var_os("QWEN3_TTS_CUSTOMVOICE_MODEL_DIR") {
        return PathBuf::from(directory).join("config.json").is_file();
    }
    dirs::home_dir()
        .map(|home| {
            home.join(
                ".OminiX/models/qwen3-tts-mlx/Qwen3-TTS-12Hz-1.7B-CustomVoice-8bit/config.json",
            )
            .is_file()
        })
        .unwrap_or(false)
}

fn strip_optional_tts(content: &str) -> String {
    let mut output = String::with_capacity(content.len());
    let mut skipping = false;
    for line in content.lines() {
        if line.contains("# TTS-BEGIN") {
            skipping = true;
            continue;
        }
        if line.contains("# TTS-END") {
            skipping = false;
            continue;
        }
        if !skipping {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

fn absolutize_dataflow_paths(template_path: &Path, content: &str) -> String {
    let base_dir = template_path.parent().unwrap_or_else(|| Path::new("."));
    let mut output = String::with_capacity(content.len() + 256);
    let trailing_newline = content.ends_with('\n');

    for line in content.lines() {
        let trimmed = line.trim_start();
        let indent = &line[..line.len().saturating_sub(trimmed.len())];
        if let Some(raw_value) = trimmed.strip_prefix("path:") {
            let raw_value = raw_value.trim();
            let (path_value, quoted) = if raw_value.len() >= 2
                && ((raw_value.starts_with('"') && raw_value.ends_with('"'))
                    || (raw_value.starts_with('\'') && raw_value.ends_with('\'')))
            {
                (&raw_value[1..raw_value.len() - 1], true)
            } else {
                (raw_value, false)
            };
            if path_value != "dynamic"
                && !Path::new(path_value).is_absolute()
                && path_value.contains('/')
            {
                let candidate = base_dir.join(path_value);
                if candidate.exists() {
                    let resolved = candidate.canonicalize().unwrap_or(candidate);
                    if quoted {
                        output.push_str(&format!("{indent}path: \"{}\"\n", resolved.display()));
                    } else {
                        output.push_str(&format!("{indent}path: {}\n", resolved.display()));
                    }
                    continue;
                }
            }
        }
        output.push_str(line);
        output.push('\n');
    }
    if !trailing_newline {
        output.pop();
    }
    output
}

fn ensure_executable_directory_on_path() {
    let Ok(executable) = env::current_exe() else {
        return;
    };
    let Some(directory) = executable.parent() else {
        return;
    };
    let mut paths: Vec<PathBuf> = env::var_os("PATH")
        .map(|value| env::split_paths(&value).collect())
        .unwrap_or_default();
    if paths.iter().any(|path| path == directory) {
        return;
    }
    paths.insert(0, directory.to_path_buf());
    if let Ok(joined) = env::join_paths(paths) {
        env::set_var("PATH", joined);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_translation_template_independent_of_working_directory() {
        let template = resolve_template(None).expect("translation template should resolve");
        assert!(template.is_file());
        assert_eq!(
            template.file_name().and_then(|name| name.to_str()),
            Some("translation_qwen35.yml")
        );
    }

    #[test]
    fn renders_translation_dataflow_from_shell_working_directory() {
        let binary_dir =
            env::temp_dir().join(format!("hen-local-dataflow-test-{}", std::process::id()));
        fs::create_dir_all(&binary_dir).expect("temporary binary directory should be created");
        for name in ["dora-qwen3-asr", "dora-qwen35-translator"] {
            let filename = if cfg!(windows) {
                format!("{name}.exe")
            } else {
                name.to_string()
            };
            fs::write(binary_dir.join(filename), []).expect("placeholder binary should be written");
        }

        let previous_binary_dir = env::var_os("MOXIN_DORA_BIN_DIR");
        env::set_var("MOXIN_DORA_BIN_DIR", &binary_dir);
        let rendered = render_translation_dataflow(
            None,
            RenderOptions {
                source_language: "zh",
                target_language: "en",
                system_audio: true,
                spoken_translation: false,
                spoken_voice: "ryan",
            },
        );
        if let Some(previous) = previous_binary_dir {
            env::set_var("MOXIN_DORA_BIN_DIR", previous);
        } else {
            env::remove_var("MOXIN_DORA_BIN_DIR");
        }
        let rendered = rendered.expect("translation dataflow should render");
        let content = fs::read_to_string(rendered).expect("rendered dataflow should be readable");
        assert!(!content.contains("__TRANSLATION_SRC_LANG__"));
        assert!(!content.contains("__TRANSLATION_TGT_LANG__"));
        assert!(!content.contains("__ASR_BIN_PATH__"));
        assert!(!content.contains("__TRANSLATOR_BIN_PATH__"));
        let _ = fs::remove_dir_all(binary_dir);
    }

    #[test]
    fn removes_optional_tts_block_without_touching_translation_nodes() {
        let input = "before\n# TTS-BEGIN\ntts\n# TTS-END\nafter\n";
        assert_eq!(strip_optional_tts(input), "before\nafter\n");
    }

    #[test]
    fn spoken_translation_uses_translator_output_directly() {
        let template = resolve_template(None).expect("translation template should resolve");
        let content = fs::read_to_string(template).expect("template should be readable");
        assert!(content.contains("text: translator/translation"));
    }
}
