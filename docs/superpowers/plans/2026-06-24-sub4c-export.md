# Sub-4C — Export (MP3 / Markdown / PDF) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a user export a meeting as Markdown, PDF, and/or a real MP3 transcode of its recording, delivered through a native Save-as / Select-folder dialog, by wiring the existing `ExportModal`.

**Architecture:** A new pure crate `smart-noter-export` holds one independently-testable generator per format: `markdown::to_markdown(&MeetingDetail, &ExportOpts) -> String`, `pdf::to_pdf(...) -> Result<Vec<u8>>` (genpdf, embedded TTF font), `audio::wav_or_flac_to_mp3(&Path) -> Result<Vec<u8>>` (decode via hound/claxon, encode via mp3lame-encoder). The binary's `export_meeting` command loads the `MeetingDetail` (and audio path) from the DB, runs the requested generators, then uses `tauri-plugin-dialog` to pick a destination and writes the bytes. All meeting text comes from `get_meeting` — no recomputation.

**Tech Stack:** Rust crate (`mp3lame-encoder` 0.2, `genpdf` 0.2, `hound`, `claxon`, `thiserror`), Tauri command + `tauri-plugin-dialog` 2.x + specta, React (existing `ExportModal`) + RTK Query, vitest/RTL.

**Conventions (same as Sub-4A/B/D):**
- Build/commit with the LLVM/cmake env preamble on EVERY `cargo`/`git commit` (the pre-commit clippy hook rebuilds native crates; `mp3lame-encoder` vendors+compiles libmp3lame in C, so it needs cmake too):
  ```bash
  export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"
  export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
  ```
  Run cargo with `--manifest-path "src-tauri/Cargo.toml"`. Use a LONG timeout (600000 ms) on the first build of each new native dep.
- New SQL (none here) would use UNCHECKED sqlx; this module adds no migration (tables exist).
- Run `cargo fmt` (from `src-tauri/`) AND `npx biome format --write` on touched files before each commit (lefthook blocks on format; rustfmt splits long lines — if `cargo fmt --check` fails in the hook, run `cargo fmt` from `src-tauri/` and re-stage).
- Generated `bindings.ts` / `keys.ts` are gitignored — regenerate, never commit.
- `from_files`-based genpdf font loading needs a runtime path that won't exist in an installed app — embed the TTF via `include_bytes!` + `FontData::new` instead (Task 3).
- Sequencing follows the spec's risk order: **MD → PDF → MP3** (MP3 last; it carries the `mp3lame-encoder` native-build risk). Each format ships green before the next.

**Verified integration facts (already checked — trust these):**
- `meetings_repo::get_detail(&pool, id) -> Result<MeetingDetail, DbError>` returns the full content model.
- `MeetingAssetsRepo(&pool).get_audio(id) -> Result<Option<MeetingAsset>, AppError>`; `MeetingAsset.path: String` is the `.wav`/`.flac` on disk.
- Models: `MeetingDetail { id, title: Bilingual, template, date, duration_sec: i64, device_used: Option<String>, word_count: i64, summary: Option<Bilingual>, participants: Vec<Participant>, actions: Vec<Action>, decisions: Vec<Decision>, blockers: Vec<Blocker>, transcript: Vec<TranscriptLine> }`. `Bilingual { es: String, en: Option<String> }` with `.pick(lang)`. `Participant { id, label, name: Option<String>, talk_pct: i64, ... }`. `Action { text: Bilingual, owner_participant_id: Option<String>, due: Option<String>, done: bool, ... }`. `Decision/Blocker { id: i64, text: Bilingual }`. `TranscriptLine { id: i64, t: String, speaker_id: String, text: Bilingual }`.
- The `AppError` enum has `NotFound(String)`, `Validation(String)`, `Database(String)`, `Internal(String)`. `from_db(DbError) -> AppError` exists in `src-tauri/src/error.rs`.
- Decode pattern to mirror (NO downmix/resample for export): `crates/whisper/src/decode.rs` reads WAV via `hound::WavReader` and FLAC via `claxon::FlacReader`, returning `(Vec<f32> interleaved, sample_rate: u32, channels: u16)`.
- `ExportModal` is rendered in `src/features/meeting-detail/MeetingDetailPage.tsx` (~L117) where `id` (from `useParams`) is in scope; the modal currently takes only `{ open, onClose, meetingTitle }` and its export button is a no-op (`title="Próximamente"`). Its format values are `'audio' | 'md' | 'pdf'`.
- Tauri builder chain is in `src-tauri/src/lib.rs` (~L80): `.plugin(tauri_plugin_log::Builder::default().build())`. Commands registered in `collect_commands![...]`.

---

## Phase 1 — The `export` crate scaffold

### Task 1: Create `smart-noter-export` crate with `ExportOpts` + `ExportError`

**Files:**
- Create: `src-tauri/crates/export/Cargo.toml`
- Create: `src-tauri/crates/export/src/lib.rs`
- Modify: `src-tauri/Cargo.toml` (workspace `members`)

- [ ] **Step 1: Add the crate to the workspace**

In `src-tauri/Cargo.toml`, add `"crates/export"` to `[workspace].members`.

- [ ] **Step 2: Create `src-tauri/crates/export/Cargo.toml`**

