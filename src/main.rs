mod config;

use anyhow::{bail, Context, Result};
use chrono::Local;
use clap::{Parser, Subcommand};
use config::Config;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Parser, Debug)]
#[command(name = "matrixshot", version, about = "Wayland screenshots and recordings")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Region screenshot (default)
    Region,
    /// Fullscreen / focused output screenshot
    Fullscreen,
    /// Open last screenshot with configured viewer
    OpenLast,
    /// Open folder containing last screenshot
    Folder,
    /// Upload last capture and copy the URL (provider from config)
    UploadLast,
    /// Show / init config path
    Config,
    Record {
        #[command(subcommand)]
        action: Option<RecordCmd>,
    },
}

#[derive(Subcommand, Debug)]
enum RecordCmd {
    Fullscreen,
    Monitor,
    Region,
    Stop,
    Toggle,
    Status,
}


fn sibling_bin(name: &str) -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    // Fall back to PATH lookup (system wrappers after NixOS rebuild).
    std::path::PathBuf::from(name)
}

fn xdg_pictures() -> PathBuf {
    directories::UserDirs::new()
        .and_then(|u| u.picture_dir().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("Pictures"))
}

fn xdg_videos() -> PathBuf {
    directories::UserDirs::new()
        .and_then(|u| u.video_dir().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("Videos"))
}

fn ensure_dir(p: &Path) -> Result<()> {
    fs::create_dir_all(p).with_context(|| format!("mkdir {}", p.display()))
}

fn unique_path(dir: &Path, prefix: &str, ext: &str) -> PathBuf {
    let stamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let mut path = dir.join(format!("{prefix}_{stamp}.{ext}"));
    let mut n = 1u32;
    while path.exists() {
        path = dir.join(format!("{prefix}_{stamp}_{n}.{ext}"));
        n += 1;
    }
    path
}

fn state_dir() -> PathBuf {
    directories::BaseDirs::new()
        .and_then(|b| b.state_dir().map(|p| p.join("matrixshot")))
        .unwrap_or_else(|| PathBuf::from(".local/state/matrixshot"))
}

fn write_last(kind: &str, path: &Path) -> Result<()> {
    let d = state_dir();
    ensure_dir(&d)?;
    fs::write(d.join("last"), format!("{kind}\n{}", path.display()))?;
    Ok(())
}

fn read_last() -> Result<(String, PathBuf)> {
    let text = fs::read_to_string(state_dir().join("last")).context("no last capture")?;
    let mut lines = text.lines();
    let kind = lines.next().unwrap_or("screenshot").to_string();
    let path = PathBuf::from(lines.next().unwrap_or(""));
    if path.as_os_str().is_empty() {
        bail!("corrupt last-capture state");
    }
    Ok((kind, path))
}

fn require_bin(name: &str) -> Result<PathBuf> {
    which::which(name).with_context(|| format!("missing dependency: {name}"))
}

