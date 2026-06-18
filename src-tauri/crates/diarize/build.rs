fn main() {
    // sherpa-onnx static libs on Windows need advapi32 for:
    //   - ETW telemetry: EventWriteTransfer, EventRegister, EventUnregister, EventSetInformation
    //   - eSpeak-ng init: RegOpenKeyExA, RegQueryValueExA
    // These are Windows system libs that the sherpa-rs-sys build script omits.
    #[cfg(target_os = "windows")]
    println!("cargo:rustc-link-lib=advapi32");
}
