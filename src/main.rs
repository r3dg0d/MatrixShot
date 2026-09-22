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
    /// Upload last capture (manual only; disabled by default)
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
    let _ = Command::new("matrixshot-preview")
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
    let _ = Command::new("matrixshot-preview").arg(&out).spawn();
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
    println!("recording -> {}", out.display());
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
            if !cfg.upload.enabled {
                bail!("uploads disabled in config (set [upload] enabled=true)");
            }
            bail!("upload providers not configured yet — local file left untouched");
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