```toml
[package]
name = "smart-noter-export"
version.workspace = true
edition.workspace = true

[dependencies]
smart-noter-core = { path = "../core" }
thiserror.workspace = true
hound = "3.5"
claxon = "0.4"
genpdf = "0.2"
mp3lame-encoder = "0.2"

[dev-dependencies]
tempfile = "3"
```
(`hound`/`claxon` are already transitive deps used by the audio/whisper crates, so versions are proven on this toolchain.)

- [ ] **Step 3: Create `src-tauri/crates/export/src/lib.rs`**

```rust
//! Pure, side-effect-free meeting exporters: one function per format, each
//! takes the already-loaded `MeetingDetail` (or an audio path) and returns
//! bytes/string. No DB, no filesystem dialog — the binary's `export_meeting`
//! command orchestrates I/O.

pub mod audio;
pub mod markdown;
pub mod pdf;

use thiserror::Error;

/// Per-export options from the modal. `timestamps`/`bilingual` apply to text
/// formats only (ignored by MP3).
#[derive(Debug, Clone, Copy)]
pub struct ExportOpts {
    pub timestamps: bool,
    pub bilingual: bool,
}

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("audio decode failed: {0}")]
    Decode(String),
    #[error("mp3 encode failed: {0}")]
    Mp3(String),
    #[error("pdf render failed: {0}")]
    Pdf(String),
    #[error("unsupported audio format: {0}")]
    UnsupportedAudio(String),
}

pub use audio::wav_or_flac_to_mp3;
pub use markdown::to_markdown;
pub use pdf::to_pdf;
```

- [ ] **Step 4: Stub the three modules so it compiles**

Create `src/markdown.rs`, `src/pdf.rs`, `src/audio.rs` each with a `use` of the crate types and a `todo!()`-free minimal signature that returns an empty result, so `cargo build -p smart-noter-export` compiles. (The real bodies land in Tasks 2–4; stub now only to get a green build + commit.)

`src/markdown.rs`:
```rust
use crate::ExportOpts;
use smart_noter_core::models::MeetingDetail;

pub fn to_markdown(_m: &MeetingDetail, _opts: &ExportOpts) -> String {
    String::new()
}
```
`src/pdf.rs`:
```rust
use crate::{ExportError, ExportOpts};
use smart_noter_core::models::MeetingDetail;

pub fn to_pdf(_m: &MeetingDetail, _opts: &ExportOpts) -> Result<Vec<u8>, ExportError> {
    Ok(Vec::new())
}
```
`src/audio.rs`:
```rust
use crate::ExportError;
use std::path::Path;

pub fn wav_or_flac_to_mp3(_path: &Path) -> Result<Vec<u8>, ExportError> {
    Ok(Vec::new())
}
```

- [ ] **Step 5: Build + commit**

```bash
export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
cargo build -p smart-noter-export --manifest-path "src-tauri/Cargo.toml"   # LONG timeout: first genpdf+mp3lame build
cargo fmt --manifest-path "src-tauri/Cargo.toml"
git add src-tauri/Cargo.toml src-tauri/crates/export
git commit -m "feat(export): scaffold smart-noter-export crate (ExportOpts, ExportError, module stubs)"
```
Expected: compiles (proves `genpdf` + `mp3lame-encoder` build on this toolchain BEFORE writing real code — if `mp3lame-encoder` fails with a cmake/cc error, that's the build-risk surfacing; resolve the C toolchain before continuing).

---

## Phase 2 — Markdown generator

### Task 2: `markdown::to_markdown` (TDD)

**Files:**
- Modify: `src-tauri/crates/export/src/markdown.rs`

- [ ] **Step 1: Write failing tests**

Replace `src/markdown.rs` test section — write the tests FIRST (impl is still the stub):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use smart_noter_core::models::{Decision, MeetingDetail, Participant, TranscriptLine};
    use smart_noter_core::Bilingual;

    fn fixture() -> MeetingDetail {
        MeetingDetail {
            id: "m1".into(),
            title: Bilingual { es: "Reunión técnica".into(), en: Some("Technical meeting".into()) },
            template: "tecnica".into(),
            date: "2026-06-20T15:00:00Z".into(),
            duration_sec: 95,
            device_used: None,
            word_count: 3,
            summary: Some(Bilingual { es: "Resumen es".into(), en: Some("Summary en".into()) }),
            participants: vec![Participant {
                id: "p1".into(), meeting_id: "m1".into(), label: "S1".into(),
                name: Some("Ana".into()), color_class: "c1".into(), word_count: 3, talk_pct: 100,
            }],
            actions: vec![],
            decisions: vec![Decision { id: 1, text: Bilingual { es: "Decidir X".into(), en: None } }],
            blockers: vec![],
            transcript: vec![TranscriptLine {
                id: 1, t: "00:00".into(), speaker_id: "p1".into(),
                text: Bilingual { es: "hola equipo".into(), en: Some("hi team".into()) },
            }],
        }
    }

    #[test]
    fn has_core_sections() {
        let md = to_markdown(&fixture(), &ExportOpts { timestamps: true, bilingual: false });
        assert!(md.starts_with("# Reunión técnica"), "title heading");
        assert!(md.contains("## Participantes"));
        assert!(md.contains("Ana"));
        assert!(md.contains("## Resumen"));
        assert!(md.contains("Resumen es"));
        assert!(md.contains("## Decisiones"));
        assert!(md.contains("Decidir X"));
        assert!(md.contains("## Transcripción"));
        assert!(md.contains("hola equipo"));
    }

    #[test]
    fn timestamps_toggle() {
        let on = to_markdown(&fixture(), &ExportOpts { timestamps: true, bilingual: false });
        assert!(on.contains("[00:00]"), "timestamp present when on");
        let off = to_markdown(&fixture(), &ExportOpts { timestamps: false, bilingual: false });
        assert!(!off.contains("[00:00]"), "timestamp absent when off");
    }

    #[test]
    fn bilingual_emits_en_alongside_es() {
        let md = to_markdown(&fixture(), &ExportOpts { timestamps: false, bilingual: true });
        assert!(md.contains("hola equipo"), "es text");
        assert!(md.contains("hi team"), "en text when bilingual");
    }

    #[test]
    fn empty_sections_are_skipped() {
        let mut m = fixture();
        m.decisions.clear();
        let md = to_markdown(&m, &ExportOpts { timestamps: false, bilingual: false });
        assert!(!md.contains("## Decisiones"), "no Decisiones heading when none");
    }
}
```

- [ ] **Step 2: Run — verify FAIL**

`cargo test -p smart-noter-export --manifest-path "src-tauri/Cargo.toml" markdown` → FAIL (stub returns "").

- [ ] **Step 3: Implement `to_markdown`**

Replace the stub body (keep the test module):

```rust
use crate::ExportOpts;
use smart_noter_core::models::{MeetingDetail, Participant};
use smart_noter_core::Bilingual;

