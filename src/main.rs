mod config;

use anyhow::{bail, Context, Result};
use chrono::Local;
use clap::{Parser, Subcommand, ValueEnum};
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
    /// Region select → Screenshot | Screen Record chooser (Print default)
    Choose,
    /// Region screenshot (optional --geometry skips slurp)
    Region {
        /// Geometry from slurp (`WxH+X+Y`); skips interactive selection
        #[arg(long)]
        geometry: Option<String>,
    },
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
    Fullscreen {
        #[arg(long)]
        fps: Option<u32>,
        #[arg(long, value_enum)]
        audio: Option<AudioMode>,
        #[arg(long)]
        output_dir: Option<String>,
    },
    Monitor {
        #[arg(long)]
        fps: Option<u32>,
        #[arg(long, value_enum)]
        audio: Option<AudioMode>,
        #[arg(long)]
        output_dir: Option<String>,
    },
    Region {
        #[arg(long)]
        geometry: Option<String>,
        #[arg(long)]
        fps: Option<u32>,
        #[arg(long, value_enum)]
        audio: Option<AudioMode>,
        #[arg(long)]
        output_dir: Option<String>,
    },
    Stop,
    Toggle,
    Status,
    /// List audio devices from gpu-screen-recorder (id|label)
    ListAudio,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AudioMode {
    None,
    Desktop,
    Mic,
    Both,
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
    std::path::PathBuf::from(name)
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

fn slurp_border(cfg: &Config) -> String {
    let raw = cfg.selection.border.trim();
    let hex = raw.trim_start_matches('#');
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        format!("{hex}ff")
    } else if hex.len() == 8 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        hex.to_string()
    } else {
        "00ff00ff".into()
    }
}

fn run_slurp(cfg: &Config) -> Result<String> {
    let slurp = require_bin("slurp")?;
    let border = slurp_border(cfg);
    let geom = Command::new(&slurp)
        .args(["-b", "00000066", "-c", &border, "-w", "2"])
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
    Ok(region)
}

