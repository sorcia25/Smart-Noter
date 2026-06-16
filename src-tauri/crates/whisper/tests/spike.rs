#![cfg(feature = "whisper-integration")]

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

// Run manually:  WHISPER_TEST_MODEL=C:\path\ggml-base.bin \
//   cargo test -p smart-noter-whisper --features whisper-integration -- --ignored spike
#[test]
#[ignore = "needs a real ggml model via WHISPER_TEST_MODEL"]
fn spike_transcribes_silence() {
    let model = std::env::var("WHISPER_TEST_MODEL").expect("set WHISPER_TEST_MODEL");
    let ctx = WhisperContext::new_with_params(&model, WhisperContextParameters::default())
        .expect("load model");
    let mut state = ctx.create_state().expect("create state");

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(2);
    params.set_translate(false);
    params.set_language(Some("auto"));
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    // 5 s of silence at 16 kHz mono.
    let audio = vec![0.0_f32; 16_000 * 5];
    state.full(params, &audio).expect("run");

    let n = state.full_n_segments(); // 0.16: returns i32 directly
    for i in 0..n {
        if let Some(seg) = state.get_segment(i) {
            let _text = seg.to_str_lossy().expect("text");
            let _t0 = seg.start_timestamp(); // centiseconds (i64)
            let _t1 = seg.end_timestamp();
        }
    }
    // Success = the API compiled and ran. Silence may yield 0 segments — that's fine.
}
