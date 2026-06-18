#![cfg(feature = "diarize-integration")]

// THE de-risk test: proves sherpa-rs static links + loads real ONNX models on Windows.
// Run manually (models default to C:\Users\erick\diarize-models):
//   cargo test -p smart-noter-diarize --features diarize-integration -- --ignored spike --nocapture
use sherpa_rs::diarize::{Diarize, DiarizeConfig};

fn seg_model() -> String {
    std::env::var("SHERPA_SEG_MODEL")
        .unwrap_or_else(|_| r"C:\Users\erick\diarize-models\segmentation.onnx".into())
}
fn emb_model() -> String {
    std::env::var("SHERPA_EMB_MODEL")
        .unwrap_or_else(|_| r"C:\Users\erick\diarize-models\embedding.onnx".into())
}

fn config() -> DiarizeConfig {
    DiarizeConfig {
        num_clusters: None,
        threshold: Some(0.5),
        min_duration_on: Some(0.3),
        min_duration_off: Some(0.5),
        provider: None,
        debug: false,
    }
}

// Generate ~6s of 16 kHz mono f32 PCM: two 3s "turns" at different frequencies.
// Not real speech (clustering result is meaningless) — this only proves compute()
// links + runs end-to-end without crashing. Real multi-voice accuracy is validated
// in the Phase-7 smoke with actual TTS voices.
fn synth_two_turns() -> Vec<f32> {
    let sr = 16_000usize;
    let mut s = Vec::with_capacity(sr * 6);
    for i in 0..sr * 3 {
        s.push((i as f32 * 2.0 * std::f32::consts::PI * 180.0 / sr as f32).sin() * 0.3);
    }
    for i in 0..sr * 3 {
        s.push((i as f32 * 2.0 * std::f32::consts::PI * 320.0 / sr as f32).sin() * 0.3);
    }
    s
}

fn read_wav_f32_mono(path: &str) -> (Vec<f32>, u32) {
    let mut r = hound::WavReader::open(path).expect("open wav");
    let spec = r.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => r
            .samples::<i32>()
            .map(|s| s.unwrap() as f32 / i32::from(i16::MAX) as f32)
            .collect(),
    };
    (samples, spec.sample_rate)
}

/// PRIMARY GATE: link + construct the diarizer with the real models.
#[test]
#[ignore = "needs the real ONNX models"]
fn spike_links_and_loads_models() {
    let sd = Diarize::new(seg_model(), emb_model(), config());
    assert!(
        sd.is_ok(),
        "Diarize::new failed (link OK but model load failed): {:?}",
        sd.err()
    );
    println!("LINK+LOAD OK: sherpa-rs static linked and loaded both ONNX models");
}

/// SECONDARY: run the full pipeline end-to-end. Uses SHERPA_TEST_WAV if set
/// (real multi-speaker recording), else a synthetic 2-turn signal.
#[test]
#[ignore = "needs the real ONNX models"]
fn spike_compute_runs() {
    let mut sd = Diarize::new(seg_model(), emb_model(), config()).expect("init diarizer");
    let samples = match std::env::var("SHERPA_TEST_WAV") {
        Ok(p) => {
            let (s, sr) = read_wav_f32_mono(&p);
            assert_eq!(sr, 16_000, "models expect 16 kHz mono");
            s
        }
        Err(_) => synth_two_turns(),
    };
    let segments = sd.compute(samples, None).expect("compute failed");
    let speakers: std::collections::BTreeSet<i32> = segments.iter().map(|s| s.speaker).collect();
    for s in &segments {
        println!(
            "start={:.2}s end={:.2}s speaker={}",
            s.start, s.end, s.speaker
        );
    }
    println!(
        "COMPUTE OK: {} segments, {} distinct speakers",
        segments.len(),
        speakers.len()
    );
    // Do NOT assert a specific speaker count here (synthetic tones don't cluster
    // like real voices). Reaching this line proves the pipeline ran end-to-end.
}
