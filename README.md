# Hen Local Translator

Offline live speech translation for macOS, built with Rust, Tauri 2, Svelte, Dora, and OminiX MLX.

Hen Local Translator focuses on one workflow: capture microphone or system audio, transcribe it with Qwen3-ASR, translate committed speech chunks with the Qwen3.5 translator node, and display bilingual subtitles in a floating overlay.

## Features

- Live translation from microphone or macOS system audio
- Floating subtitle overlay with compact/fullscreen modes
- Bilingual source and translated text display
- Adjustable subtitle size, opacity, and anchor position
- Transcript export/autosave support
- Translation-only Dora dataflow with ASR and translator nodes

## Requirements

- Apple Silicon Mac
- macOS 14.0+ recommended
- Rust 1.82+
- Node.js 20+ and npm
- Dora CLI (`cargo install dora-cli`)
- Python 3.8+ only for the optional development model download helper

System audio capture uses ScreenCaptureKit and requires macOS Screen Recording permission. Microphone input remains available if Screen Recording permission is not granted.

## Model Setup

Development helper:

```bash
bash scripts/init_qwen3_models.sh
```

This downloads:

| Model | Purpose |
| --- | --- |
| `Qwen3-ASR-1.7B-8bit` | Speech recognition |
| `Qwen3.5-2B-MLX-4bit` | Text translation |

Packaged builds use the bundled `hen-local-init` helper for first-run model bootstrap.

## Build And Run

```bash
npm --prefix hen-local-translator-shell/ui install
npm --prefix hen-local-translator-shell/ui run desktop:dev
```

The desktop command builds MLX-backed translation nodes in a temporary Cargo target directory whose path contains no spaces. This is required because MLX 0.30.1 can generate invalid Metal JIT sources when Cargo's target path contains spaces (for example, the `Moxin AI` workspace directory).

Useful checks:

```bash
npm --prefix hen-local-translator-shell/ui run check
cargo test -p hen-local-translator-shell
```

## Translation Dataflow

The live translation pipeline is defined in:

```text
hen-local-translator-shell/dataflow/translation_qwen35.yml
```

Runtime graph:

```text
moxin-mic-input -> dora-qwen3-asr -> dora-qwen35-translator -> moxin-translation-listener
```

## macOS Packaging

```bash
bash scripts/build_macos_app.sh
bash scripts/build_macos_dmg.sh
```

The app bundle uses the checked-in Hen Local `.icns` icon by default. Regenerate all
platform icon formats from the original artwork with:

```bash
python3 scripts/generate_app_icons.py \
  --source hen-local-translator-shell/icons/HenLocal_logo.jpeg \
  --output-dir hen-local-translator-shell/icons
```

The generated app defaults to:

- App name: `Hen Local Translator`
- Bundle id: `com.henlocal.translator`
- DMG name: `Hen-Local-Translator-v<version>.dmg`

## License

Apache License 2.0. See [LICENSE](LICENSE).
