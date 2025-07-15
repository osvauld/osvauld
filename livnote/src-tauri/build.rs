use std::env;
use std::fs;
use std::path::Path;
fn main() {
    setup_x86_64_android_workaround();
    tauri_build::build()
}
// https://github.com/smartvaults/smartvaults/blob/be6fc3e1377a29942395c41fdd1ae7de86985286/bindings/smartvaults-sdk-ffi/build.rs#L16
// workaround for andorid emulator x86_64 which was not compiling sqlite3-sys
fn setup_x86_64_android_workaround() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS not set");
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH not set");

    if target_arch == "x86_64" && target_os == "android" {
        let android_ndk_home = env::var("NDK_HOME").expect("NDK_HOME not set");

        let build_os = match env::consts::OS {
            "linux" => "linux",
            "macos" => "darwin",
            "windows" => "windows",
            _ => panic!(
                "Unsupported OS. You must use either Linux, MacOS or Windows to build the crate."
            ),
        };

        let clang_version = detect_ndk_clang_version(&android_ndk_home, build_os);

        let linux_x86_64_lib_dir = format!(
            "toolchains/llvm/prebuilt/{build_os}-x86_64/lib/clang/{clang_version}/lib/linux/"
        );
        let linkpath = format!("{android_ndk_home}/{linux_x86_64_lib_dir}");

        if Path::new(&linkpath).exists() {
            println!("cargo:rustc-link-search={android_ndk_home}/{linux_x86_64_lib_dir}");
            println!("cargo:rustc-link-lib=static=clang_rt.builtins-x86_64-android");
        } else {
            panic!("Path {linkpath} does not exist");
        }
    }
}

fn detect_ndk_clang_version(ndk_home: &str, build_os: &str) -> String {
    let clang_dir = Path::new(ndk_home).join(format!(
        "toolchains/llvm/prebuilt/{build_os}-x86_64/lib/clang"
    ));

    if let Ok(entries) = fs::read_dir(clang_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(version) = entry.file_name().to_str() {
                    return version.to_string();
                }
            }
        }
    }

    panic!("Unable to detect NDK Clang version")
}