/// One Markdown line for a bilingual value: `es` always; ` / en` appended when
/// `bilingual` is on and an `en` exists.
fn bi(text: &Bilingual, opts: &ExportOpts) -> String {
    match (&text.en, opts.bilingual) {
        (Some(en), true) if !en.is_empty() => format!("{} / {}", text.es, en),
        _ => text.es.clone(),
    }
}

fn speaker_name(participants: &[Participant], speaker_id: &str) -> String {
    participants
        .iter()
        .find(|p| p.id == speaker_id)
        .map(|p| p.name.clone().unwrap_or_else(|| p.label.clone()))
        .unwrap_or_else(|| "—".into())
}

fn fmt_duration(sec: i64) -> String {
    let m = sec / 60;
    let s = sec % 60;
    format!("{m:02}:{s:02}")
}

pub fn to_markdown(m: &MeetingDetail, opts: &ExportOpts) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", bi(&m.title, opts)));
    out.push_str(&format!("**Fecha:** {}  \n", m.date));
    out.push_str(&format!("**Duración:** {}\n", fmt_duration(m.duration_sec)));

    if !m.participants.is_empty() {
        out.push_str("\n## Participantes\n\n");
        for p in &m.participants {
            let name = p.name.clone().unwrap_or_else(|| p.label.clone());
            out.push_str(&format!("- {} ({}%)\n", name, p.talk_pct));
        }
    }
    if let Some(s) = &m.summary {
        out.push_str("\n## Resumen\n\n");
        out.push_str(&format!("{}\n", bi(s, opts)));
    }
    if !m.decisions.is_empty() {
        out.push_str("\n## Decisiones\n\n");
        for d in &m.decisions {
            out.push_str(&format!("- {}\n", bi(&d.text, opts)));
        }
    }
    if !m.blockers.is_empty() {
        out.push_str("\n## Bloqueos\n\n");
        for b in &m.blockers {
            out.push_str(&format!("- {}\n", bi(&b.text, opts)));
        }
    }
    if !m.actions.is_empty() {
        out.push_str("\n## Acciones\n\n");
        for a in &m.actions {
            let check = if a.done { "x" } else { " " };
            let mut line = format!("- [{}] {}", check, bi(&a.text, opts));
            if let Some(due) = &a.due {
                line.push_str(&format!(" _(vence: {due})_"));
            }
            out.push_str(&line);
            out.push('\n');
        }
    }
    out.push_str("\n## Transcripción\n\n");
    for line in &m.transcript {
        let ts = if opts.timestamps { format!("`[{}]` ", line.t) } else { String::new() };
        let who = speaker_name(&m.participants, &line.speaker_id);
        out.push_str(&format!("{ts}**{who}:** {}\n\n", bi(&line.text, opts)));
    }
    out
}
```

- [ ] **Step 4: Run — verify PASS**

`cargo test -p smart-noter-export --manifest-path "src-tauri/Cargo.toml" markdown` → all 4 pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt --manifest-path "src-tauri/Cargo.toml"
git add src-tauri/crates/export/src/markdown.rs
git commit -m "feat(export): markdown generator (sections, timestamps, bilingual)"
```

---

## Phase 3 — PDF generator

### Task 3: `pdf::to_pdf` with embedded font (TDD)

**Files:**
- Create: `src-tauri/crates/export/fonts/` — 4 TTF files (see Step 1)
- Modify: `src-tauri/crates/export/src/pdf.rs`

- [ ] **Step 1: Bundle a TTF font family**

genpdf needs real font files (it does NOT use PDF base-14 fonts). Download the 4 **Liberation Sans** variants (SIL OFL, redistributable) and place them at exactly these paths:
- `src-tauri/crates/export/fonts/LiberationSans-Regular.ttf`
- `src-tauri/crates/export/fonts/LiberationSans-Bold.ttf`
- `src-tauri/crates/export/fonts/LiberationSans-Italic.ttf`
- `src-tauri/crates/export/fonts/LiberationSans-BoldItalic.ttf`

Source: https://github.com/liberationfonts/liberation-fonts/releases (the `liberation-fonts-ttf-*.tar.gz` asset). These get embedded via `include_bytes!` (Step 3), so they ship inside the binary — no runtime path needed. Commit the TTFs (they are NOT gitignored).

