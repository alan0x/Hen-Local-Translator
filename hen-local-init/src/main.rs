//! # hen-local-init
//!
//! First-run model downloader for Hen Local Translator.
//! Replaces the conda/Python bootstrap: downloads ASR and translator models
//! directly via HTTP, with ModelScope as the default provider and Hugging Face
//! available as a fallback.
//!
//! ## Configuration (environment variables)
//!
//! All variables are optional and have sensible defaults:
//!
//! | Variable                          | Default                                              |
//! |-----------------------------------|------------------------------------------------------|
//! | `HEN_LOCAL_BOOTSTRAP_STATE_PATH`      | (no state file written)                              |
//! | `QWEN3_ASR_MODEL_PATH`            | `~/.OminiX/models/qwen3-asr-1.7b`                    |
//! | `QWEN3_ASR_REPO`                  | `mlx-community/Qwen3-ASR-1.7B-8bit`                 |
//! | `QWEN35_TRANSLATOR_MODEL_PATH`    | `~/.OminiX/models/Qwen3.5-2B-MLX-4bit`              |
//! | `QWEN35_TRANSLATOR_REPO`          | `mlx-community/Qwen3.5-2B-MLX-4bit`                 |
//! | `MOXIN_MODEL_PROVIDER`            | `auto` (`modelscope`/`huggingface` force one path)  |
//! | `MOXIN_MODELSCOPE_ENDPOINT`       | `https://modelscope.cn`                             |
//! | `HF_ENDPOINT`                     | `https://huggingface.co` (Hugging Face provider)    |

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

// ── State file ────────────────────────────────────────────────────────────────
//
// Format consumed by screen.rs poll_runtime_initialization:
//   "{current}/{total}|{title}|{detail}|{pct}\n"
// where pct is overall download progress as a float 0.0000–1.0000.

// Actual download sizes in bytes (measured 2026-04-17, `du -sk` × 1024)
const BYTES_TRANSLATOR: u64 = 1_749_164_032; // Qwen3.5-2B-MLX-4bit
const BYTES_ASR: u64 = 2_473_308_160; // Qwen3-ASR-1.7B-8bit
const BYTES_TTS: u64 = 3_075_601_408; // Qwen3-TTS CustomVoice + speech tokenizer
const TOTAL_BYTES: u64 = BYTES_TRANSLATOR + BYTES_ASR;
const MODEL_COMPLETION_MARKER: &str = ".moxin-model-complete.json";
const BOOTSTRAP_VERSION: u32 = 1;
const DEFAULT_HF_ENDPOINT: &str = "https://huggingface.co";
const DEFAULT_MODELSCOPE_ENDPOINT: &str = "https://modelscope.cn";
const HTTP_USER_AGENT: &str = "HenLocalTranslator/hen-local-init";
const PROVIDER_PROBE_REPO: &str = "mlx-community/Qwen3.5-2B-MLX-4bit";
const PROVIDER_PROBE_FILE: &str = "config.json";
const BOOTSTRAP_LOCK_FILE: &str = "bootstrap.lock";
const MAX_DOWNLOAD_FILE_ATTEMPTS: usize = 4;

const ASR_MODEL_FILES: &[&str] = &[
    ".gitattributes",
    "README.md",
    "chat_template.json",
    "config.json",
    "generation_config.json",
    "merges.txt",
    "model.safetensors",
    "model.safetensors.index.json",
    "preprocessor_config.json",
    "tokenizer_config.json",
    "vocab.json",
];

const QWEN35_TRANSLATOR_MODEL_FILES: &[&str] = &[
    ".gitattributes",
    "README.md",
    "chat_template.jinja",
    "config.json",
    "model.safetensors",
    "model.safetensors.index.json",
    "preprocessor_config.json",
    "processor_config.json",
    "tokenizer.json",
    "tokenizer_config.json",
    "video_preprocessor_config.json",
    "vocab.json",
];

const QWEN3_TTS_MODEL_FILES: &[&str] = &[
    ".gitattributes",
    "README.md",
    "config.json",
    "generation_config.json",
    "merges.txt",
    "model.safetensors",
    "model.safetensors.index.json",
    "preprocessor_config.json",
    "tokenizer_config.json",
    "vocab.json",
    "speech_tokenizer/config.json",
    "speech_tokenizer/configuration.json",
    "speech_tokenizer/model.safetensors",
    "speech_tokenizer/preprocessor_config.json",
];

fn write_state(
    state_file: Option<&Path>,
    current: usize,
    total: usize,
    title: &str,
    detail: &str,
    bytes_done: u64,
    total_bytes: u64,
) {
    let pct = if total_bytes > 0 {
        (bytes_done as f64 / total_bytes as f64).min(0.99)
    } else {
        0.0
    };
    eprintln!(
        "[hen-local-init] {}/{} {} — {} ({:.1}%)",
        current,
        total,
        title,
        detail,
        pct * 100.0
    );
    let Some(path) = state_file else { return };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(
        path,
        format!("{}/{}|{}|{}|{:.4}\n", current, total, title, detail, pct),
    );
}

fn file_exists(path: &Path) -> bool {
    path.metadata().map(|m| m.is_file()).unwrap_or(false)
}

fn file_len(path: &Path) -> u64 {
    path.metadata().map(|m| m.len()).unwrap_or(0)
}

