use crate::config;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct SetupOptions {
    force: bool,
    startup: bool,
    launch: bool,
    uninstall: bool,
}

impl Default for SetupOptions {
    fn default() -> Self {
        Self {
            force: false,
            startup: true,
            launch: true,
            uninstall: false,
        }
    }
}

fn parse_args(args: &[String]) -> Result<SetupOptions, String> {
    let mut opts = SetupOptions::default();
    for arg in args {
        match arg.as_str() {
            "--force" | "-f" => opts.force = true,
            "--no-startup" => opts.startup = false,
            "--no-launch" => opts.launch = false,
            "--uninstall" => opts.uninstall = true,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown setup option: {other}")),
        }
    }
    Ok(opts)
}

fn print_help() {
    println!(
        "vDeck setup\n\n\
         Usage: vdeck setup [OPTIONS]\n\n\
         Options:\n\
         \x20 -f, --force       Overwrite existing ~/.config/vdeck/config.toml\n\
         \x20 --no-startup      Skip registering Windows automatic startup\n\
         \x20 --no-launch       Do not launch vDeck after setup completes\n\
         \x20 --uninstall       Remove Windows automatic startup entry\n\
         \x20 -h, --help        Show this help message"
    );
}

fn find_source_asset(rel: &str) -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let candidate = dir.join(rel);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join(rel);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<usize, String> {
    fs::create_dir_all(dst).map_err(|e| format!("create {}: {e}", dst.display()))?;
    let mut count = 0usize;
    let entries = fs::read_dir(src).map_err(|e| format!("read {}: {e}", src.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read entry in {}: {e}", src.display()))?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str == "__pycache__" || name_str.ends_with(".pyc") {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        if src_path.is_dir() {
            count += copy_dir_recursive(&src_path, &dst_path)?;
        } else if src_path.is_file() {
            fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("copy {} -> {}: {e}", src_path.display(), dst_path.display()))?;
            count += 1;
        }
    }
    Ok(count)
}

fn ensure_uv() -> Result<(), String> {
    let ok = Command::new("uv")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if ok {
        println!("[ok] uv is installed");
        return Ok(());
    }
    Err(
        "uv is not installed or not on PATH. Install uv from https://docs.astral.sh/uv/ and re-run `vdeck setup`."
            .into(),
    )
}

fn install_binary(current_exe: &Path) -> Result<PathBuf, String> {
    let home = config::user_home_dir()
        .ok_or_else(|| "could not resolve user home directory".to_string())?;
    let bin_dir = home.join(".local").join("share").join("bin");
    fs::create_dir_all(&bin_dir).map_err(|e| format!("create {}: {e}", bin_dir.display()))?;

    let file_name = current_exe
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("vdeck.exe"));
    let target = bin_dir.join(file_name);

    let same_file = fs::canonicalize(current_exe).ok() == fs::canonicalize(&target).ok();
    if !same_file {
        fs::copy(current_exe, &target)
            .map_err(|e| format!("copy {} -> {}: {e}", current_exe.display(), target.display()))?;
        println!("[ok] Installed binary to {}", target.display());
    } else {
        println!("[ok] Binary already at {}", target.display());
    }
    Ok(target)
}

