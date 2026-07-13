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

    tauri_build::build();
}
