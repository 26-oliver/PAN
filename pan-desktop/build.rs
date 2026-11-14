fn main() {
    println!("cargo:rerun-if-changed=packaging/macos/events.m");
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") => {
            cc::Build::new()
                .file("packaging/macos/events.m")
                .flags(["-fobjc-arc", "-fblocks"])
                .compile("pan_macos_events");
            println!("cargo:rustc-link-lib=framework=AppKit");
            println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks/vlc/lib");
            println!("cargo:rustc-link-arg=-Wl,-rpath,/Applications/VLC.app/Contents/MacOS/lib");
        }
        Ok("windows") if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") => {
            println!("cargo:rerun-if-changed=assets/pan.ico");
            println!("cargo:rerun-if-changed=packaging/windows/pan.rc");
            let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
            let resource = out.join("pan.res");
            let tool = cc::windows_registry::find_tool(&std::env::var("TARGET").unwrap(), "rc.exe")
                .expect("Windows SDK resource compiler");
            let status = tool
                .to_command()
                .args(["/nologo", "/fo"])
                .arg(&resource)
                .arg("packaging/windows/pan.rc")
                .status()
                .expect("run rc.exe");
            assert!(status.success(), "could not compile PAN icon");
            println!("cargo:rustc-link-arg={}", resource.display());
        }
        _ => {}
    }
}
