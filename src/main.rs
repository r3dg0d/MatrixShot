mod config;

use anyhow::{bail, Context, Result};
use chrono::Local;
use clap::{Parser, Subcommand, ValueEnum};
use config::Config;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Parser, Debug)]
#[command(
    name = "matrixshot",
    version,
    about = "Wayland screenshots and recordings"
)]
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
        /// Video codec for gpu-screen-recorder `-k` (auto skips the flag)
        #[arg(long, short = 'k')]
        codec: Option<String>,
        #[arg(long)]
        output_dir: Option<String>,
    },
    /// Focused monitor (gsr `-w focused`); differs from Fullscreen (`-w screen`)
    Monitor {
        #[arg(long)]
        fps: Option<u32>,
        #[arg(long, value_enum)]
        audio: Option<AudioMode>,
        /// Video codec for gpu-screen-recorder `-k` (auto skips the flag)
        #[arg(long, short = 'k')]
        codec: Option<String>,
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
        /// Video codec for gpu-screen-recorder `-k` (auto skips the flag)
        #[arg(long, short = 'k')]
        codec: Option<String>,
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
    if let Ok(p) = which::which(name) {
        return Ok(p);
    }
    // Hyprland sessions often omit ~/.local/bin from PATH.
    if let Some(home) = directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf()) {
        let local = home.join(".local/bin").join(name);
        if local.is_file() {
            return Ok(local);
        }
    }
    let nix = PathBuf::from("/run/current-system/sw/bin").join(name);
    if nix.is_file() {
        return Ok(nix);
    }
    bail!("missing dependency: {name}")
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