fn format_bytes_per_second(bytes_per_sec: f64) -> String {
    if bytes_per_sec >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} GB/s", bytes_per_sec / (1024.0 * 1024.0 * 1024.0))
    } else if bytes_per_sec >= 1024.0 * 1024.0 {
        format!("{:.1} MB/s", bytes_per_sec / (1024.0 * 1024.0))
    } else if bytes_per_sec >= 1024.0 {
        format!("{:.1} KB/s", bytes_per_sec / 1024.0)
    } else {
        format!("{:.0} B/s", bytes_per_sec)
    }
}

#[derive(Debug)]
struct BootstrapLock {
    path: PathBuf,
    _file: File,
}

impl Drop for BootstrapLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn parse_bootstrap_lock_pid(contents: &str) -> Option<u32> {
    contents.lines().find_map(|line| {
        let line = line.trim();
        let pid = line.strip_prefix("pid=").unwrap_or(line);
        pid.parse::<u32>().ok().filter(|pid| *pid > 0)
    })
}

fn process_id_is_running(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    if pid == std::process::id() {
        return true;
    }
    Command::new("/bin/kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn bootstrap_lock_is_active(path: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(path) else {
        return false;
    };
    parse_bootstrap_lock_pid(&contents)
        .map(process_id_is_running)
        .unwrap_or(false)
}

fn acquire_bootstrap_lock(path: &Path) -> Result<BootstrapLock> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }

    for _ in 0..2 {
        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(mut file) => {
                write!(file, "pid={}\n", std::process::id())
                    .with_context(|| format!("write bootstrap lock {}", path.display()))?;
                return Ok(BootstrapLock {
                    path: path.to_path_buf(),
                    _file: file,
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                if bootstrap_lock_is_active(path) {
                    bail!(
                        "another hen-local-init bootstrap is already running; lock: {}",
                        path.display()
                    );
                }
                fs::remove_file(path)
                    .with_context(|| format!("remove stale bootstrap lock {}", path.display()))?;
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("create bootstrap lock {}", path.display()));
            }
        }
    }

    bail!("could not acquire bootstrap lock {}", path.display())
}

#[derive(Serialize, Deserialize)]
struct ModelCompletionMarker {
    repo_id: String,
    bootstrap_version: u32,
}

fn model_completion_marker_path(dir: &Path) -> PathBuf {
    dir.join(MODEL_COMPLETION_MARKER)
}

fn model_completion_marker_valid(dir: &Path, repo_id: &str) -> bool {
    let marker_path = model_completion_marker_path(dir);
    let Ok(contents) = fs::read_to_string(marker_path) else {
        return false;
    };
    let Ok(marker) = serde_json::from_str::<ModelCompletionMarker>(&contents) else {
        return false;
    };
    marker.repo_id == repo_id
}

fn write_model_completion_marker(dir: &Path, repo_id: &str) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("mkdir {:?}", dir))?;
    let marker = ModelCompletionMarker {
        repo_id: repo_id.to_string(),
        bootstrap_version: BOOTSTRAP_VERSION,
    };
    let marker_path = model_completion_marker_path(dir);
    let body = serde_json::to_string_pretty(&marker)?;
    fs::write(&marker_path, body)
        .with_context(|| format!("write model completion marker {:?}", marker_path))
}

fn ensure_model_dir_ready(
    dir: &Path,
    repo_id: &str,
    ready_check: impl Fn(&Path) -> bool,
) -> Result<bool> {
    if ready_check(dir) {
        if !model_completion_marker_valid(dir, repo_id) {
            eprintln!(
                "[hen-local-init] complete model found without a valid marker, writing {}",
                dir.display()
            );
            write_model_completion_marker(dir, repo_id)?;
        }
        return Ok(true);
    }

    if model_completion_marker_valid(dir, repo_id) {
        eprintln!(
            "[hen-local-init] marker present but model is incomplete, clearing {}",
            dir.display()
        );
        if dir.exists() {
            fs::remove_dir_all(dir)
                .with_context(|| format!("remove incomplete model dir {}", dir.display()))?;
        }
        return Ok(false);
    }

    if dir.exists() {
        eprintln!(
            "[hen-local-init] model directory without a valid completion marker, removing {}",
            dir.display()
        );
        fs::remove_dir_all(dir)
            .with_context(|| format!("remove incomplete model dir {}", dir.display()))?;
    }
    Ok(false)
}

// ── Model readiness checks ────────────────────────────────────────────────────

fn asr_model_ready(dir: &Path) -> bool {
    file_exists(&dir.join("config.json"))
}

fn qwen35_translation_model_ready(dir: &Path) -> bool {
    file_exists(&dir.join("config.json"))
        && file_exists(&dir.join("tokenizer.json"))
        && file_exists(&dir.join("tokenizer_config.json"))
        && (file_exists(&dir.join("model.safetensors"))
            || file_exists(&dir.join("model.safetensors.index.json")))
}

fn qwen3_tts_model_ready(dir: &Path) -> bool {
    file_exists(&dir.join("config.json"))
        && file_exists(&dir.join("model.safetensors"))
        && file_exists(&dir.join("speech_tokenizer/config.json"))
        && file_exists(&dir.join("speech_tokenizer/model.safetensors"))
}

// ── Model download providers ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelProvider {
    Auto,
    HuggingFace,
    ModelScope,
}

