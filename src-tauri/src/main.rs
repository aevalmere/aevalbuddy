// No console window in release builds. Smoke lines still reach a parent that
// redirects stdout (tools/installer-check.ps1 reads them that way).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    aevaleron_lib::run();
}
