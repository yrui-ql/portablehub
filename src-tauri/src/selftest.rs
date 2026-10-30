//! Headless self-test (`PortableHub.exe --selftest --out <file>`).
//!
//! Produces machine-readable evidence for the SALDC verify gate. It never touches
//! the real startup folder or the real config: everything happens in a temp root.

use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::config::{self, AppEntry, Config};
use crate::domain::{launch, scan, theme};
use crate::infra::{autostart, registry, shortcut};

pub fn run(args: &[String]) -> i32 {
    let out = arg_value(args, "--out");
    let root = std::env::temp_dir().join(format!("portablehub-selftest-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let _ = fs::create_dir_all(&root);

    let mut checks = Map::new();
    let mut errors: Vec<String> = Vec::new();

    check(&mut checks, &mut errors, "config", config_check(&root));
    check(&mut checks, &mut errors, "scan", scan_check(&root));
    check(&mut checks, &mut errors, "launch", launch_check(&root));
    check(&mut checks, &mut errors, "launchSpawn", launch_spawn_check());
    check(&mut checks, &mut errors, "theme", theme_check(&root));
    check(&mut checks, &mut errors, "shortcut", shortcut_check(&root));
    check(&mut checks, &mut errors, "autostart", autostart_check());
    check(
        &mut checks,
        &mut errors,
        "autostartDryRun",
        autostart_dry_run(&root),
    );

    let ok = checks.values().all(|v| v == &Value::Bool(true)) && errors.is_empty();
    let payload = json!({
        "ok": ok,
        "checks": checks,
        "errors": errors,
        "webview2": registry::webview2_version(),
    });
    let text = serde_json::to_string_pretty(&payload).unwrap();

    if let Some(path) = out {
        let _ = fs::write(path, &text);
    }
    // Best effort when a console is attached; the --out file is authoritative.
    println!("{text}");

    let _ = fs::remove_dir_all(&root);
    if ok {
        0
    } else {
        1
    }
}

fn check(checks: &mut Map<String, Value>, errors: &mut Vec<String>, name: &str, r: Result<(), String>) {
    match r {
        Ok(()) => {
            checks.insert(name.to_string(), Value::Bool(true));
        }
        Err(e) => {
            checks.insert(name.to_string(), Value::Bool(false));
            errors.push(format!("{name}: {e}"));
        }
    }
}

fn arg_value(args: &[String], key: &str) -> Option<PathBuf> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
}

fn config_check(root: &Path) -> Result<(), String> {
    let dir = root.join("config");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut cfg = Config::default();
    cfg.apps.push(AppEntry::new("Demo", "Apps/Demo/Demo.exe"));
    config::save(&dir, &cfg)?;
    let loaded = config::load(&dir);
    if loaded.config.apps.len() != 1 {
        return Err("roundtrip lost apps".into());
    }
    if loaded.config.apps[0].path != "Apps/Demo/Demo.exe" {
        return Err("relative path not preserved".into());
    }
    fs::write(config::config_path(&dir), "{broken").map_err(|e| e.to_string())?;
    let broken = config::load(&dir);
    if broken.warning.is_none() {
        return Err("corrupt config did not produce a warning".into());
    }
    Ok(())
}

fn scan_check(root: &Path) -> Result<(), String> {
    let dir = root.join("scan");
    write_fake_exe(&dir.join("Apps/测试 应用/测试 应用.exe"))?;
    write_fake_exe(&dir.join("Apps/Nested/bin/nested.exe"))?;
    write_fake_exe(&dir.join("Apps/Bad/unins000.exe"))?;
    let found = scan::scan(&dir, "Apps");
    if found.len() != 2 {
        return Err(format!("expected 2 apps, got {}", found.len()));
    }
    if !found.iter().any(|c| c.name == "测试 应用") {
        return Err("chinese app not found".into());
    }
    Ok(())
}

fn launch_check(root: &Path) -> Result<(), String> {
    let dir = root.join("launch");
    let exe = dir.join("Apps/Demo/Demo App.exe");
    write_fake_exe(&exe)?;
    let mut entry = AppEntry::new("Demo", "Apps/Demo/Demo App.exe");
    entry.args = vec!["--flag".into()];
    let plan = launch::plan(&dir, &entry)?;
    if !plan.command_line.contains("--flag") {
        return Err("args missing from command line".into());
    }
    if !plan.cwd.ends_with("Demo") {
        return Err("workdir not derived from exe".into());
    }
    Ok(())
}

fn launch_spawn_check() -> Result<(), String> {
    let shell = std::env::var("COMSPEC").map_err(|_| "COMSPEC is not set".to_string())?;
    let exe = Path::new(&shell);
    if !exe.exists() {
        return Err(format!("shell not found: {shell}"));
    }
    let mut entry = AppEntry::new("cmd", shell);
    entry.args = vec!["/C".into(), "exit 0".into()];
    let plan = launch::plan(Path::new("."), &entry)?;
    let pid = launch::spawn(&plan)?;
    if pid == 0 {
        return Err("spawn returned pid 0".into());
    }
    Ok(())
}

fn theme_check(root: &Path) -> Result<(), String> {
    let dir = root.join("themes");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for t in theme::builtin_all() {
        theme::validate(&t)?;
    }
    let mut t = theme::default_theme();
    t.name = "Override".into();
    fs::write(dir.join("pixel-dark.json"), serde_json::to_string(&t).unwrap())
        .map_err(|e| e.to_string())?;
    let got = theme::get(&dir, "pixel-dark")?;
    if got.name != "Override" {
        return Err("user theme did not override builtin".into());
    }
    let mut bad = theme::default_theme();
    bad.css = Some("@import url(https://example.com)".into());
    if theme::validate(&bad).is_ok() {
        return Err("css guard did not reject @import".into());
    }
    Ok(())
}

fn shortcut_check(root: &Path) -> Result<(), String> {
    let dir = root.join("shortcut");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join("SampleApp.exe");
    write_fake_exe(&target)?;
    let lnk = dir.join("SampleApp.lnk");
    shortcut::create(&lnk, &target, "--autostart", &dir)?;
    let info = shortcut::read(&lnk)?;
    if !info.target.ends_with("SampleApp.exe") {
        return Err(format!("unexpected target: {}", info.target));
    }
    if info.arguments != "--autostart" {
        return Err(format!("unexpected args: {}", info.arguments));
    }
    if !shortcut::is_valid(&lnk, &target, "--autostart") {
        return Err("is_valid rejected a link we just created".into());
    }
    shortcut::remove(&lnk)?;
    if lnk.exists() {
        return Err("remove did not delete the link".into());
    }
    Ok(())
}

fn autostart_check() -> Result<(), String> {
    let name = crate::domain::identity::app_name();
    if registry::run_key_has(&name) {
        return Err(format!("HKCU Run contains a {name} entry"));
    }
    // status() only reads; it must not panic even when the link is absent.
    let _ = autostart::status();
    Ok(())
}

fn autostart_dry_run(root: &Path) -> Result<(), String> {
    let dir = root.join("autostart");
    write_fake_exe(&dir.join("Apps/A/A.exe"))?;
    write_fake_exe(&dir.join("Apps/B/B.exe"))?;
    let mut cfg = Config::default();
    let mut a = AppEntry::new("A", "Apps/A/A.exe");
    a.autostart = true;
    let mut b = AppEntry::new("B", "Apps/B/B.exe");
    b.autostart = false;
    cfg.apps = vec![a, b];
    let reports = autostart::launch_flagged(&dir, &cfg, true);
    if reports.len() != 1 {
        return Err(format!("expected 1 flagged app, got {}", reports.len()));
    }
    if !reports[0].ok {
        return Err(reports[0].message.clone());
    }
    Ok(())
}

fn write_fake_exe(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut data = vec![0u8; 512];
    data[0] = b'M';
    data[1] = b'Z';
    data[0x3C..0x40].copy_from_slice(&(128u32).to_le_bytes());
    data[128..132].copy_from_slice(b"PE\0\0");
    data[128 + 24..128 + 26].copy_from_slice(&(0x10bu16).to_le_bytes());
    data[128 + 24 + 68..128 + 24 + 70].copy_from_slice(&(2u16).to_le_bytes());
    fs::write(path, data).map_err(|e| e.to_string())
}