fn ensure_overlay() {
    let _ = Command::new(sibling_bin("matrixshot-ui"))
        .arg("ensure")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn write_chooser(geometry: &str, phase: &str) -> Result<()> {
    let d = state_dir();
    ensure_dir(&d)?;
    let body = format!(
        "{{\"geometry\":{},\"phase\":{},\"ts\":{}}}",
        serde_json_str(geometry),
        serde_json_str(phase),
        now_secs()
    );
    fs::write(d.join("chooser.json"), body)?;
    Ok(())
}

fn clear_chooser() {
    let _ = fs::remove_file(state_dir().join("chooser.json"));
}

fn choose_flow(cfg: &Config) -> Result<()> {
    let region = run_slurp(cfg)?;
    write_chooser(&region, "choose")?;
    ensure_overlay();
    println!("{region}");
    Ok(())
}

fn screenshot_with_geometry(cfg: &Config, region: &str) -> Result<PathBuf> {
    let grim = require_bin("grim")?;
    let wl_copy = require_bin("wl-copy")?;

    let dir = cfg.screenshot_dir();
    ensure_dir(&dir)?;
    let out = unique_path(&dir, "Screenshot", "png");

    let grim_geom = to_grim_region(region);
    let status = Command::new(&grim)
        .args(["-g", &grim_geom])
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
    clear_chooser();
    if cfg.preview.enabled {
        let _ = Command::new(sibling_bin("matrixshot-preview"))
            .arg(&out)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
    println!("{}", out.display());
    Ok(out)
}

fn screenshot_region(cfg: &Config, geometry: Option<String>) -> Result<PathBuf> {
    let region = match geometry {
        Some(g) if !g.trim().is_empty() => g.trim().to_string(),
        _ => run_slurp(cfg)?,
    };
    screenshot_with_geometry(cfg, &region)
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
    if cfg.preview.enabled {
        let _ = Command::new(sibling_bin("matrixshot-preview")).arg(&out).spawn();
    }
    println!("{}", out.display());
    Ok(out)
}

fn record_pid_file() -> PathBuf {
    state_dir().join("record.pid")
}

fn record_out_file() -> PathBuf {
    state_dir().join("record.out")
}

fn record_log_file() -> PathBuf {
    state_dir().join("record.log")
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
        let _ = Command::new(sibling_bin("matrixshot-rec-ui")).arg("stop").status();
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
        for _ in 0..50 {
            if !Path::new(&format!("/proc/{pid}")).exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    let out = fs::read_to_string(record_out_file()).unwrap_or_default();
    let _ = fs::remove_file(&pidf);
    let _ = Command::new(sibling_bin("matrixshot-rec-ui")).arg("stop").status();
    clear_chooser();
    if !out.trim().is_empty() {
        write_last("recording", Path::new(out.trim()))?;
        println!("{}", out.trim());
    }
    Ok(())
}

fn resolve_audio(cfg: &Config, override_mode: Option<AudioMode>) -> Option<String> {
    let mode = override_mode.unwrap_or_else(|| match (cfg.recording.audio, cfg.recording.microphone) {
        (false, false) => AudioMode::None,
        (true, false) => AudioMode::Desktop,
        (false, true) => AudioMode::Mic,
        (true, true) => AudioMode::Both,
    });
    match mode {
        AudioMode::None => None,
        AudioMode::Desktop => Some("default_output".into()),
        AudioMode::Mic => Some("default_input".into()),
        AudioMode::Both => Some("default_output|default_input".into()),
    }
}

struct RecordOpts {
    fps: u32,
    audio: Option<String>,
    output_dir: PathBuf,
    geometry: Option<String>,
}

fn build_record_opts(
    cfg: &Config,
    fps: Option<u32>,
    audio: Option<AudioMode>,
    output_dir: Option<String>,
    geometry: Option<String>,
) -> RecordOpts {
    let dir = output_dir
        .map(|s| {
            if let Some(rest) = s.strip_prefix("~/") {
                directories::BaseDirs::new()
                    .map(|b| b.home_dir().join(rest))
                    .unwrap_or_else(|| PathBuf::from(&s))
            } else {
                PathBuf::from(s)
            }
        })
        .unwrap_or_else(|| cfg.recording_dir());
    RecordOpts {
        fps: fps.unwrap_or(cfg.recording.fps),
        audio: resolve_audio(cfg, audio),
        output_dir: dir,
        geometry,
    }
}

fn record_start(cfg: &Config, mode: &str, opts: RecordOpts) -> Result<()> {
    if record_pid_file().exists() {
        let pid: i32 = fs::read_to_string(record_pid_file())
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        if pid > 0 && Path::new(&format!("/proc/{pid}")).exists() {
            bail!("already recording; use matrixshot record stop");
        }
        let _ = fs::remove_file(record_pid_file());
    }
    let gsr = require_bin("gpu-screen-recorder")?;
    let dir = opts.output_dir;
    ensure_dir(&dir)?;
    let out = unique_path(&dir, "Recording", "mp4");

    let mut args: Vec<String> = vec!["-w".into()];
    match mode {
        "fullscreen" | "monitor" => args.push("screen".into()),
        "region" => {
            let region = match opts.geometry {
                Some(g) if !g.trim().is_empty() => g.trim().to_string(),
                _ => run_slurp(cfg)?,
            };
            let gsr_region = to_gsr_region(&region);
            args.push("region".into());
            args.push("-region".into());
            args.push(gsr_region);
        }
        _ => bail!("unknown mode"),
    }
    args.extend([
        "-f".into(),
        opts.fps.to_string(),
        "-o".into(),
        out.display().to_string(),
    ]);
    if let Some(a) = opts.audio {
        args.push("-a".into());
        args.push(a);
    }

    ensure_dir(&state_dir())?;
    fs::write(record_out_file(), out.display().to_string())?;
    let log = fs::File::create(record_log_file()).ok();
    let stderr = match log {
        Some(f) => Stdio::from(f),
        None => Stdio::null(),
    };
    let child = Command::new(&gsr)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr)
        .spawn()
        .context("spawn gpu-screen-recorder")?;
    let pid = child.id();
    fs::write(record_pid_file(), pid.to_string())?;

    std::thread::sleep(std::time::Duration::from_millis(400));
    if !Path::new(&format!("/proc/{pid}")).exists() {
        let _ = fs::remove_file(record_pid_file());
        let _ = Command::new(sibling_bin("matrixshot-rec-ui")).arg("stop").status();
        let log_tail = fs::read_to_string(record_log_file()).unwrap_or_default();
        let msg = if log_tail.trim().is_empty() {
            "gpu-screen-recorder exited immediately".to_string()
        } else {
            format!("gpu-screen-recorder exited immediately:\n{}", log_tail.trim())
        };
        notify("MatrixShot record failed", &msg);
        bail!("{msg}");
    }

    clear_chooser();
    let _ = Command::new(sibling_bin("matrixshot-rec-ui"))
        .args(["start", &out.display().to_string()])
        .status();
    ensure_overlay();
    println!("recording -> {}", out.display());
    Ok(())
}

fn to_gsr_region(geometry: &str) -> String {
    let g = geometry.trim();
    // Already gsr form: WxH+X+Y
    if g.contains('+') && g.contains('x') && !g.contains(',') {
        return g.to_string();
    }
    // Slurp/grim form: "X,Y WxH"
    if let Some((xy, wh)) = g.split_once(' ') {
        if let Some((x, y)) = xy.split_once(',') {
            if let Some((w, h)) = wh.split_once('x') {
                let (x, y, w, h) = (x.trim(), y.trim(), w.trim(), h.trim());
                if !x.is_empty() && !y.is_empty() && !w.is_empty() && !h.is_empty() {
                    return format!("{w}x{h}+{x}+{y}");
                }
            }
        }
    }
    g.to_string()
}

fn to_grim_region(geometry: &str) -> String {
    let g = geometry.trim();
    // Already grim/slurp form
    if g.contains(',') && g.contains(' ') {
        return g.to_string();
    }
    // gsr form WxH+X+Y → X,Y WxH
    if let Some((wh, rest)) = g.split_once('+') {
        if let Some((x, y)) = rest.split_once('+') {
            if let Some((w, h)) = wh.split_once('x') {
                return format!("{},{} {}x{}", x.trim(), y.trim(), w.trim(), h.trim());
            }
        }
    }
    g.to_string()
}

fn list_audio_devices() -> Result<()> {
    let gsr = require_bin("gpu-screen-recorder")?;
    let output = Command::new(&gsr)
        .arg("--list-audio-devices")
        .output()
        .context("list audio devices")?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        bail!("gpu-screen-recorder --list-audio-devices failed: {err}");
    }
    print!("{}", String::from_utf8_lossy(&output.stdout));
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
        now_secs()
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
                    "-sS", "-f", "--connect-timeout", "15", "--max-time", "120",
                    "-F", &format!("file=@{}", path.display()),
                    "https://0x0.st",
                ])
                .output(),
            "catbox" => Command::new(&curl)
                .args([
                    "-sS", "-f", "--connect-timeout", "15", "--max-time", "120",
                    "-F", "reqtype=fileupload",
                    "-F", &format!("fileToUpload=@{}", path.display()),
                    "https://catbox.moe/user/api.php",
                ])
                .output(),
            "litterbox" => Command::new(&curl)
                .args([
                    "-sS", "-f", "--connect-timeout", "15", "--max-time", "120",
                    "-F", "reqtype=fileupload", "-F", "time=72h",
                    "-F", &format!("fileToUpload=@{}", path.display()),
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
                        "-sS", "-f", "--connect-timeout", "15", "--max-time", "120",
                        "-H", &format!("Authorization: Client-ID {}", cfg.upload.imgur_client_id.trim()),
                        "-F", &format!("image=@{}", path.display()),
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
    match cli.cmd.unwrap_or(Cmd::Choose) {
        Cmd::Choose => {
            choose_flow(&cfg)?;
        }
        Cmd::Region { geometry } => {
            screenshot_region(&cfg, geometry)?;
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
            RecordCmd::ListAudio => list_audio_devices()?,
            RecordCmd::Toggle => {
                if record_pid_file().exists() {
                    let pid: i32 = fs::read_to_string(record_pid_file())
                        .ok()
                        .and_then(|s| s.trim().parse().ok())
                        .unwrap_or(0);
                    if pid > 0 && Path::new(&format!("/proc/{pid}")).exists() {
                        record_stop()?;
                    } else {
                        let _ = fs::remove_file(record_pid_file());
                        record_start(
                            &cfg,
                            "fullscreen",
                            build_record_opts(&cfg, None, None, None, None),
                        )?;
                    }
                } else {
                    record_start(
                        &cfg,
                        "fullscreen",
                        build_record_opts(&cfg, None, None, None, None),
                    )?;
                }
            }
            RecordCmd::Fullscreen { fps, audio, output_dir } => record_start(
                &cfg,
                "fullscreen",
                build_record_opts(&cfg, fps, audio, output_dir, None),
            )?,
            RecordCmd::Monitor { fps, audio, output_dir } => record_start(
                &cfg,
                "monitor",
                build_record_opts(&cfg, fps, audio, output_dir, None),
            )?,
            RecordCmd::Region {
                geometry,
                fps,
                audio,
                output_dir,
            } => record_start(
                &cfg,
                "region",
                build_record_opts(&cfg, fps, audio, output_dir, geometry),
            )?,
        },
    }
    Ok(())
}