impl ModelProvider {
    fn from_env_value(value: Option<&str>) -> Result<Self> {
        let normalized = value.unwrap_or("").trim().to_ascii_lowercase();
        match normalized.as_str() {
            "" | "auto" => Ok(Self::Auto),
            "modelscope" | "ms" => Ok(Self::ModelScope),
            "huggingface" | "hf" => Ok(Self::HuggingFace),
            other => bail!(
                "unsupported MOXIN_MODEL_PROVIDER={other:?}; expected auto, modelscope, or huggingface"
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct DownloadProvider {
    kind: ModelProvider,
    endpoint: String,
}

impl DownloadProvider {
    fn providers_from_env() -> Result<Vec<Self>> {
        let preference =
            ModelProvider::from_env_value(env::var("MOXIN_MODEL_PROVIDER").ok().as_deref())?;
        let modelscope = Self::modelscope(endpoint_from_env(
            "MOXIN_MODELSCOPE_ENDPOINT",
            DEFAULT_MODELSCOPE_ENDPOINT,
        ));
        let huggingface = Self::huggingface(endpoint_from_env("HF_ENDPOINT", DEFAULT_HF_ENDPOINT));

        match preference {
            ModelProvider::Auto => {
                let probe_client = build_http_client(Duration::from_secs(5))?;
                let modelscope_probe = probe_provider(&probe_client, &modelscope);
                let huggingface_probe = probe_provider(&probe_client, &huggingface);
                if let Err(err) = &modelscope_probe {
                    eprintln!("[hen-local-init] ModelScope probe failed: {err:#}");
                }
                if let Err(err) = &huggingface_probe {
                    eprintln!("[hen-local-init] Hugging Face probe failed: {err:#}");
                }
                let order =
                    auto_provider_order(modelscope_probe.is_ok(), huggingface_probe.is_ok())?;
                Ok(order
                    .into_iter()
                    .map(|kind| provider_for_kind(kind, &modelscope, &huggingface))
                    .collect())
            }
            ModelProvider::HuggingFace => Ok(vec![huggingface]),
            ModelProvider::ModelScope => Ok(vec![modelscope]),
        }
    }

    fn huggingface(endpoint: String) -> Self {
        Self {
            kind: ModelProvider::HuggingFace,
            endpoint: normalize_endpoint(&endpoint),
        }
    }

    fn modelscope(endpoint: String) -> Self {
        Self {
            kind: ModelProvider::ModelScope,
            endpoint: normalize_endpoint(&endpoint),
        }
    }

    fn name(&self) -> &'static str {
        match self.kind {
            ModelProvider::Auto => "auto",
            ModelProvider::HuggingFace => "huggingface",
            ModelProvider::ModelScope => "modelscope",
        }
    }

    fn repo_file_url(&self, repo_id: &str, filename: &str) -> String {
        match self.kind {
            ModelProvider::Auto => unreachable!("auto provider must be resolved before download"),
            ModelProvider::HuggingFace => {
                format!("{}/{}/resolve/main/{}", self.endpoint, repo_id, filename)
            }
            ModelProvider::ModelScope => {
                format!(
                    "{}/models/{}/resolve/master/{}",
                    self.endpoint, repo_id, filename
                )
            }
        }
    }

    fn huggingface_repo_info_url(&self, repo_id: &str) -> String {
        format!("{}/api/models/{}", self.endpoint, repo_id)
    }
}

fn provider_for_kind(
    kind: ModelProvider,
    modelscope: &DownloadProvider,
    huggingface: &DownloadProvider,
) -> DownloadProvider {
    match kind {
        ModelProvider::ModelScope => modelscope.clone(),
        ModelProvider::HuggingFace => huggingface.clone(),
        ModelProvider::Auto => unreachable!("auto provider must be resolved before download"),
    }
}

fn auto_provider_order(
    modelscope_reachable: bool,
    huggingface_reachable: bool,
) -> Result<Vec<ModelProvider>> {
    match (modelscope_reachable, huggingface_reachable) {
        (true, true) => Ok(vec![ModelProvider::ModelScope, ModelProvider::HuggingFace]),
        (true, false) => Ok(vec![ModelProvider::ModelScope]),
        (false, true) => Ok(vec![ModelProvider::HuggingFace]),
        (false, false) => bail!("could not reach ModelScope or Hugging Face model endpoints"),
    }
}

fn probe_provider(client: &reqwest::blocking::Client, provider: &DownloadProvider) -> Result<()> {
    let url = provider.repo_file_url(PROVIDER_PROBE_REPO, PROVIDER_PROBE_FILE);
    let resp = client
        .head(&url)
        .send()
        .with_context(|| format!("HEAD {}", url))?;
    if resp.status().is_success() {
        Ok(())
    } else {
        bail!("HTTP {} probing {}", resp.status(), provider.name())
    }
}

fn run_with_provider_fallback(
    providers: &[DownloadProvider],
    operation_name: &str,
    mut run: impl FnMut(usize, &DownloadProvider) -> Result<()>,
) -> Result<()> {
    if providers.is_empty() {
        bail!("no model download providers configured for {operation_name}");
    }

    let mut failures = Vec::new();
    for (attempt, provider) in providers.iter().enumerate() {
        match run(attempt, provider) {
            Ok(()) => return Ok(()),
            Err(err) => {
                eprintln!(
                    "[hen-local-init] {} failed via {}: {:#}",
                    operation_name,
                    provider.name(),
                    err
                );
                failures.push(format!("{}: {:#}", provider.name(), err));
            }
        }
    }

    bail!(
        "{} failed using all reachable providers: {}",
        operation_name,
        failures.join(" | ")
    )
}

fn endpoint_from_env(name: &str, default: &str) -> String {
    match env::var(name) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => default.to_string(),
    }
}

fn build_http_client(timeout: Duration) -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent(HTTP_USER_AGENT)
        .build()
        .context("Build HTTP client")
}

fn normalize_endpoint(endpoint: &str) -> String {
    endpoint.trim().trim_end_matches('/').to_string()
}

fn modelscope_manifest_files(repo_id: &str) -> Result<&'static [&'static str]> {
    match repo_id {
        "mlx-community/Qwen3-ASR-1.7B-8bit" => Ok(ASR_MODEL_FILES),
        "mlx-community/Qwen3.5-2B-MLX-4bit" => Ok(QWEN35_TRANSLATOR_MODEL_FILES),
        "mlx-community/Qwen3-TTS-12Hz-1.7B-CustomVoice-8bit" => Ok(QWEN3_TTS_MODEL_FILES),
        _ => bail!("no built-in ModelScope manifest for {}", repo_id),
    }
}