- [ ] **Step 2: Write the failing test**

In `src/pdf.rs`, add (reuse the `fixture()` shape from Task 2 — duplicate a minimal fixture here or move it to a shared `#[cfg(test)]` helper module):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ExportOpts;
    use smart_noter_core::models::{MeetingDetail, TranscriptLine};
    use smart_noter_core::Bilingual;

    fn fixture() -> MeetingDetail {
        MeetingDetail {
            id: "m1".into(),
            title: Bilingual { es: "Reunión".into(), en: None },
            template: "tecnica".into(),
            date: "2026-06-20T15:00:00Z".into(),
            duration_sec: 60, device_used: None, word_count: 1,
            summary: None, participants: vec![], actions: vec![],
            decisions: vec![], blockers: vec![],
            transcript: vec![TranscriptLine {
                id: 1, t: "00:00".into(), speaker_id: "p1".into(),
                text: Bilingual { es: "contenido".into(), en: None },
            }],
        }
    }

    #[test]
    fn renders_nonempty_pdf() {
        let bytes = to_pdf(&fixture(), &ExportOpts { timestamps: true, bilingual: false }).unwrap();
        assert!(bytes.len() > 1000, "pdf should have real content, got {}", bytes.len());
        assert_eq!(&bytes[0..5], b"%PDF-", "starts with the PDF magic header");
    }
}
```

- [ ] **Step 3: Run — verify FAIL**

`cargo test -p smart-noter-export --manifest-path "src-tauri/Cargo.toml" pdf` → FAIL (stub returns empty Vec; `&bytes[0..5]` panics or length assert fails).

- [ ] **Step 4: Implement `to_pdf`**

> API NOTE (genpdf 0.2): build a `FontFamily<FontData>` from embedded bytes, `Document::new(family)`, `push` elements, `render(&mut Vec<u8>)`. Headings = `Paragraph` wrapped in a bold/larger `Style`. Verify exact `FontData::new` signature against `genpdf::fonts` docs; it takes `(Vec<u8>, Option<Settings>)` and returns `Result<FontData, _>`. If `render` takes a writer, pass `&mut Vec<u8>`.

```rust
use crate::{ExportError, ExportOpts};
use genpdf::fonts::{FontData, FontFamily};
use genpdf::style::Style;
use genpdf::{elements, Document, Element};
use smart_noter_core::models::MeetingDetail;
use smart_noter_core::Bilingual;

fn bi(text: &Bilingual, opts: &ExportOpts) -> String {
    match (&text.en, opts.bilingual) {
        (Some(en), true) if !en.is_empty() => format!("{} / {}", text.es, en),
        _ => text.es.clone(),
    }
}

fn embedded_font_family() -> Result<FontFamily<FontData>, ExportError> {
    let load = |bytes: &[u8]| {
        FontData::new(bytes.to_vec(), None).map_err(|e| ExportError::Pdf(format!("font: {e}")))
    };
    Ok(FontFamily {
        regular: load(include_bytes!("../fonts/LiberationSans-Regular.ttf"))?,
        bold: load(include_bytes!("../fonts/LiberationSans-Bold.ttf"))?,
        italic: load(include_bytes!("../fonts/LiberationSans-Italic.ttf"))?,
        bold_italic: load(include_bytes!("../fonts/LiberationSans-BoldItalic.ttf"))?,
    })
}

fn heading(text: &str, size: u8) -> impl Element {
    elements::Paragraph::new(text).styled(Style::new().bold().with_font_size(size))
}

