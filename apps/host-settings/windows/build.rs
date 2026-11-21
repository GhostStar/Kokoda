fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // mt.exe cannot consume Parallels UNC source paths; stage beside build outputs.
        let manifest =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("app.manifest");
        std::fs::write(&manifest, include_bytes!("app.manifest")).unwrap();
        println!("cargo:rerun-if-changed=app.manifest");
        println!("cargo:rustc-link-arg-bin=KokodaHost=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=KokodaHost=/MANIFESTINPUT:{}",
            manifest.display()
        );
        // Stage resources locally too: Windows SDK tools cannot use a UNC cwd.
        let out = manifest.parent().unwrap();
        std::fs::write(
            out.join("Kokoda.ico"),
            include_bytes!("../../../packaging/icons/Kokoda.ico"),
        )
        .unwrap();
        std::fs::write(out.join("icon.rc"), "1 ICON \"Kokoda.ico\"\n").unwrap();
        println!("cargo:rerun-if-changed=../../../packaging/icons/Kokoda.ico");
        println!("cargo:rerun-if-env-changed=RC");
        let target = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
        let compiler = std::env::var_os("RC")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                find_msvc_tools::find_windows_sdk(&target).and_then(|sdk| {
                    sdk.path()
                        .flat_map(|p| {
                            // SDK discovery also returns unversioned bin/<host>;
                            // newer SDKs install tools in bin/<version>/<host>.
                            let versioned = p.parent().and_then(|base| {
                                p.file_name().map(|arch| {
                                    base.join(sdk.sdk_version()).join(arch).join("rc.exe")
                                })
                            });
                            std::iter::once(p.join("rc.exe")).chain(versioned)
                        })
                        .find(|p| p.is_file())
                })
            })
            .expect("Windows SDK rc.exe not found; install the SDK or set RC");
        let status = std::process::Command::new(compiler)
            .current_dir(out)
            .args(["/nologo", "/fo", "icon.res", "icon.rc"])
            .status()
            .expect("Run the Windows resource compiler");
        assert!(status.success(), "Compile Kokoda icon resource");
        println!(
            "cargo:rustc-link-arg-bin=KokodaHost={}",
            out.join("icon.res").display()
        );
    }
}