#[derive(Deserialize)]
struct Sibling {
    rfilename: String,
}

#[derive(Deserialize)]
struct RepoInfo {
    siblings: Vec<Sibling>,
}

/// Fetch the list of files in a model repo.
fn list_repo_files(
    client: &reqwest::blocking::Client,
    provider: &DownloadProvider,
    repo_id: &str,
) -> Result<Vec<String>> {
    match provider.kind {
        ModelProvider::Auto => unreachable!("auto provider must be resolved before listing files"),
        ModelProvider::HuggingFace => {
            let url = provider.huggingface_repo_info_url(repo_id);
            let info: RepoInfo = client
                .get(&url)
                .send()
                .with_context(|| format!("GET {}", url))?
                .error_for_status()
                .with_context(|| format!("HTTP error listing {}", repo_id))?
                .json()
                .context("Parse repo info JSON")?;
            Ok(info.siblings.into_iter().map(|s| s.rfilename).collect())
        }
        ModelProvider::ModelScope => Ok(modelscope_manifest_files(repo_id)?
            .iter()
            .map(|filename| (*filename).to_string())
            .collect()),
    }
}

/// Download a single file from a model repo to `dest`.
///
/// Uses `Range` requests for resume: if `dest` already exists and is non-empty,
/// only the remaining bytes are fetched and appended.
/// Returns the number of bytes actually written in this call.
fn download_file(
    client: &reqwest::blocking::Client,
    provider: &DownloadProvider,
    repo_id: &str,
    filename: &str,
    dest: &Path,
    mut on_progress: impl FnMut(u64, f64),
) -> Result<u64> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {:?}", dest.parent()))?;
    }

    let existing_bytes = dest.metadata().map(|m| m.len()).unwrap_or(0);
    let url = provider.repo_file_url(repo_id, filename);

    let mut req = client.get(&url);
    if existing_bytes > 0 {
        req = req.header("Range", format!("bytes={}-", existing_bytes));
    }

    let resp = req.send().with_context(|| format!("GET {}", url))?;
    let status = resp.status();

    // 416 Range Not Satisfiable = file is already complete
    if status.as_u16() == 416 {
        return Ok(0);
    }

    if !status.is_success() {
        bail!("HTTP {} downloading {}/{}", status, repo_id, filename);
    }

    let is_partial = status.as_u16() == 206;
    let mut file = if is_partial {
        OpenOptions::new()
            .append(true)
            .open(dest)
            .with_context(|| format!("open for append {:?}", dest))?
    } else {
        File::create(dest).with_context(|| format!("create {:?}", dest))?
    };

    let mut resp = resp;
    let mut downloaded: u64 = 0;
    let start = Instant::now();
    let mut last_report = Instant::now();
    let mut buf = [0_u8; 256 * 1024];

    loop {
        let n = resp
            .read(&mut buf)
            .with_context(|| format!("read body of {}/{}", repo_id, filename))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .with_context(|| format!("write {:?}", dest))?;
        downloaded += n as u64;

        let should_report = last_report.elapsed() >= Duration::from_millis(250);
        if should_report {
            let elapsed = start.elapsed().as_secs_f64().max(0.001);
            let speed = downloaded as f64 / elapsed;
            on_progress(downloaded, speed);
            last_report = Instant::now();
        }
    }

    let elapsed = start.elapsed().as_secs_f64().max(0.001);
    let speed = downloaded as f64 / elapsed;
    on_progress(downloaded, speed);
    Ok(downloaded)
}

fn should_retry_download_error(err: &anyhow::Error) -> bool {
    let msg = format!("{err:#}").to_lowercase();
    if msg.contains("http 4") {
        return false;
    }
    [
        "body",
        "connection",
        "end of file",
        "timed out",
        "timeout",
        "request",
    ]
    .iter()
    .any(|needle| msg.contains(needle))
}

