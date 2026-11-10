fn main() {
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rerun-if-changed=native/macos/OpenWrapNetworkExtensionBridge.m");
        println!("cargo:rerun-if-changed=native/macos/OpenWrapNetworkExtensionBridge.h");
        cc::Build::new()
            .file("native/macos/OpenWrapNetworkExtensionBridge.m")
            .flag("-fobjc-arc")
            .compile("openwrap_ne_bridge");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=NetworkExtension");
        println!("cargo:rustc-link-lib=framework=SystemExtensions");
    }

    let mut attributes = tauri_build::Attributes::new();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Windows: OpenVPN is launched directly and needs an elevated app.
        println!("cargo:rerun-if-changed=windows/openwrap.exe.manifest");
        attributes = attributes.windows_attributes(
            tauri_build::WindowsAttributes::new()
                .app_manifest(include_str!("windows/openwrap.exe.manifest")),
        );
    }
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
