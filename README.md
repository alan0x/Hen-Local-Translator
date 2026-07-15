# Hen Local Translator

Offline live speech translation for macOS, built with Rust, Makepad, Dora, and OminiX MLX.

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
cargo build --release
cargo run -p hen-local-translator-shell
```

Some source directories still carry the original fork names during the staged cleanup, but the Cargo packages, product surface, and packaged app are now Hen Local Translator.

## Translation Dataflow

The live translation pipeline is defined in:

```text
apps/hen-local-translator/dataflow/translation_qwen35.yml
```

Runtime graph:

```text
moxin-mic-input -> dora-qwen3-asr -> dora-qwen35-translator -> moxin-translation-listener
```

## macOS Packaging

```bash
bash scripts/build_macos_app.sh \
  --icon moxin-widgets/resources/hen_local_icon.png
bash scripts/build_macos_dmg.sh
```

The generated app defaults to:

- App name: `Hen Local Translator`
- Bundle id: `com.henlocal.translator`
- DMG name: `Hen-Local-Translator-v<version>.dmg`

## License

Apache License 2.0. See [LICENSE](LICENSE).