pub fn to_pdf(m: &MeetingDetail, opts: &ExportOpts) -> Result<Vec<u8>, ExportError> {
    let mut doc = Document::new(embedded_font_family()?);
    doc.set_title(m.title.es.clone());

    doc.push(heading(&bi(&m.title, opts), 18));
    doc.push(elements::Paragraph::new(format!("Fecha: {}", m.date)));
    doc.push(elements::Break::new(1));

    if !m.participants.is_empty() {
        doc.push(heading("Participantes", 14));
        for p in &m.participants {
            let name = p.name.clone().unwrap_or_else(|| p.label.clone());
            doc.push(elements::Paragraph::new(format!("• {} ({}%)", name, p.talk_pct)));
        }
        doc.push(elements::Break::new(1));
    }
    if let Some(s) = &m.summary {
        doc.push(heading("Resumen", 14));
        doc.push(elements::Paragraph::new(bi(s, opts)));
        doc.push(elements::Break::new(1));
    }
    for (title, items) in [
        ("Decisiones", m.decisions.iter().map(|d| &d.text).collect::<Vec<_>>()),
        ("Bloqueos", m.blockers.iter().map(|b| &b.text).collect::<Vec<_>>()),
    ] {
        if !items.is_empty() {
            doc.push(heading(title, 14));
            for t in items {
                doc.push(elements::Paragraph::new(format!("• {}", bi(t, opts))));
            }
            doc.push(elements::Break::new(1));
        }
    }
    if !m.actions.is_empty() {
        doc.push(heading("Acciones", 14));
        for a in &m.actions {
            let mark = if a.done { "[x]" } else { "[ ]" };
            doc.push(elements::Paragraph::new(format!("{mark} {}", bi(&a.text, opts))));
        }
        doc.push(elements::Break::new(1));
    }

    doc.push(heading("Transcripción", 14));
    for line in &m.transcript {
        let ts = if opts.timestamps { format!("[{}] ", line.t) } else { String::new() };
        let who = m
            .participants
            .iter()
            .find(|p| p.id == line.speaker_id)
            .map(|p| p.name.clone().unwrap_or_else(|| p.label.clone()))
            .unwrap_or_else(|| "—".into());
        doc.push(elements::Paragraph::new(format!("{ts}{who}: {}", bi(&line.text, opts))));
    }

    let mut buf = Vec::new();
    doc.render(&mut buf).map_err(|e| ExportError::Pdf(e.to_string()))?;
    Ok(buf)
}
```

- [ ] **Step 5: Run — verify PASS**

`cargo test -p smart-noter-export --manifest-path "src-tauri/Cargo.toml" pdf` → PASS. If genpdf API names differ (e.g. `render` signature, `Style::with_font_size`), adjust to the 0.2 docs — the test (non-empty, `%PDF-` magic) is the invariant.

- [ ] **Step 6: Commit**

```bash
cargo fmt --manifest-path "src-tauri/Cargo.toml"
git add src-tauri/crates/export/fonts src-tauri/crates/export/src/pdf.rs
git commit -m "feat(export): PDF generator via genpdf with embedded Liberation Sans"
```

---

## Phase 4 — MP3 generator (highest build risk — last)

### Task 4: `audio::wav_or_flac_to_mp3` (TDD)

**Files:**
- Modify: `src-tauri/crates/export/src/audio.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn write_wav(path: &std::path::Path, rate: u32, channels: u16, frames: usize) {
        let spec = hound::WavSpec {
            channels, sample_rate: rate, bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..frames {
            for _c in 0..channels {
                w.write_sample(((i % 200) as i16 - 100) * 50).unwrap();
            }
        }
        w.finalize().unwrap();
    }

    #[test]
    fn wav_transcodes_to_nonempty_mp3() {
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("a.wav");
        write_wav(&wav, 44_100, 2, 44_100); // 1s stereo
        let mp3 = wav_or_flac_to_mp3(&wav).unwrap();
        assert!(mp3.len() > 200, "mp3 should have frames, got {}", mp3.len());
        // MP3 starts with an ID3 tag or an MPEG frame sync (0xFF 0xEx/0xFx).
        let ok = &mp3[0..3] == b"ID3" || (mp3[0] == 0xFF && (mp3[1] & 0xE0) == 0xE0);
        assert!(ok, "looks like MP3: first bytes {:02X?}", &mp3[0..4]);
    }

    #[test]
    fn rejects_unknown_extension() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.ogg");
        std::fs::write(&p, b"x").unwrap();
        assert!(matches!(wav_or_flac_to_mp3(&p), Err(ExportError::UnsupportedAudio(_))));
    }
}
```

- [ ] **Step 2: Run — verify FAIL**

`cargo test -p smart-noter-export --manifest-path "src-tauri/Cargo.toml" audio` → FAIL (stub returns empty Vec).

- [ ] **Step 3: Implement decode + encode**

> API NOTE (mp3lame-encoder 0.2.4): `Builder::new()` → `.with_num_channels(u8)?` → `.with_sample_rate(u32)?` → `.with_brate(Bitrate::Kbps128)?` → `.with_quality(Quality::Good)?` → `.build()?`. Encode interleaved i16 via `InterleavedPcm(&[i16])`: `encoder.encode(InterleavedPcm(&pcm_i16), out.spare_capacity_mut())?` returns the written byte count; grow with `unsafe { out.set_len(out.len() + n) }`. Size buffer with `max_required_buffer_size(num_samples)`. Finish with `encoder.flush::<FlushNoGap>(out.spare_capacity_mut())?`. Verify the exact builder method names against the 0.2.4 docs.

```rust
use crate::ExportError;
use mp3lame_encoder::{Bitrate, Builder, FlushNoGap, InterleavedPcm, Quality};
use std::path::Path;

/// Decode WAV or FLAC into interleaved i16 PCM at its native rate/channels —
/// NO downmix, NO resample (preserve the original recording for export).
fn decode_interleaved_i16(path: &Path) -> Result<(Vec<i16>, u32, u16), ExportError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "wav" => read_wav_i16(path),
        "flac" => read_flac_i16(path),
        other => Err(ExportError::UnsupportedAudio(other.to_string())),
    }
}

fn read_wav_i16(path: &Path) -> Result<(Vec<i16>, u32, u16), ExportError> {
    let mut reader = hound::WavReader::open(path).map_err(|e| ExportError::Decode(e.to_string()))?;
    let spec = reader.spec();
    let samples: Vec<i16> = match spec.sample_format {
        hound::SampleFormat::Int => reader
            .samples::<i32>()
            .map(|s| s.map(|v| {
                // Scale down to 16-bit if the source is wider; our recorder writes 16-bit.
                let shift = spec.bits_per_sample.saturating_sub(16);
                (v >> shift) as i16
            }).map_err(|e| ExportError::Decode(e.to_string())))
            .collect::<Result<_, _>>()?,
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .map(|s| s.map(|v| (v.clamp(-1.0, 1.0) * 32_767.0) as i16)
                .map_err(|e| ExportError::Decode(e.to_string())))
            .collect::<Result<_, _>>()?,
    };
    Ok((samples, spec.sample_rate, spec.channels))
}

