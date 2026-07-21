# Model, Voice, Library, and Asset License Audit

**Audit date:** 2026-07-21

**Status:** Repository audit complete; final counsel review and release NOTICE
assembly remain required before commercial distribution.

| Component | Delivery | Declared license | Finding / action |
| --- | --- | --- | --- |
| Hen Local source | Bundled | Apache-2.0 | Preserve `LICENSE`, copyright and NOTICE obligations. Source license does not grant trademark rights. |
| [`mlx-community/Qwen3-ASR-1.7B-8bit`](https://huggingface.co/mlx-community/Qwen3-ASR-1.7B-8bit) | Downloaded after install | Apache-2.0 | Model card identifies conversion from the official Qwen ASR model. Audited revision: `a8379a2e2f9e313c9292cdf1af4055ab56d50d55`. |
| [`mlx-community/Qwen3.5-2B-MLX-4bit`](https://huggingface.co/mlx-community/Qwen3.5-2B-MLX-4bit) | Downloaded after install | Apache-2.0 | Model card explicitly inherits Apache-2.0. Audited revision: `93760be4f1f69842a46bc13dbdc0f19e291392a3`. |
| [`mlx-community/Qwen3-TTS-12Hz-1.7B-CustomVoice-8bit`](https://huggingface.co/mlx-community/Qwen3-TTS-12Hz-1.7B-CustomVoice-8bit) | Optional download | Apache-2.0 | Model card identifies the official source and declares Apache-2.0. Audited revision: `41d3337e8b7f2843a75841595fc14e4b9a7a4b96`. |
| Patched `qwen3-tts-mlx` code | Bundled | MIT OR Apache-2.0 | Choose and document Apache-2.0 path or include both required notices. |
| Rust/npm transitive libraries | Bundled | Mixed permissive | Generate a machine-readable third-party license report in release CI and fail on unknown/denied licenses. |
| Hen Local logo and generated icon variants | Bundled | Company asset | Confirm written ownership/assignment for the supplied logo artwork. Do not imply Apache-2.0 grants trademark use. |
| Eight checked-in voice preview WAV files | Bundled | Not recorded in repo | **Release blocker:** document who generated/owns each preview and commercial distribution/voice-persona consent. |

## Commercial-release actions

1. Pin every model repository to an immutable commit instead of moving `main`.
2. Download the corresponding license/model card with each model and show
   attribution in About or the installed licenses directory.
3. Add automated Rust and npm dependency license reports to CI.
4. Create a consolidated `THIRD_PARTY_NOTICES` file for every shipped artifact.
5. Resolve the preview-audio provenance blocker and retain consent records.
6. Re-run this audit whenever a model, voice, library, logo, or bundled asset is
   added or replaced.

The current architecture keeps models out of the DMG and downloads them after
first launch. That reduces installer size but does not remove license and notice
obligations for the downloaded weights.