fn screenshot_region(cfg: &Config) -> Result<PathBuf> {
    let grim = require_bin("grim")?;
    let slurp = require_bin("slurp")?;
    let wl_copy = require_bin("wl-copy")?;

    let dir = cfg.screenshot_dir();
    ensure_dir(&dir)?;
    let out = unique_path(&dir, "Screenshot", "png");

    // Green selection border (#00ff00). Escape cancels → non-zero, no file.
    let geom = Command::new(&slurp)
        .args(["-b", "00000066", "-c", "00ff00ff", "-w", "2"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("run slurp")?;
    if !geom.status.success() {
        bail!("selection cancelled");
    }
    let region = String::from_utf8_lossy(&geom.stdout).trim().to_string();
    if region.is_empty() {
        bail!("empty selection");
    }

    let status = Command::new(&grim)
        .args(["-g", &region])
        .arg(&out)
        .status()
        .context("run grim")?;
    if !status.success() || !out.exists() {
        bail!("grim failed");
    }

    if cfg.screenshot.copy_to_clipboard {
        let _ = Command::new(&wl_copy)
            .args(["-t", "image/png"])
            .stdin(Stdio::from(fs::File::open(&out)?))
            .status();
    }

    write_last("screenshot", &out)?;
    // Optional Quickshell preview hook (non-fatal if missing).
    let _ = Command::new(sibling_bin("matrixshot-preview"))
        .arg(&out)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    println!("{}", out.display());
    Ok(out)
}

fn screenshot_fullscreen(cfg: &Config) -> Result<PathBuf> {
    let grim = require_bin("grim")?;
    let wl_copy = require_bin("wl-copy")?;
    let dir = cfg.screenshot_dir();
    ensure_dir(&dir)?;
    let out = unique_path(&dir, "Screenshot", "png");
    let status = Command::new(&grim).arg(&out).status()?;
    if !status.success() || !out.exists() {
        bail!("grim failed");
    }
    if cfg.screenshot.copy_to_clipboard {
        let _ = Command::new(&wl_copy)
            .args(["-t", "image/png"])
            .stdin(Stdio::from(fs::File::open(&out)?))
            .status();
    }
    write_last("screenshot", &out)?;
    let _ = Command::new(sibling_bin("matrixshot-preview")).arg(&out).spawn();
    println!("{}", out.display());
    Ok(out)
}

fn record_pid_file() -> PathBuf {
    state_dir().join("record.pid")
}

fn record_out_file() -> PathBuf {
    state_dir().join("record.out")
}

fn record_status() -> Result<()> {
    let pidf = record_pid_file();
    if !pidf.exists() {
        println!("not recording");
        return Ok(());
    }
    let pid: i32 = fs::read_to_string(&pidf)?.trim().parse().unwrap_or(0);
    if pid > 0 && Path::new(&format!("/proc/{pid}")).exists() {
        let out = fs::read_to_string(record_out_file()).unwrap_or_default();
        println!("recording pid={pid} out={}", out.trim());
    } else {
        println!("not recording (stale pid)");
        let _ = fs::remove_file(pidf);
    }
    Ok(())
}

fn record_stop() -> Result<()> {
    let pidf = record_pid_file();
    if !pidf.exists() {
        println!("not recording");
        return Ok(());
    }
    let pid: i32 = fs::read_to_string(&pidf)?.trim().parse().unwrap_or(0);
    if pid > 0 {
        let _ = Command::new("kill").args(["-INT", &pid.to_string()]).status();
        // wait briefly
        for _ in 0..50 {
            if !Path::new(&format!("/proc/{pid}")).exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    let out = fs::read_to_string(record_out_file()).unwrap_or_default();
    let _ = fs::remove_file(&pidf);
    let _ = Command::new("matrixshot-rec-ui").arg("stop").status();
    if !out.trim().is_empty() {
        write_last("recording", Path::new(out.trim()))?;
        println!("{}", out.trim());
    }
    Ok(())
}

fn record_start(cfg: &Config, mode: &str) -> Result<()> {
    if record_pid_file().exists() {
        bail!("already recording; use matrixshot record stop");
    }
    let gsr = require_bin("gpu-screen-recorder")?;
    let dir = cfg.recording_dir();
    ensure_dir(&dir)?;
    let out = unique_path(&dir, "Recording", "mp4");

    let mut args: Vec<String> = vec!["-w".into()];
    match mode {
        "fullscreen" | "monitor" => args.push("screen".into()), // gsr uses monitor name or screen
        "region" => {
            let slurp = require_bin("slurp")?;
            let geom = Command::new(slurp)
                .args(["-b", "00000066", "-c", "00ff00ff", "-w", "2"])
                .output()?;
            if !geom.status.success() {
                bail!("region cancelled");
            }
            let region = String::from_utf8_lossy(&geom.stdout).trim().to_string();
            args.push("region".into());
            args.push("-region".into());
            args.push(region);
        }
        _ => bail!("unknown mode"),
    }
    args.extend([
        "-f".into(),
        cfg.recording.fps.to_string(),
        "-fallback-cpu-encoding".into(),
        "yes".into(),
        "-o".into(),
        out.display().to_string(),
    ]);
    if cfg.recording.audio {
        // default device list discovery — empty means let gsr pick if supported;
        // user can set explicit sinks later. Use --list-audio-devices offline.
        if let Ok(outp) = Command::new(&gsr).arg("--list-audio-devices").output() {
            let text = String::from_utf8_lossy(&outp.stdout);
            // Prefer a non-monitor? Keep simple: first default-looking line.
            if let Some(line) = text.lines().find(|l| !l.trim().is_empty()) {
                args.push("-a".into());
                args.push(line.trim().to_string());
            }
        }
    }

    ensure_dir(&state_dir())?;
    fs::write(record_out_file(), out.display().to_string())?;
    let child = Command::new(&gsr)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn gpu-screen-recorder")?;
    fs::write(record_pid_file(), child.id().to_string())?;
    let _ = Command::new("matrixshot-rec-ui").args(["start", &out.display().to_string()]).status();
    println!("recording -> {}", out.display());
    Ok(())
}


fn write_upload_state(status: &str, url: &str, error: &str) -> Result<()> {
    let d = state_dir();
    ensure_dir(&d)?;
    let body = format!(
        "{{\"status\":{},\"url\":{},\"error\":{},\"ts\":{}}}",
        serde_json_str(status),
        serde_json_str(url),
        serde_json_str(error),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    fs::write(d.join("upload.json"), body)?;
    Ok(())
}

fn serde_json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn notify(summary: &str, body: &str) {
    let _ = Command::new("notify-send")
        .args(["-a", "MatrixShot", summary, body])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn upload_last(cfg: &Config) -> Result<()> {
    if !cfg.upload.enabled {
        let msg = "uploads disabled in config (set [upload] enabled = true)";
        let _ = write_upload_state("error", "", msg);
        bail!("{msg}");
    }
    let (_kind, path) = read_last()?;
    if !path.is_file() {
        bail!("last capture missing: {}", path.display());
    }
    let _ = write_upload_state("uploading", "", "");
    notify("MatrixShot", "Uploading screenshot…");

    let curl = require_bin("curl")?;
    let primary = cfg.upload.provider.to_lowercase();
    // Prefer configured provider, then fall back so a flaky host doesn't brick Upload.
    let mut providers: Vec<&str> = Vec::new();
    for p in [primary.as_str(), "catbox", "0x0", "litterbox"] {
        if !providers.iter().any(|x| *x == p) {
            providers.push(p);
        }
    }

    let mut last_err = String::from("all upload providers failed");
    let mut url: Option<String> = None;
    let mut used = String::new();

    for provider in providers {
        let result = match provider {
            "0x0" | "0x0.st" => Command::new(&curl)
                .args([
                    "-sS",
                    "-f",
                    "--connect-timeout",
                    "15",
                    "--max-time",
                    "120",
                    "-F",
                    &format!("file=@{}", path.display()),
                    "https://0x0.st",
                ])
                .output(),
            "catbox" => Command::new(&curl)
                .args([
                    "-sS",
                    "-f",
                    "--connect-timeout",
                    "15",
                    "--max-time",
                    "120",
                    "-F",
                    "reqtype=fileupload",
                    "-F",
                    &format!("fileToUpload=@{}", path.display()),
                    "https://catbox.moe/user/api.php",
                ])
                .output(),
            "litterbox" => Command::new(&curl)
                .args([
                    "-sS",
                    "-f",
                    "--connect-timeout",
                    "15",
                    "--max-time",
                    "120",
                    "-F",
                    "reqtype=fileupload",
                    "-F",
                    "time=72h",
                    "-F",
                    &format!("fileToUpload=@{}", path.display()),
                    "https://litterbox.catbox.moe/resources/internals/api.php",
                ])
                .output(),
            "imgur" => {
                if cfg.upload.imgur_client_id.trim().is_empty() {
                    last_err = "imgur needs [upload] imgur_client_id in config".into();
                    continue;
                }
                Command::new(&curl)
                    .args([
                        "-sS",
                        "-f",
                        "--connect-timeout",
                        "15",
                        "--max-time",
                        "120",
                        "-H",
                        &format!("Authorization: Client-ID {}", cfg.upload.imgur_client_id.trim()),
                        "-F",
                        &format!("image=@{}", path.display()),
                        "https://api.imgur.com/3/image",
                    ])
                    .output()
            }
            other => {
                last_err = format!("unknown upload provider: {other}");
                continue;
            }
        };

        let output = match result {
            Ok(o) => o,
            Err(e) => {
                last_err = format!("{provider}: {e}");
                continue;
            }
        };
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            last_err = if err.is_empty() {
                format!("{provider}: HTTP/curl failure")
            } else {
                format!("{provider}: {err}")
            };
            continue;
        }

        let body = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let parsed = if provider == "imgur" {
            body.split("\"link\":\"")
                .nth(1)
                .and_then(|s| s.split('"').next())
                .map(|s| s.replace("\\/", "/"))
                .filter(|s| s.starts_with("http"))
        } else {
            body.lines()
                .find(|l| l.starts_with("http"))
                .map(|s| s.trim().to_string())
                .filter(|s| s.starts_with("http"))
        };
        match parsed {
            Some(u) => {
                url = Some(u);
                used = provider.to_string();
                break;
            }
            None => {
                last_err = format!("{provider}: non-URL response: {body}");
            }
        }
    }

    let url = match url {
        Some(u) => u,
        None => {
            let _ = write_upload_state("error", "", &last_err);
            notify("MatrixShot upload failed", &last_err);
            bail!("{last_err}");
        }
    };

    if cfg.upload.copy_url {
        let wl_copy = require_bin("wl-copy")?;
        let status = Command::new(&wl_copy)
            .arg(&url)
            .status()
            .context("wl-copy url")?;
        if !status.success() {
            let msg = "uploaded but failed to copy URL to clipboard";
            let _ = write_upload_state("ok", &url, msg);
            notify("MatrixShot", &format!("Uploaded via {used} (clipboard failed):\n{url}"));
            println!("{url}");
            return Ok(());
        }
    }

    let _ = write_upload_state("ok", &url, "");
    notify("MatrixShot", &format!("URL copied ({used}):\n{url}"));
    println!("{url}");
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = Config::load_or_init()?;
    match cli.cmd.unwrap_or(Cmd::Region) {
        Cmd::Region => {
            screenshot_region(&cfg)?;
        }
        Cmd::Fullscreen => {
            screenshot_fullscreen(&cfg)?;
        }
        Cmd::OpenLast => {
            let (_k, path) = read_last()?;
            Command::new(&cfg.viewer.command).arg(&path).spawn()?;
        }
        Cmd::Folder => {
            let (_k, path) = read_last()?;
            let dir = path.parent().unwrap_or(Path::new("."));
            // Prefer nautilus select when available.
            if which::which("nautilus").is_ok() {
                let _ = Command::new("nautilus").args(["--select"]).arg(&path).spawn();
            } else {
                let _ = Command::new("xdg-open").arg(dir).spawn();
            }
        }
        Cmd::UploadLast => {
            upload_last(&cfg)?;
        }
        Cmd::Config => {
            println!("{}", Config::path().display());
        }
        Cmd::Record { action } => match action.unwrap_or(RecordCmd::Toggle) {
            RecordCmd::Status => record_status()?,
            RecordCmd::Stop => record_stop()?,
            RecordCmd::Toggle => {
                if record_pid_file().exists() {
                    record_stop()?;
                } else {
                    record_start(&cfg, "fullscreen")?;
                }
            }
            RecordCmd::Fullscreen => record_start(&cfg, "fullscreen")?,
            RecordCmd::Monitor => record_start(&cfg, "monitor")?,
            RecordCmd::Region => record_start(&cfg, "region")?,
        },
    }
    Ok(())
}
