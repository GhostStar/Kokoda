fn main() {
    println!("cargo:rerun-if-changed=native/ffmpeg_encoder.cpp");
    println!("cargo:rerun-if-env-changed=KOKODA_FFMPEG_INCLUDE");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        cc::Build::new()
            .cpp(true)
            .include(
                std::env::var("KOKODA_FFMPEG_INCLUDE")
                    .unwrap_or_else(|_| "../../.tools/ffmpeg/windows-x64/include".into()),
            )
            .file("native/ffmpeg_encoder.cpp")
            .flag_if_supported("/std:c++17")
            .flag_if_supported("/EHsc")
            .compile("kokoda_ffmpeg");
        println!("cargo:rustc-link-lib=d3d11");
        println!("cargo:rustc-link-lib=dxgi");
    }
}
