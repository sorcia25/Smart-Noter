use smart_noter_core::{models::AudioDevice, AppError, Bilingual};

#[tauri::command]
#[specta::specta]
pub async fn list_audio_devices() -> Result<Vec<AudioDevice>, AppError> {
    Ok(vec![
        AudioDevice {
            id: "system-loopback".into(),
            name: Bilingual::with_en("Audio del sistema (Loopback)", "System Audio (Loopback)"),
            desc: Bilingual::with_en(
                "Captura todo el audio que reproduce la PC — recomendado para Teams/Zoom.",
                "Captures all audio playing on this PC — recommended for Teams/Zoom.",
            ),
            icon: "monitor".into(),
            recommended: true,
            active: true,
        },
        AudioDevice {
            id: "realtek-mic".into(),
            name: Bilingual::with_en(
                "Micrófono — Realtek HD Audio",
                "Microphone — Realtek HD Audio",
            ),
            desc: Bilingual::with_en(
                "Sólo capturará tu voz local, no la de los demás participantes.",
                "Will only capture your local voice, not other participants.",
            ),
            icon: "mic".into(),
            recommended: false,
            active: false,
        },
        AudioDevice {
            id: "jabra-evolve".into(),
            name: Bilingual::with_en("Jabra Evolve2 75 — Headset", "Jabra Evolve2 75 — Headset"),
            desc: Bilingual::with_en(
                "Audio del headset USB. Captura el lado del usuario.",
                "USB headset audio. Captures the user side.",
            ),
            icon: "headphones".into(),
            recommended: false,
            active: false,
        },
        AudioDevice {
            id: "stereo-mix".into(),
            name: Bilingual::with_en("Mezcla estéreo (Stereo Mix)", "Stereo Mix"),
            desc: Bilingual::with_en(
                "Combina entrada y salida del sistema. Alternativa al loopback.",
                "Combines system input and output. Alternative to loopback.",
            ),
            icon: "sliders".into(),
            recommended: false,
            active: false,
        },
    ])
}
