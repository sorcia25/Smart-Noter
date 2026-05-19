//! Generates 1 second of 440 Hz sine, writes WAV, reads it back, asserts samples
//! match within epsilon. Validates writer.rs end-to-end without touching WASAPI.

use smart_noter_audio::capture::writer::{AudioWriter, WavWriterImpl};
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sn-roundtrip-{}-{}.wav", name, std::process::id()))
}

#[test]
fn sine_440_round_trip_matches_within_epsilon() {
    let path = tmp("sine440");
    let sample_rate = 48_000u32;
    let n = sample_rate as usize;
    let samples: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
        })
        .collect();

    let mut w = WavWriterImpl::create(path.clone(), sample_rate, 1).unwrap();
    w.write(&samples).unwrap();
    let res = Box::new(w).finalize().unwrap();
    assert_eq!(res.sample_count, n as u64);

    let mut reader = hound::WavReader::open(&path).unwrap();
    assert_eq!(reader.spec().sample_rate, sample_rate);
    assert_eq!(reader.spec().channels, 1);

    let read_back: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32_767.0)
        .collect();
    assert_eq!(read_back.len(), n);

    // Compare samples — quantisation gives ~3e-5 max error
    let max_err = read_back
        .iter()
        .zip(samples.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(max_err < 1e-3, "max diff after round-trip: {max_err}");

    std::fs::remove_file(&path).ok();
}