fn download_file_with_retries(
    client: &reqwest::blocking::Client,
    provider: &DownloadProvider,
    repo_id: &str,
    filename: &str,
    dest: &Path,
    mut on_progress: impl FnMut(u64, f64),
) -> Result<u64> {
    let initial_len = file_len(dest);

    for attempt in 1..=MAX_DOWNLOAD_FILE_ATTEMPTS {
        let attempt_base = file_len(dest).saturating_sub(initial_len);
        match download_file(
            client,
            provider,
            repo_id,
            filename,
            dest,
            |written_so_far, speed_bps| {
                on_progress(attempt_base + written_so_far, speed_bps);
            },
        ) {
            Ok(_) => return Ok(file_len(dest).saturating_sub(initial_len)),
            Err(err)
                if attempt < MAX_DOWNLOAD_FILE_ATTEMPTS && should_retry_download_error(&err) =>
            {
                eprintln!(
                    "[hen-local-init] retrying {}/{} via {} after transient download error (attempt {}/{}): {:#}",
                    repo_id,
                    filename,
                    provider.name(),
                    attempt + 1,
                    MAX_DOWNLOAD_FILE_ATTEMPTS,
                    err
                );
                std::thread::sleep(Duration::from_secs(attempt as u64));
            }
            Err(err) => return Err(err),
        }
    }

    bail!("download retry loop exhausted for {}/{}", repo_id, filename)
}

/// Download all files in a model repo to `target_dir`.
///
/// Already-present files are skipped. The `state_file` is updated per-file
/// so the UI progress bar reflects real download activity.
/// `bytes_done` is updated after each file; `total_bytes` is used for pct.
fn download_repo(
    client: &reqwest::blocking::Client,
    provider: &DownloadProvider,
    repo_id: &str,
    target_dir: &Path,
    state_file: Option<&Path>,
    step: usize,
    total_steps: usize,
    bytes_done: &mut u64,
    total_bytes: u64,
) -> Result<()> {
    fs::create_dir_all(target_dir).with_context(|| format!("mkdir {:?}", target_dir))?;

    let short_name = repo_id.split('/').last().unwrap_or(repo_id);
    eprintln!(
        "[hen-local-init] listing files for {} via {}",
        repo_id,
        provider.name()
    );

    let files = list_repo_files(client, provider, repo_id)
        .with_context(|| format!("list files for {}", repo_id))?;

    eprintln!("[hen-local-init] {} file(s) in {}", files.len(), repo_id);

    for (i, filename) in files.iter().enumerate() {
        let dest = target_dir.join(filename);
        if dest.exists() && dest.metadata().map(|m| m.len()).unwrap_or(0) > 0 {
            eprintln!("[hen-local-init] skip (exists): {}", filename);
            continue;
        }
        eprintln!(
            "[hen-local-init] downloading [{}/{}]: {}",
            i + 1,
            files.len(),
            filename
        );
        write_state(
            state_file,
            step,
            total_steps,
            &format!("Downloading {}", short_name),
            &format!("[{}/{}] {}", i + 1, files.len(), filename),
            *bytes_done,
            total_bytes,
        );
        let written = download_file_with_retries(
            client,
            provider,
            repo_id,
            filename,
            &dest,
            |written_so_far, speed_bps| {
                write_state(
                    state_file,
                    step,
                    total_steps,
                    &format!("Downloading {}", short_name),
                    &format!(
                        "[{}/{}] {} • {}",
                        i + 1,
                        files.len(),
                        filename,
                        format_bytes_per_second(speed_bps),
                    ),
                    *bytes_done + written_so_far,
                    total_bytes,
                );
            },
        )
        .with_context(|| format!("download {}/{}", repo_id, filename))?;
        *bytes_done += written;
    }
    Ok(())
}