#[cfg(target_os = "windows")]
fn ensure_user_path_contains(dir: &Path) -> Result<(), String> {
    let dir_str = dir.to_string_lossy().to_string();
    let script = r#"
$target = $env:VDECK_BIN_DIR
$current = [Environment]::GetEnvironmentVariable('Path', 'User')
$parts = @()
if ($current) {
    $parts = $current -split ';' | Where-Object { $_ -and $_.Trim() -ne '' }
}
$exists = $parts | Where-Object { $_.TrimEnd('\') -ieq $target.TrimEnd('\') }
if (-not $exists) {
    $newPath = if ($parts.Count -gt 0) { ($parts + $target) -join ';' } else { $target }
    [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
    Write-Output 'ADDED'
} else {
    Write-Output 'EXISTS'
}
"#;
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .env("VDECK_BIN_DIR", &dir_str)
        .output()
        .map_err(|e| format!("update user PATH: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "update user PATH failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim() == "ADDED" {
        println!("[ok] Added {} to user PATH", dir.display());
    } else {
        println!("[ok] {} is already in user PATH", dir.display());
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn ensure_user_path_contains(_dir: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn set_windows_startup(enable: bool, exe_path: &Path) -> Result<(), String> {
    const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    if enable {
        let quoted = format!("\"{}\"", exe_path.display());
        let output = Command::new("reg")
            .args(["add", RUN_KEY, "/v", "vDeck", "/t", "REG_SZ", "/d", &quoted, "/f"])
            .output()
            .map_err(|e| format!("reg add failed: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "reg add failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        println!("[ok] Registered automatic startup: {}", exe_path.display());
    } else {
        let _ = Command::new("reg")
            .args(["delete", RUN_KEY, "/v", "vDeck", "/f"])
            .output();
        println!("[ok] Removed automatic startup entry");
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn set_windows_startup(_enable: bool, _exe_path: &Path) -> Result<(), String> {
    Ok(())
}

pub fn run(args: &[String]) -> Result<(), String> {
    let opts = parse_args(args)?;
    let exe_path =
        std::env::current_exe().map_err(|e| format!("resolve current executable: {e}"))?;

    if opts.uninstall {
        set_windows_startup(false, &exe_path)?;
        return Ok(());
    }

    ensure_uv()?;

    let installed_exe = install_binary(&exe_path)?;
    if let Some(bin_dir) = installed_exe.parent() {
        ensure_user_path_contains(bin_dir)?;
    }

    if let Some(dst_cfg) = config::config_file_path() {
        if let Some(src_cfg) = find_source_asset("vdeck.toml") {
            if dst_cfg.exists() && !opts.force {
                println!(
                    "[ok] Config already exists at {} (use --force to overwrite)",
                    dst_cfg.display()
                );
            } else {
                if let Some(parent) = dst_cfg.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("create {}: {e}", parent.display()))?;
                }
                fs::copy(&src_cfg, &dst_cfg).map_err(|e| {
                    format!("copy {} -> {}: {e}", src_cfg.display(), dst_cfg.display())
                })?;
                println!("[ok] Installed config to {}", dst_cfg.display());
            }
        } else {
            println!("[warn] Source vdeck.toml not found next to executable or in current directory");
        }
    }

    if let Some(dst_plugins) = config::plugins_dir() {
        if let Some(src_plugins) = find_source_asset("plugins") {
            let copied = copy_dir_recursive(&src_plugins, &dst_plugins)?;
            println!(
                "[ok] Synced {copied} plugin file(s) to {}",
                dst_plugins.display()
            );
        } else {
            println!("[warn] Source plugins/ directory not found");
        }
    }

    if opts.startup {
        set_windows_startup(true, &installed_exe)?;
    }

    if opts.launch {
        Command::new(&installed_exe)
            .spawn()
            .map_err(|e| format!("launch {}: {e}", installed_exe.display()))?;
        println!("[ok] Launched vDeck");
    }

    Ok(())
}

struct UpdateOptions {
    repo: Option<String>,
    from: Option<PathBuf>,
    launch: bool,
}

fn parse_update_args(args: &[String]) -> Result<UpdateOptions, String> {
    let mut opts = UpdateOptions {
        repo: std::env::var("VDECK_REPO").ok().filter(|v| !v.trim().is_empty()),
        from: None,
        launch: true,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--repo" => {
                i += 1;
                let val = args.get(i).ok_or("--repo requires <owner/repo>")?;
                opts.repo = Some(val.clone());
            }
            "--from" => {
                i += 1;
                let val = args.get(i).ok_or("--from requires <path>")?;
                opts.from = Some(PathBuf::from(val));
            }
            "--no-launch" => opts.launch = false,
            "--help" | "-h" => {
                println!(
                    "vDeck update\n\n\
                     Usage: vdeck update [OPTIONS]\n\n\
                     Options:\n\
                     \x20 --repo <owner/repo>  Download latest release from GitHub\n\
                     \x20 --from <path>        Update from local .zip, directory, or vdeck.exe\n\
                     \x20 --no-launch          Do not relaunch vDeck after updating\n\
                     \x20 -h, --help           Show this help message"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown update option: {other}")),
        }
        i += 1;
    }
    Ok(opts)
}

fn installed_bin_path() -> Result<PathBuf, String> {
    let home = config::user_home_dir()
        .ok_or_else(|| "could not resolve user home directory".to_string())?;
    Ok(home.join(".local").join("share").join("bin").join("vdeck.exe"))
}

#[cfg(target_os = "windows")]
fn stop_other_vdeck_processes() {
    let pid = std::process::id();
    let script = format!(
        "Get-Process -Name vdeck -ErrorAction SilentlyContinue | Where-Object {{ $_.Id -ne {pid} }} | Stop-Process -Force"
    );
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output();
}

#[cfg(not(target_os = "windows"))]
fn stop_other_vdeck_processes() {}

fn replace_binary(src_exe: &Path, dst_exe: &Path) -> Result<(), String> {
    if let Some(parent) = dst_exe.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let old_exe = dst_exe.with_extension("exe.old");
    let _ = fs::remove_file(&old_exe);

    let same_file = fs::canonicalize(src_exe).ok() == fs::canonicalize(dst_exe).ok();
    if same_file {
        println!("[ok] Binary already up to date at {}", dst_exe.display());
        return Ok(());
    }

    stop_other_vdeck_processes();
    if dst_exe.exists() {
        // Rename first so replacing a currently-running Windows executable succeeds.
        fs::rename(dst_exe, &old_exe)
            .map_err(|e| format!("rename {} -> {}: {e}", dst_exe.display(), old_exe.display()))?;
    }
    fs::copy(src_exe, dst_exe)
        .map_err(|e| format!("copy {} -> {}: {e}", src_exe.display(), dst_exe.display()))?;
    let _ = fs::remove_file(&old_exe);
    println!("[ok] Updated binary at {}", dst_exe.display());
    Ok(())
}

#[cfg(target_os = "windows")]
fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dest_dir).map_err(|e| format!("create {}: {e}", dest_dir.display()))?;
    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Expand-Archive -Path $env:VDECK_ZIP -DestinationPath $env:VDECK_DEST -Force",
        ])
        .env("VDECK_ZIP", zip_path)
        .env("VDECK_DEST", dest_dir)
        .status()
        .map_err(|e| format!("expand archive: {e}"))?;
    if !status.success() {
        return Err(format!("Expand-Archive failed for {}", zip_path.display()));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn extract_zip(_zip_path: &Path, _dest_dir: &Path) -> Result<(), String> {
    Err("zip extraction is only supported on Windows".into())
}

#[cfg(target_os = "windows")]
fn download_github_release(repo: &str, dest_dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(dest_dir).map_err(|e| format!("create {}: {e}", dest_dir.display()))?;
    let zip_path = dest_dir.join("vdeck-windows-x86_64.zip");
    let script = r#"
$headers = @{ 'User-Agent' = 'vdeck-updater' }
$rel = Invoke-RestMethod -Uri "https://api.github.com/repos/$env:VDECK_REPO/releases/latest" -Headers $headers
$asset = $rel.assets | Where-Object { $_.name -eq 'vdeck-windows-x86_64.zip' } | Select-Object -First 1
if (-not $asset) { throw "Release asset vdeck-windows-x86_64.zip not found in $env:VDECK_REPO" }
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $env:VDECK_ZIP -Headers $headers
Write-Output $rel.tag_name
"#;
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .env("VDECK_REPO", repo)
        .env("VDECK_ZIP", &zip_path)
        .output()
        .map_err(|e| format!("download release: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "release download failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
    println!("[ok] Downloaded {repo} release {tag}");
    let extracted = dest_dir.join("extracted");
    extract_zip(&zip_path, &extracted)?;
    Ok(extracted)
}

#[cfg(not(target_os = "windows"))]
fn download_github_release(_repo: &str, _dest_dir: &Path) -> Result<PathBuf, String> {
    Err("GitHub release download is only supported on Windows".into())
}

fn find_exe_and_plugins(root: &Path) -> Option<(PathBuf, Option<PathBuf>)> {
    let direct = root.join("vdeck.exe");
    if direct.is_file() {
        let plugins = root.join("plugins");
        return Some((direct, plugins.is_dir().then_some(plugins)));
    }
    let nested = root.join("vdeck").join("vdeck.exe");
    if nested.is_file() {
        let plugins = root.join("vdeck").join("plugins");
        return Some((nested, plugins.is_dir().then_some(plugins)));
    }
    None
}

pub fn run_update(args: &[String]) -> Result<(), String> {
    let opts = parse_update_args(args)?;
    let dst_exe = installed_bin_path()?;
    let current_exe =
        std::env::current_exe().map_err(|e| format!("resolve current executable: {e}"))?;
    let temp_root = std::env::temp_dir().join(format!("vdeck-update-{}", std::process::id()));

    let (src_exe, src_plugins) = if let Some(from) = opts.from {
        if from.is_dir() {
            find_exe_and_plugins(&from)
                .ok_or_else(|| format!("vdeck.exe not found in {}", from.display()))?
        } else if from.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
            let extracted = temp_root.join("extracted");
            extract_zip(&from, &extracted)?;
            find_exe_and_plugins(&extracted)
                .ok_or_else(|| format!("vdeck.exe not found inside {}", from.display()))?
        } else if from.is_file() {
            let plugins = from.parent().map(|p| p.join("plugins")).filter(|p| p.is_dir());
            (from, plugins)
        } else {
            return Err(format!("update source not found: {}", from.display()));
        }
    } else if let Some(repo) = opts.repo {
        let extracted = download_github_release(&repo, &temp_root)?;
        find_exe_and_plugins(&extracted)
            .ok_or_else(|| "vdeck.exe not found in downloaded release archive".to_string())?
    } else if fs::canonicalize(&current_exe).ok() != fs::canonicalize(&dst_exe).ok() {
        let plugins = current_exe
            .parent()
            .map(|p| p.join("plugins"))
            .filter(|p| p.is_dir());
        (current_exe, plugins)
    } else {
        return Err(
            "specify --repo <owner/repo> (or VDECK_REPO) or --from <zip|dir|vdeck.exe>".into(),
        );
    };

    replace_binary(&src_exe, &dst_exe)?;
    if let (Some(src_p), Some(dst_p)) = (src_plugins, config::plugins_dir()) {
        let copied = copy_dir_recursive(&src_p, &dst_p)?;
        println!("[ok] Synced {copied} plugin file(s) to {}", dst_p.display());
    }
    let _ = fs::remove_dir_all(&temp_root);

    if opts.launch {
        Command::new(&dst_exe)
            .spawn()
            .map_err(|e| format!("launch {}: {e}", dst_exe.display()))?;
        println!("[ok] Relaunched vDeck");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_supports_flags() {
        let args: Vec<String> = ["--force", "--no-startup", "--no-launch"]
            .into_iter()
            .map(String::from)
            .collect();
        let opts = parse_args(&args).unwrap();
        assert!(opts.force);
        assert!(!opts.startup);
        assert!(!opts.launch);
        assert!(!opts.uninstall);
    }

    #[test]
    fn parse_args_rejects_unknown_flag() {
        let args = vec!["--bogus".to_string()];
        assert!(parse_args(&args).is_err());
    }
}