fn read_flac_i16(path: &Path) -> Result<(Vec<i16>, u32, u16), ExportError> {
    let mut reader = claxon::FlacReader::open(path).map_err(|e| ExportError::Decode(e.to_string()))?;
    let info = reader.streaminfo();
    let shift = (info.bits_per_sample as i32 - 16).max(0);
    let mut samples = Vec::new();
    for s in reader.samples() {
        let v = s.map_err(|e| ExportError::Decode(e.to_string()))?;
        samples.push((v >> shift) as i16);
    }
    Ok((samples, info.sample_rate, info.channels as u16))
}

pub fn wav_or_flac_to_mp3(path: &Path) -> Result<Vec<u8>, ExportError> {
    let (pcm, rate, channels) = decode_interleaved_i16(path)?;

    let mut encoder = Builder::new()
        .ok_or_else(|| ExportError::Mp3("builder init".into()))?;
    encoder
        .set_num_channels(channels.clamp(1, 2) as u8)
        .map_err(|e| ExportError::Mp3(format!("channels: {e:?}")))?;
    encoder
        .set_sample_rate(rate)
        .map_err(|e| ExportError::Mp3(format!("rate: {e:?}")))?;
    encoder
        .set_brate(Bitrate::Kbps128)
        .map_err(|e| ExportError::Mp3(format!("brate: {e:?}")))?;
    encoder
        .set_quality(Quality::Good)
        .map_err(|e| ExportError::Mp3(format!("quality: {e:?}")))?;
    let mut encoder = encoder.build().map_err(|e| ExportError::Mp3(format!("build: {e:?}")))?;

    let mut out: Vec<u8> = Vec::new();
    out.reserve(mp3lame_encoder::max_required_buffer_size(pcm.len()));
    let n = encoder
        .encode(InterleavedPcm(&pcm), out.spare_capacity_mut())
        .map_err(|e| ExportError::Mp3(format!("encode: {e:?}")))?;
    unsafe { out.set_len(out.len() + n) };

    out.reserve(mp3lame_encoder::max_required_buffer_size(0).max(7200));
    let n = encoder
        .flush::<FlushNoGap>(out.spare_capacity_mut())
        .map_err(|e| ExportError::Mp3(format!("flush: {e:?}")))?;
    unsafe { out.set_len(out.len() + n) };

    Ok(out)
}
```

> If `Builder::new()` returns a `Builder` directly (not `Option`) or the setters are the `with_*` consuming form in 0.2.4, adapt to the docs — the decode half and the test are the stable parts. Keep the i16 InterleavedPcm path.

- [ ] **Step 4: Run — verify PASS**

`cargo test -p smart-noter-export --manifest-path "src-tauri/Cargo.toml" audio` → both pass. Then run the FULL crate suite: `cargo test -p smart-noter-export --manifest-path "src-tauri/Cargo.toml"` (markdown + pdf + audio all green).

- [ ] **Step 5: Commit**

```bash
cargo fmt --manifest-path "src-tauri/Cargo.toml"
git add src-tauri/crates/export/src/audio.rs
git commit -m "feat(export): MP3 transcode (wav/flac decode + mp3lame encode)"
```

---

## Phase 5 — Command + native dialog

### Task 5: `export_meeting` command + `tauri-plugin-dialog`

**Files:**
- Modify: `src-tauri/Cargo.toml` (`smart-noter-export`, `tauri-plugin-dialog` deps)
- Create: `src-tauri/src/commands/export.rs`
- Modify: `src-tauri/src/commands/mod.rs` (`pub mod export;`)
- Modify: `src-tauri/src/lib.rs` (plugin + command registration)
- Modify: `src-tauri/capabilities/default.json` (dialog permissions — verify path)

- [ ] **Step 1: Add deps**

In `src-tauri/Cargo.toml` `[dependencies]`: add
```toml
tauri-plugin-dialog = "2"
smart-noter-export = { path = "crates/export" }
```

- [ ] **Step 2: Register plugin + capability**

In `src-tauri/src/lib.rs`, add to the builder chain after the log plugin:
```rust
.plugin(tauri_plugin_dialog::Builder::default().build())
```
Find the capabilities file (`src-tauri/capabilities/default.json` or `tauri.conf.json` `app.security`). Add the dialog permissions to the window's `permissions` array:
```json
"dialog:allow-save",
"dialog:allow-open"
```
(Verify the exact permission identifiers in the generated `gen/schemas/` — they're `dialog:allow-save` / `dialog:allow-open` in plugin v2. Without them the dialog call is denied at runtime.)

- [ ] **Step 3: Write the command**

Create `src-tauri/src/commands/export.rs`:

```rust
use crate::error::from_db;
use crate::state::AppState;
use smart_noter_core::AppError;
use smart_noter_db::repos::{meeting_assets_repo::MeetingAssetsRepo, meetings_repo};
use smart_noter_export::{to_markdown, to_pdf, wav_or_flac_to_mp3, ExportOpts};
use std::path::PathBuf;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

fn ext_for(fmt: &str) -> &'static str {
    match fmt {
        "audio" => "mp3",
        "pdf" => "pdf",
        _ => "md",
    }
}