fn copy_screenshot(cfg: &Config, out: &Path) {
    if !cfg.screenshot.copy_to_clipboard {
        return;
    }
    let result = (|| -> Result<()> {
        let wl_copy = require_bin("wl-copy")?;
        let status = Command::new(wl_copy)
            .args(["-t", "image/png"])
            .stdin(Stdio::from(fs::File::open(out)?))
            .status()
            .context("run wl-copy")?;
        if !status.success() {
            bail!("wl-copy exited with {status}");
        }
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!(
            "warning: screenshot saved to {}, but clipboard copy failed: {error:#}",
            out.display()
        );
    }
}

fn screenshot_with_geometry(cfg: &Config, region: &str) -> Result<PathBuf> {
    let grim = require_bin("grim")?;

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

    copy_screenshot(cfg, &out);

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
    let dir = cfg.screenshot_dir();
    ensure_dir(&dir)?;
    let out = unique_path(&dir, "Screenshot", "png");
    let status = Command::new(&grim).arg(&out).status()?;
    if !status.success() || !out.exists() {
        bail!("grim failed");
    }
    copy_screenshot(cfg, &out);
    write_last("screenshot", &out)?;
    if cfg.preview.enabled {
        let _ = Command::new(sibling_bin("matrixshot-preview"))
            .arg(&out)
            .spawn();
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
        let _ = Command::new(sibling_bin("matrixshot-rec-ui"))
            .arg("stop")
            .status();
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
        let _ = Command::new("kill")
            .args(["-INT", &pid.to_string()])
            .status();
        for _ in 0..50 {
            if !Path::new(&format!("/proc/{pid}")).exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    let out = fs::read_to_string(record_out_file()).unwrap_or_default();
    let _ = fs::remove_file(&pidf);
    let _ = Command::new(sibling_bin("matrixshot-rec-ui"))
        .arg("stop")
        .status();
    clear_chooser();
    if !out.trim().is_empty() {
        write_last("recording", Path::new(out.trim()))?;
        println!("{}", out.trim());
    }
    Ok(())
}

fn resolve_audio(cfg: &Config, override_mode: Option<AudioMode>) -> Option<String> {
    let mode = override_mode.unwrap_or(match (cfg.recording.audio, cfg.recording.microphone) {
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
    /// When Some and not "auto", passed as gsr `-k`
    codec: Option<String>,
    output_dir: PathBuf,
    geometry: Option<String>,
}

fn resolve_codec(cfg: &Config, override_codec: Option<String>) -> Option<String> {
    let raw = override_codec
        .unwrap_or_else(|| cfg.recording.codec.clone())
        .trim()
        .to_string();
    if raw.is_empty() || raw.eq_ignore_ascii_case("auto") {
        None
    } else {
        Some(raw)
    }
}

fn build_record_opts(
    cfg: &Config,
    fps: Option<u32>,
    audio: Option<AudioMode>,
    codec: Option<String>,
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
        codec: resolve_codec(cfg, codec),
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
        "fullscreen" => args.push("screen".into()),
        "monitor" => args.push("focused".into()),
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
    if let Some(k) = opts.codec {
        args.push("-k".into());
        args.push(k);
    }
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
        let _ = Command::new(sibling_bin("matrixshot-rec-ui"))
            .arg("stop")
            .status();
        let log_tail = fs::read_to_string(record_log_file()).unwrap_or_default();
        let msg = if log_tail.trim().is_empty() {
            "gpu-screen-recorder exited immediately".to_string()
        } else {
            format!(
                "gpu-screen-recorder exited immediately:\n{}",
                log_tail.trim()
            )
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

fn curl_base(curl: &Path) -> Command {
    let mut cmd = Command::new(curl);
    // Short connect so dead hosts (catbox from many networks) fail fast.
    // No -f: keep response bodies for clear errors (e.g. 0x0 503).
    // -4: prefer IPv4; avoids IPv6 stalls on dual-stack hosts.
    cmd.args(["-sS", "--connect-timeout", "3", "--max-time", "45", "-4"]);
    cmd
}

fn parse_http_url(body: &str) -> Option<String> {
    body.lines()
        .map(str::trim)
        .find(|l| l.starts_with("http://") || l.starts_with("https://"))
        .map(|s| {
            s.trim_end_matches(['\r', '\n', ' ', '\t', '"', '\''])
                .to_string()
        })
}

/// Extract a JSON string value for key `"name"` (tolerates pretty-printed spaces).
fn json_string_field(body: &str, name: &str) -> Option<String> {
    let needle = format!("\"{name}\"");
    let mut rest = body;
    while let Some(idx) = rest.find(&needle) {
        rest = &rest[idx + needle.len()..];
        let trimmed = rest.trim_start();
        if !trimmed.starts_with(':') {
            continue;
        }
        let after = trimmed[1..].trim_start();
        let Some(s) = after.strip_prefix('"') else {
            continue;
        };
        let mut out = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => {
                    return Some(out.replace("\\/", "/"));
                }
                '\\' => {
                    if let Some(n) = chars.next() {
                        out.push(n);
                    }
                }
                c => out.push(c),
            }
        }
    }
    None
}

fn parse_uguu_url(body: &str) -> Option<String> {
    json_string_field(body, "url").filter(|s| s.starts_with("http"))
}

fn parse_tmpfiles_url(body: &str) -> Option<String> {
    let page = json_string_field(body, "url").filter(|s| s.starts_with("http"))?;
    if let Some(rest) = page.strip_prefix("https://tmpfiles.org/") {
        if !rest.starts_with("dl/") {
            return Some(format!("https://tmpfiles.org/dl/{rest}"));
        }
    }
    Some(page)
}

fn parse_imgur_url(body: &str) -> Option<String> {
    json_string_field(body, "link").filter(|s| s.starts_with("http"))
}

fn upload_one(
    curl: &Path,
    provider: &str,
    path: &Path,
    imgur_client_id: &str,
) -> Result<String, String> {
    let file = path.display().to_string();
    let result = match provider {
        "uguu" | "uguu.se" => curl_base(curl)
            .args(["-F", &format!("files[]=@{file}"), "https://uguu.se/upload"])
            .output(),
        "catbox" => curl_base(curl)
            .args([
                "-F",
                "reqtype=fileupload",
                "-F",
                &format!("fileToUpload=@{file}"),
                "https://catbox.moe/user/api.php",
            ])
            .output(),
        "0x0" | "0x0.st" => curl_base(curl)
            .args(["-F", &format!("file=@{file}"), "https://0x0.st"])
            .output(),
        "litterbox" => curl_base(curl)
            .args([
                "-F",
                "reqtype=fileupload",
                "-F",
                "time=72h",
                "-F",
                &format!("fileToUpload=@{file}"),
                "https://litterbox.catbox.moe/resources/internals/api.php",
            ])
            .output(),
        "tmpfiles" | "tmpfiles.org" => curl_base(curl)
            .args([
                "-F",
                &format!("file=@{file}"),
                "https://tmpfiles.org/api/v1/upload",
            ])
            .output(),
        "imgur" => {
            if imgur_client_id.trim().is_empty() {
                return Err("imgur needs [upload] imgur_client_id in config".into());
            }
            curl_base(curl)
                .args([
                    "-H",
                    &format!("Authorization: Client-ID {}", imgur_client_id.trim()),
                    "-F",
                    &format!("image=@{file}"),
                    "https://api.imgur.com/3/image",
                ])
                .output()
        }
        other => return Err(format!("unknown upload provider: {other}")),
    };

    let output = match result {
        Ok(o) => o,
        Err(e) => return Err(format!("spawn curl: {e}")),
    };

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout.chars().take(200).collect()
        } else {
            "HTTP/curl failure".into()
        };
        return Err(detail);
    }

    let parsed = match provider {
        "uguu" | "uguu.se" => parse_uguu_url(&stdout),
        "tmpfiles" | "tmpfiles.org" => parse_tmpfiles_url(&stdout),
        "imgur" => parse_imgur_url(&stdout),
        _ => parse_http_url(&stdout),
    };

    match parsed {
        Some(u) => Ok(u),
        None => {
            let snippet: String = stdout.chars().take(180).collect();
            if snippet.is_empty() {
                Err("empty response".into())
            } else {
                Err(format!("non-URL response: {snippet}"))
            }
        }
    }
}

fn provider_race_order(primary: &str) -> Vec<String> {
    // Race anonymous hosts together. Catbox/litterbox often blackhole-timeout;
    // 0x0 currently returns 503 (uploads disabled). uguu is typically the
    // fastest working host — it wins the race without waiting on dead peers.
    let mut out: Vec<String> = Vec::new();
    let primary = primary.to_lowercase();
    if !primary.is_empty() && primary != "imgur" {
        out.push(primary);
    }
    // tmpfiles is opt-in only: uploads fast but returns HTML landing pages,
    // not direct image URLs — poor for screenshot sharing.
    for p in ["uguu", "catbox", "0x0", "litterbox"] {
        if !out.iter().any(|x| x == p) {
            out.push(p.to_string());
        }
    }
    out
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
    let mut providers = provider_race_order(&cfg.upload.provider);
    if cfg.upload.provider.eq_ignore_ascii_case("imgur") {
        providers.insert(0, "imgur".into());
    }

    let (tx, rx) = std::sync::mpsc::channel::<(String, Result<String, String>)>();
    let imgur_id = cfg.upload.imgur_client_id.clone();
    for provider in providers {
        let tx = tx.clone();
        let curl = curl.clone();
        let path = path.clone();
        let imgur_id = imgur_id.clone();
        std::thread::spawn(move || {
            let res = upload_one(&curl, &provider, &path, &imgur_id);
            let _ = tx.send((provider, res));
        });
    }
    drop(tx);

    let mut errors: Vec<String> = Vec::new();
    let mut url: Option<String> = None;
    let mut used = String::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(50);
    while std::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        match rx.recv_timeout(remaining.min(std::time::Duration::from_millis(200))) {
            Ok((provider, Ok(u))) => {
                url = Some(u);
                used = provider;
                break;
            }
            Ok((provider, Err(e))) => {
                errors.push(format!("{provider}: {e}"));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let url = match url {
        Some(u) => u,
        None => {
            let last_err = if errors.is_empty() {
                "all upload providers failed (timeout)".to_string()
            } else {
                errors.join(" | ")
            };
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
            notify(
                "MatrixShot",
                &format!("Uploaded via {used} (clipboard failed):\n{url}"),
            );
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
                let _ = Command::new("nautilus")
                    .args(["--select"])
                    .arg(&path)
                    .spawn();
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
                            build_record_opts(&cfg, None, None, None, None, None),
                        )?;
                    }
                } else {
                    record_start(
                        &cfg,
                        "fullscreen",
                        build_record_opts(&cfg, None, None, None, None, None),
                    )?;
                }
            }
            RecordCmd::Fullscreen {
                fps,
                audio,
                codec,
                output_dir,
            } => record_start(
                &cfg,
                "fullscreen",
                build_record_opts(&cfg, fps, audio, codec, output_dir, None),
            )?,
            RecordCmd::Monitor {
                fps,
                audio,
                codec,
                output_dir,
            } => record_start(
                &cfg,
                "monitor",
                build_record_opts(&cfg, fps, audio, codec, output_dir, None),
            )?,
            RecordCmd::Region {
                geometry,
                fps,
                audio,
                codec,
                output_dir,
            } => record_start(
                &cfg,
                "region",
                build_record_opts(&cfg, fps, audio, codec, output_dir, geometry),
            )?,
        },
    }
    Ok(())
}
