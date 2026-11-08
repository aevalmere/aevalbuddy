//! Stages the existing renderer pages into `ui/`, which is `frontendDist`, so
//! relative paths such as `../renderer/glass.js` resolve exactly as they did
//! under file:// in Electron. The pages themselves stay where they are.

use std::fs;
use std::path::Path;

const PAGES: [&str; 3] = ["renderer", "settings", "history"];

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create ui dir");
    for entry in fs::read_dir(from).expect("read page dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let dest = to.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &dest);
        } else {
            fs::copy(&path, &dest).expect("copy page file");
        }
    }
}

fn main() {
    let root = Path::new("..");
    let ui = Path::new("ui");
    if ui.exists() {
        fs::remove_dir_all(ui).expect("clear ui dir");
    }
    for page in PAGES {
        copy_dir(&root.join(page), &ui.join(page));
        println!("cargo:rerun-if-changed=../{page}");
    }
    // The shots fixture stands in for the desktop behind the glass.
    fs::create_dir_all(ui.join("lib")).expect("create ui/lib");
    fs::copy(
        root.join("lib/backdrop-fixture.html"),
        ui.join("lib/backdrop-fixture.html"),
    )
    .expect("copy backdrop fixture");
    println!("cargo:rerun-if-changed=../lib/backdrop-fixture.html");
    println!("cargo:rerun-if-changed=../preload.js");
    println!("cargo:rerun-if-changed=bridge/aeval-bridge.js");
    // The Common Controls v6 manifest goes to every linked target, unit
    // tests included: without it the test exe dies on load (0xc0000139,
    // TaskDialogIndirect). tauri-build would embed it in the app exe only, so
    // its own copy is turned off. app.manifest is tauri-build's default.
    // (agent c)
    let manifest = std::env::current_dir().expect("cwd").join("app.manifest");
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(
        tauri_build::WindowsAttributes::new_without_app_manifest(),
    ))
    .expect("tauri build");
}