fn download_model_with_provider_fallback(
    client: &reqwest::blocking::Client,
    providers: &[DownloadProvider],
    repo_id: &str,
    target_dir: &Path,
    state_file: Option<&Path>,
    step: usize,
    total_steps: usize,
    bytes_done: &mut u64,
    total_bytes: u64,
    ready_check: impl Fn(&Path) -> bool + Copy,
    incomplete_message: &str,
) -> Result<()> {
    let short_name = repo_id.split('/').last().unwrap_or(repo_id);
    let bytes_before_model = *bytes_done;

    run_with_provider_fallback(providers, repo_id, |attempt, provider| {
        if attempt > 0 {
            *bytes_done = bytes_before_model;
            if target_dir.exists() {
                fs::remove_dir_all(target_dir)
                    .with_context(|| format!("remove failed model dir {}", target_dir.display()))?;
            }
            write_state(
                state_file,
                step,
                total_steps,
                &format!("Retrying {}", short_name),
                &format!("Switching to {}", provider.name()),
                *bytes_done,
                total_bytes,
            );
        }

        download_repo(
            client,
            provider,
            repo_id,
            target_dir,
            state_file,
            step,
            total_steps,
            bytes_done,
            total_bytes,
        )?;

        if !ready_check(target_dir) {
            bail!("{}: {}", incomplete_message, target_dir.display());
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("hen-local-init-{name}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn ready_check_requires_expected_translator_files() {
        let dir = unique_temp_dir("translator-ready");
        fs::write(dir.join("config.json"), b"{}").unwrap();
        fs::write(dir.join("tokenizer.json"), b"{}").unwrap();
        fs::write(dir.join("tokenizer_config.json"), b"{}").unwrap();
        File::create(dir.join("model.safetensors")).unwrap();

        assert!(qwen35_translation_model_ready(&dir));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn formats_download_speed_human_readably() {
        assert_eq!(format_bytes_per_second(512.0), "512 B/s");
        assert_eq!(format_bytes_per_second(2048.0), "2.0 KB/s");
        assert_eq!(format_bytes_per_second(5.5 * 1024.0 * 1024.0), "5.5 MB/s");
    }

    #[test]
    fn model_provider_defaults_to_auto_and_accepts_aliases() {
        assert_eq!(
            ModelProvider::from_env_value(None).unwrap(),
            ModelProvider::Auto
        );
        assert_eq!(
            ModelProvider::from_env_value(Some("  ")).unwrap(),
            ModelProvider::Auto
        );
        assert_eq!(
            ModelProvider::from_env_value(Some("auto")).unwrap(),
            ModelProvider::Auto
        );
        assert_eq!(
            ModelProvider::from_env_value(Some("modelscope")).unwrap(),
            ModelProvider::ModelScope
        );
        assert_eq!(
            ModelProvider::from_env_value(Some("ms")).unwrap(),
            ModelProvider::ModelScope
        );
        assert_eq!(
            ModelProvider::from_env_value(Some("huggingface")).unwrap(),
            ModelProvider::HuggingFace
        );
        assert_eq!(
            ModelProvider::from_env_value(Some("hf")).unwrap(),
            ModelProvider::HuggingFace
        );
    }

    #[test]
    fn model_provider_rejects_unknown_values() {
        let err = ModelProvider::from_env_value(Some("unknown")).unwrap_err();
        assert!(err.to_string().contains("MOXIN_MODEL_PROVIDER"));
    }

    #[test]
    fn auto_provider_order_prefers_modelscope_with_huggingface_fallback() {
        assert_eq!(
            auto_provider_order(true, true).unwrap(),
            vec![ModelProvider::ModelScope, ModelProvider::HuggingFace]
        );
        assert_eq!(
            auto_provider_order(true, false).unwrap(),
            vec![ModelProvider::ModelScope]
        );
        assert_eq!(
            auto_provider_order(false, true).unwrap(),
            vec![ModelProvider::HuggingFace]
        );
    }

    #[test]
    fn auto_provider_order_errors_when_no_provider_is_reachable() {
        let err = auto_provider_order(false, false).unwrap_err();
        assert!(err
            .to_string()
            .contains("could not reach ModelScope or Hugging Face"));
    }

    #[test]
    fn download_provider_builds_provider_specific_file_urls() {
        let hf = DownloadProvider::huggingface("https://hf.example/".to_string());
        let modelscope = DownloadProvider::modelscope("https://modelscope.example/".to_string());

        assert_eq!(
            hf.repo_file_url("owner/repo", "nested/file.txt"),
            "https://hf.example/owner/repo/resolve/main/nested/file.txt"
        );
        assert_eq!(
            modelscope.repo_file_url("owner/repo", "nested/file.txt"),
            "https://modelscope.example/models/owner/repo/resolve/master/nested/file.txt"
        );
    }

    #[test]
    fn modelscope_manifest_uses_fixed_file_lists() {
        let asr_files = modelscope_manifest_files("mlx-community/Qwen3-ASR-1.7B-8bit").unwrap();
        assert_eq!(
            asr_files,
            &[
                ".gitattributes",
                "README.md",
                "chat_template.json",
                "config.json",
                "generation_config.json",
                "merges.txt",
                "model.safetensors",
                "model.safetensors.index.json",
                "preprocessor_config.json",
                "tokenizer_config.json",
                "vocab.json",
            ]
        );

        let translator_files =
            modelscope_manifest_files("mlx-community/Qwen3.5-2B-MLX-4bit").unwrap();
        assert!(translator_files.contains(&"tokenizer.json"));
    }

    #[test]
    fn modelscope_manifest_rejects_unknown_repos() {
        let err = modelscope_manifest_files("custom/repo").unwrap_err();
        assert!(err
            .to_string()
            .contains("no built-in ModelScope manifest for custom/repo"));
    }

    #[test]
    fn http_client_sends_moxin_user_agent() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut headers = Vec::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                headers.push(line);
            }
            tx.send(headers).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
        });

        let client = build_http_client(Duration::from_secs(1)).unwrap();
        client
            .get(format!("http://{addr}/probe"))
            .send()
            .unwrap()
            .error_for_status()
            .unwrap();

        let headers = rx.recv().unwrap().join("");
        let headers_lower = headers.to_ascii_lowercase();
        assert!(
            headers_lower.contains("user-agent: henlocaltranslator/hen-local-init"),
            "request headers did not contain the expected User-Agent:\n{headers}"
        );
    }

    #[test]
    fn download_file_retries_short_body_with_range_resume() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let (mut first, _) = listener.accept().unwrap();
            let mut first_reader = BufReader::new(first.try_clone().unwrap());
            loop {
                let mut line = String::new();
                first_reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
            }
            first
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nabc")
                .unwrap();

            let (mut second, _) = listener.accept().unwrap();
            let mut second_reader = BufReader::new(second.try_clone().unwrap());
            let mut headers = Vec::new();
            loop {
                let mut line = String::new();
                second_reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                headers.push(line);
            }
            tx.send(headers.join("")).unwrap();
            second
                .write_all(
                    b"HTTP/1.1 206 Partial Content\r\nContent-Length: 7\r\nContent-Range: bytes 3-9/10\r\n\r\ndefghij",
                )
                .unwrap();
        });

        let dir = unique_temp_dir("range-resume");
        let dest = dir.join("file.bin");
        let provider = DownloadProvider::modelscope(format!("http://{addr}"));
        let client = build_http_client(Duration::from_secs(2)).unwrap();
        let written = download_file_with_retries(
            &client,
            &provider,
            "owner/repo",
            "file.bin",
            &dest,
            |_, _| {},
        )
        .unwrap();

        assert_eq!(written, 10);
        assert_eq!(fs::read(&dest).unwrap(), b"abcdefghij");
        let second_headers = rx.recv().unwrap();
        let second_headers_lower = second_headers.to_ascii_lowercase();
        assert!(
            second_headers_lower.contains("range: bytes=3-"),
            "second request did not resume from byte 3:\n{second_headers}"
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn bootstrap_lock_blocks_active_second_instance() {
        let dir = unique_temp_dir("active-lock");
        let lock_path = dir.join("bootstrap.lock");
        let lock = acquire_bootstrap_lock(&lock_path).unwrap();

        let err = acquire_bootstrap_lock(&lock_path).unwrap_err();
        assert!(err.to_string().contains("already running"));

        drop(lock);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn bootstrap_lock_replaces_stale_pid() {
        let dir = unique_temp_dir("stale-lock");
        let lock_path = dir.join("bootstrap.lock");
        fs::write(&lock_path, "pid=999999\n").unwrap();

        let lock = acquire_bootstrap_lock(&lock_path).unwrap();
        assert!(lock_path.exists());
        drop(lock);
        assert!(!lock_path.exists());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn provider_fallback_retries_next_provider_after_error() {
        let providers = vec![
            DownloadProvider::modelscope("https://modelscope.example/".to_string()),
            DownloadProvider::huggingface("https://hf.example/".to_string()),
        ];
        let mut attempts = Vec::new();

        run_with_provider_fallback(&providers, "test operation", |_, provider| {
            attempts.push(provider.kind);
            if provider.kind == ModelProvider::ModelScope {
                bail!("modelscope failed");
            }
            Ok(())
        })
        .unwrap();

        assert_eq!(
            attempts,
            vec![ModelProvider::ModelScope, ModelProvider::HuggingFace]
        );
    }

    #[test]
    fn ensure_model_dir_ready_migrates_complete_unmarked_model_dir() {
        let dir = unique_temp_dir("unmarked-migrate");
        fs::write(dir.join("config.json"), b"{}").unwrap();
        fs::write(dir.join("tokenizer.json"), b"{}").unwrap();
        fs::write(dir.join("tokenizer_config.json"), b"{}").unwrap();
        fs::write(dir.join("model.safetensors"), b"weights").unwrap();

        let ready = ensure_model_dir_ready(
            &dir,
            "mlx-community/Qwen3.5-2B-MLX-4bit",
            qwen35_translation_model_ready,
        )
        .unwrap();

        assert!(ready);
        assert!(model_completion_marker_valid(
            &dir,
            "mlx-community/Qwen3.5-2B-MLX-4bit"
        ));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ensure_model_dir_ready_removes_incomplete_model_dir() {
        let dir = unique_temp_dir("incomplete-remove");
        fs::write(dir.join("config.json"), b"{}").unwrap();
        File::create(dir.join("model.safetensors")).unwrap();

        let ready = ensure_model_dir_ready(
            &dir,
            "mlx-community/Qwen3.5-2B-MLX-4bit",
            qwen35_translation_model_ready,
        )
        .unwrap();

        assert!(!ready);
        assert!(!dir.exists());
    }
}

// ── Configuration ─────────────────────────────────────────────────────────────

struct Config {
    state_file: Option<PathBuf>,
    asr_dir: PathBuf,
    asr_repo: String,
    qwen35_translator_dir: PathBuf,
    qwen35_translator_repo: String,
    qwen3_tts_dir: PathBuf,
    qwen3_tts_repo: String,
}

fn bootstrap_lock_path(cfg: &Config) -> PathBuf {
    cfg.state_file
        .as_ref()
        .and_then(|path| path.parent().map(|parent| parent.join(BOOTSTRAP_LOCK_FILE)))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Library/Logs/HenLocalTranslator")
                .join(BOOTSTRAP_LOCK_FILE)
        })
}

fn resolve_config() -> Config {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    Config {
        state_file: env::var("HEN_LOCAL_BOOTSTRAP_STATE_PATH")
            .ok()
            .map(PathBuf::from),
        asr_dir: env::var("QWEN3_ASR_MODEL_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".OminiX/models/qwen3-asr-1.7b")),
        asr_repo: env::var("QWEN3_ASR_REPO")
            .unwrap_or_else(|_| "mlx-community/Qwen3-ASR-1.7B-8bit".to_string()),
        qwen35_translator_dir: env::var("QWEN35_TRANSLATOR_MODEL_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".OminiX/models/Qwen3.5-2B-MLX-4bit")),
        qwen35_translator_repo: env::var("QWEN35_TRANSLATOR_REPO")
            .unwrap_or_else(|_| "mlx-community/Qwen3.5-2B-MLX-4bit".to_string()),
        qwen3_tts_dir: env::var("QWEN3_TTS_CUSTOMVOICE_MODEL_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                home.join(".OminiX/models/qwen3-tts-mlx/Qwen3-TTS-12Hz-1.7B-CustomVoice-8bit")
            }),
        qwen3_tts_repo: env::var("QWEN3_TTS_CUSTOMVOICE_REPO")
            .unwrap_or_else(|_| "mlx-community/Qwen3-TTS-12Hz-1.7B-CustomVoice-8bit".to_string()),
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cfg = resolve_config();
    let lock_path = bootstrap_lock_path(&cfg);
    let _bootstrap_lock = acquire_bootstrap_lock(&lock_path)?;
    let state_file = cfg.state_file.as_deref();
    let providers = DownloadProvider::providers_from_env()?;
    let provider_names = providers
        .iter()
        .map(|provider| provider.name())
        .collect::<Vec<_>>()
        .join(" -> ");
    eprintln!("[hen-local-init] model provider order: {}", provider_names);

    if env::var("HEN_LOCAL_MODEL_COMPONENT")
        .unwrap_or_else(|_| "core".to_string())
        .eq_ignore_ascii_case("speech")
    {
        let ready = ensure_model_dir_ready(
            &cfg.qwen3_tts_dir,
            &cfg.qwen3_tts_repo,
            qwen3_tts_model_ready,
        )?;
        if ready {
            write_state(
                state_file,
                1,
                1,
                "Speech Model",
                "Already present",
                BYTES_TTS,
                BYTES_TTS,
            );
            println!("[hen-local-init] speech model already ready");
            return Ok(());
        }

        let client = build_http_client(Duration::from_secs(3600))?;
        let mut bytes_done = 0;
        write_state(
            state_file,
            0,
            1,
            "Speech Model",
            "Preparing optional spoken translation",
            0,
            BYTES_TTS,
        );
        download_model_with_provider_fallback(
            &client,
            &providers,
            &cfg.qwen3_tts_repo,
            &cfg.qwen3_tts_dir,
            state_file,
            1,
            1,
            &mut bytes_done,
            BYTES_TTS,
            qwen3_tts_model_ready,
            "Qwen3 TTS model incomplete after download",
        )
        .with_context(|| "Qwen3 TTS download failed")?;
        write_model_completion_marker(&cfg.qwen3_tts_dir, &cfg.qwen3_tts_repo)?;
        write_state(
            state_file,
            1,
            1,
            "Done",
            "Speech model ready",
            BYTES_TTS,
            BYTES_TTS,
        );
        println!("[hen-local-init] speech model initialization complete");
        return Ok(());
    }

    // 2 potential downloads: Qwen3.5 translator and ASR.
    let total: usize = 2;
    let translator_ready = ensure_model_dir_ready(
        &cfg.qwen35_translator_dir,
        &cfg.qwen35_translator_repo,
        qwen35_translation_model_ready,
    )?;
    let asr_ready = ensure_model_dir_ready(&cfg.asr_dir, &cfg.asr_repo, asr_model_ready)?;

    let mut bytes_done: u64 = 0;
    if translator_ready {
        bytes_done += BYTES_TRANSLATOR;
    }
    if asr_ready {
        bytes_done += BYTES_ASR;
    }

    write_state(
        state_file,
        0,
        total,
        "Check Models",
        "Verifying model files",
        bytes_done,
        TOTAL_BYTES,
    );

    let client = build_http_client(Duration::from_secs(3600))?;

    // ── Step 1: Qwen3.5 translator (required) ─────────────────────────────────
    if translator_ready {
        eprintln!("[hen-local-init] Qwen3.5 translator model already ready, skipping");
        write_state(
            state_file,
            1,
            total,
            "Qwen3.5 Translator",
            "Already present",
            bytes_done,
            TOTAL_BYTES,
        );
    } else {
        write_state(
            state_file,
            1,
            total,
            "Downloading Qwen3.5 Translator",
            "Starting...",
            bytes_done,
            TOTAL_BYTES,
        );
        download_model_with_provider_fallback(
            &client,
            &providers,
            &cfg.qwen35_translator_repo,
            &cfg.qwen35_translator_dir,
            state_file,
            1,
            total,
            &mut bytes_done,
            TOTAL_BYTES,
            qwen35_translation_model_ready,
            "Qwen3.5 translator model incomplete after download",
        )
        .with_context(|| "Qwen3.5 translator download failed")?;
        write_model_completion_marker(&cfg.qwen35_translator_dir, &cfg.qwen35_translator_repo)?;
        eprintln!("[hen-local-init] Qwen3.5 translator download complete");
    }

    // ── Step 2: ASR (required) ─────────────────────────────────────────────────
    if asr_ready {
        eprintln!("[hen-local-init] ASR model already ready, skipping");
        write_state(
            state_file,
            2,
            total,
            "ASR Model",
            "Already present",
            bytes_done,
            TOTAL_BYTES,
        );
    } else {
        write_state(
            state_file,
            2,
            total,
            "Downloading ASR Model",
            "Starting...",
            bytes_done,
            TOTAL_BYTES,
        );
        download_model_with_provider_fallback(
            &client,
            &providers,
            &cfg.asr_repo,
            &cfg.asr_dir,
            state_file,
            2,
            total,
            &mut bytes_done,
            TOTAL_BYTES,
            asr_model_ready,
            "ASR model incomplete after download",
        )
        .with_context(|| "ASR model download failed")?;
        write_model_completion_marker(&cfg.asr_dir, &cfg.asr_repo)?;
        eprintln!("[hen-local-init] ASR download complete");
    }

    write_state(
        state_file,
        total,
        total,
        "Done",
        "All models ready",
        TOTAL_BYTES,
        TOTAL_BYTES,
    );
    println!("[hen-local-init] initialization complete");
    Ok(())
}