/// Generate the bytes for one format. `audio` needs the audio path.
/// Takes `&SqlitePool` (not `&State`) so it composes without Deref friction.
async fn bytes_for(
    pool: &sqlx::SqlitePool,
    fmt: &str,
    detail: &smart_noter_core::MeetingDetail,
    opts: &ExportOpts,
) -> Result<Vec<u8>, AppError> {
    match fmt {
        "md" => Ok(to_markdown(detail, opts).into_bytes()),
        "pdf" => to_pdf(detail, opts).map_err(|e| AppError::Internal(e.to_string())),
        "audio" => {
            let asset = MeetingAssetsRepo(pool)
                .get_audio(&detail.id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("no audio for {}", detail.id)))?;
            let path = PathBuf::from(asset.path);
            // Encoding is CPU-bound; run it off the async runtime's reactor.
            tauri::async_runtime::spawn_blocking(move || wav_or_flac_to_mp3(&path))
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?
                .map_err(|e| AppError::Internal(e.to_string()))
        }
        other => Err(AppError::Validation(format!("unknown format: {other}"))),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn export_meeting(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    meeting_id: String,
    formats: Vec<String>,
    file_name: String,
    timestamps: bool,
    bilingual: bool,
) -> Result<Vec<String>, AppError> {
    if formats.is_empty() {
        return Err(AppError::Validation("no formats selected".into()));
    }
    let detail = meetings_repo::get_detail(&state.pool, &meeting_id)
        .await
        .map_err(from_db)?;
    let opts = ExportOpts { timestamps, bilingual };

    // Generate every artifact first (so a dialog only appears once we have data).
    let mut artifacts: Vec<(String, Vec<u8>)> = Vec::new(); // (ext, bytes)
    for fmt in &formats {
        let bytes = bytes_for(&state.pool, fmt, &detail, &opts).await?;
        artifacts.push((ext_for(fmt).to_string(), bytes));
    }

    // Single format → "Save as"; multiple → "Select folder".
    let written: Vec<String> = if artifacts.len() == 1 {
        let (ext, bytes) = &artifacts[0];
        let Some(path) = app
            .dialog()
            .file()
            .set_file_name(format!("{file_name}.{ext}"))
            .add_filter(ext.to_uppercase(), &[ext.as_str()])
            .blocking_save_file()
        else {
            return Ok(vec![]); // user cancelled
        };
        let path = path.into_path().map_err(|e| AppError::Internal(e.to_string()))?;
        std::fs::write(&path, bytes).map_err(|e| AppError::Internal(e.to_string()))?;
        vec![path.display().to_string()]
    } else {
        let Some(dir) = app.dialog().file().blocking_pick_folder() else {
            return Ok(vec![]); // cancelled
        };
        let dir = dir.into_path().map_err(|e| AppError::Internal(e.to_string()))?;
        let mut out = Vec::new();
        for (ext, bytes) in &artifacts {
            let path = dir.join(format!("{file_name}.{ext}"));
            std::fs::write(&path, bytes).map_err(|e| AppError::Internal(e.to_string()))?;
            out.push(path.display().to_string());
        }
        out
    };
    Ok(written)
}
```

> API NOTE: `blocking_save_file`/`blocking_pick_folder` return `Option<FilePath>`. Convert with `FilePath::into_path() -> Result<PathBuf, _>` (verify; in tauri v2 `FilePath` is an enum `Path(PathBuf)`/`Url(Url)` with `into_path`/`as_path`). Running the blocking dialog inside an async command is fine because the command executes off the main thread; if Windows requires the dialog on the main thread, switch to the async `save_file(move |p| ...)` callback bridged with a `tokio::sync::oneshot`.

- [ ] **Step 4: Register the command + module**

`src-tauri/src/commands/mod.rs`: add `pub mod export;`.
`src-tauri/src/lib.rs` `collect_commands![...]`: add `commands::export::export_meeting,`.

- [ ] **Step 5: Build + commit**

```bash
export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
cargo build -p smart-noter --manifest-path "src-tauri/Cargo.toml"   # LONG timeout
cargo fmt --manifest-path "src-tauri/Cargo.toml"
git add src-tauri/Cargo.toml src-tauri/src/commands/export.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/capabilities
git commit -m "feat(commands): export_meeting (generators + native save/folder dialog)"
```

---

## Phase 6 — bindings + i18n + frontend

### Task 6: Regenerate bindings, add i18n, wire `ExportModal`

**Files:**
- Modify: `src/i18n/locales/es.json`, `en.json`
- Modify: `src/store/api/meetings.api.ts` (+ test)
- Modify: `src/features/meeting-detail/ExportModal/ExportModal.tsx`
- Modify: `src/features/meeting-detail/MeetingDetailPage.tsx`

- [ ] **Step 1: Regenerate bindings**

`npm run generate:bindings` (DLL-copy workaround if it crashes — copy sherpa `*.dll` into `src-tauri/target/debug/` next to `specta-export.exe`). Confirm `exportMeeting` appears in `src/ipc/bindings.ts`.

- [ ] **Step 2: i18n keys**

Add to `es.json`: `"exporting": "Exportando…", "exportDone": "Exportación lista", "exportFailed": "No se pudo exportar", "exportCancelled": "Exportación cancelada"`. To `en.json`: `"exporting": "Exporting…", "exportDone": "Export ready", "exportFailed": "Export failed", "exportCancelled": "Export cancelled"`. Then `npm run generate:i18n-keys` and validate JSON:
```bash
node -e "JSON.parse(require('fs').readFileSync('src/i18n/locales/es.json','utf8'));JSON.parse(require('fs').readFileSync('src/i18n/locales/en.json','utf8'));console.log('OK')"
```

- [ ] **Step 3: RTK endpoint + test**

In `src/store/api/meetings.api.ts` add a mutation (no tag invalidation — export is read-only):
```ts
    exportMeeting: b.mutation<
      string[],
      { meetingId: string; formats: string[]; fileName: string; timestamps: boolean; bilingual: boolean }
    >({
      query: (args) => ({ cmd: 'export_meeting', args }),
    }),
```
Export `useExportMeetingMutation`. Add `src/store/api/meetings.export.test.ts` (hoisted-mock pattern like `meetings.search.test.ts`) asserting `invoke('export_meeting', { meetingId, formats, fileName, timestamps, bilingual })` is called.

- [ ] **Step 4: Wire `ExportModal`**

In `ExportModal.tsx`:
1. Add `meetingId: string` to `ExportModalProps`, and destructure it: `export function ExportModal({ open, onClose, meetingTitle, meetingId }: ExportModalProps)`.
2. Add imports: `import { toast } from '@/components/primitives/Toast/Toast';` and `import { useExportMeetingMutation } from '@/store/api/meetings.api';`.
3. Inside the component: `const [exportMeeting, { isLoading: busy }] = useExportMeetingMutation();`
4. Replace the primary `<Button>` (currently `onClick={onClose} title="Próximamente"`) with:

```tsx
<Button
  variant="primary"
  icon={<Icon name="download" size={14} />}
  disabled={selected.size === 0 || busy}
  onClick={async () => {
    try {
      const written = await exportMeeting({
        meetingId,
        formats: [...selected],
        fileName,
        timestamps,
        bilingual,
      }).unwrap();
      if (written.length === 0) {
        toast(t('exportCancelled')); // dialog cancelled — keep modal open
        return;
      }
      toast.success(t('exportDone'));
      onClose();
    } catch {
      toast.error(t('exportFailed'));
    }
  }}
>
  {busy ? t('exporting') : t('exportNow')}
</Button>
```
(`selected` is a `Set<Fmt>` where `Fmt = 'audio' | 'md' | 'pdf'`; `[...selected]` is the `string[]` the command expects, and `'audio'` maps to MP3 server-side via `ext_for`.)

In `MeetingDetailPage.tsx` (~L117) pass the id:
```tsx
<ExportModal
  open={exportOpen}
  onClose={() => setExportOpen(false)}
  meetingTitle={pickL(meeting.title, lang)}
  meetingId={id ?? ''}
/>
```

- [ ] **Step 5: Verify + commit**

```bash
npx biome format --write src/store/api src/features/meeting-detail
npm run test:run -- meetings.export ExportModal
npx tsc --noEmit
npm run lint   # src/ must be clean; pre-existing src-tauri/gen errors are not ours
git add src/i18n/locales/es.json src/i18n/locales/en.json src/store/api/meetings.api.ts src/store/api/meetings.export.test.ts src/features/meeting-detail
git commit -m "feat(fe): wire ExportModal to export_meeting (MP3/MD/PDF)"
```

---

## Phase 7 — Verification + smoke

### Task 7: Full verification + real-app smoke

- [ ] **Step 1: Backend** — `cargo test -p smart-noter-export --manifest-path "src-tauri/Cargo.toml"` + `cargo build -p smart-noter --manifest-path "src-tauri/Cargo.toml"` (green).
- [ ] **Step 2: Frontend** — `npm run test:run && npx tsc --noEmit && npm run lint` (green; `src/` clean).
- [ ] **Step 3: Manual smoke** — launch the app (`npm run tauri:dev` with the preamble). Open a meeting that HAS audio + transcript → Export:
  - MD only → Save-as → open the file: sections + (optionally) timestamps + bilingual correct.
  - PDF only → Save-as → opens in a PDF viewer, paginated, readable.
  - MP3 only → Save-as → plays back, ~same duration as the recording.
  - MD+PDF+MP3 together → Select-folder → all three written as `<file_name>.{md,pdf,mp3}`.
  - Cancel the dialog → no file, "cancelled" toast, no error.
  No DB is mutated by export, but if anything writes to `%APPDATA%`, back up + restore as in prior smokes. Back up the DB anyway out of habit.
- [ ] **Step 4: Final commit (if fixups).**

---

## Notes for the executor

- **Build risk is front-loaded:** Task 1 builds `genpdf` + `mp3lame-encoder` before any real code. If `mp3lame-encoder` won't compile (cmake/cc), stop and fix the C toolchain — don't write generators against a crate that won't link. `mp3lame-encoder` vendors libmp3lame and needs the same cmake the whisper/sherpa builds use (the env preamble provides it).
- **Fonts are committed binaries:** the 4 Liberation Sans TTFs live in `crates/export/fonts/` and are embedded via `include_bytes!` — NOT gitignored. Without them the PDF crate won't compile (the `include_bytes!` path must exist).
- **External-API drift:** the genpdf (0.2), mp3lame-encoder (0.2.4), and tauri-plugin-dialog (2.x) snippets reflect their documented APIs but may need small signature adjustments at implementation time. The TESTS are the contract — keep them; adapt the calls. Each API NOTE block flags where to verify.
- **Dialog threading:** `blocking_*` dialogs run inside the async command (off the main thread) — fine on macOS/Linux and generally on Windows. If a Windows main-thread requirement bites, bridge the async `save_file(cb)` with a `tokio::sync::oneshot`.
- **Cancellation is not an error:** a cancelled dialog returns `Ok(vec![])`; the frontend shows a neutral "cancelled" toast, not an error.
- **This is Module C of Sub-4 — the LAST module.** When it ships, Sub-4 Persistence is complete.
```