use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub screenshot: Screenshot,
    pub selection: Selection,
    pub preview: Preview,
    pub viewer: Viewer,
    pub recording: Recording,
    pub upload: Upload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Screenshot {
    pub save_directory: String,
    pub copy_to_clipboard: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Selection {
    pub border: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preview {
    pub enabled: bool,
    pub timeout: u32,
    pub position: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Viewer {
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Recording {
    pub backend: String,
    pub save_directory: String,
    pub fps: u32,
    pub audio: bool,
    pub microphone: bool,
    pub codec: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Upload {
    pub enabled: bool,
    pub provider: String,
    pub copy_url: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            screenshot: Screenshot::default(),
            selection: Selection::default(),
            preview: Preview::default(),
            viewer: Viewer::default(),
            recording: Recording::default(),
            upload: Upload::default(),
        }
    }
}
impl Default for Screenshot {
    fn default() -> Self {
        Self {
            save_directory: "~/Pictures/Screenshots".into(),
            copy_to_clipboard: true,
        }
    }
}
impl Default for Selection {
    fn default() -> Self {
        Self {
            border: "#00ff00".into(),
        }
    }
}
impl Default for Preview {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout: 10,
            position: "top-right".into(),
        }
    }
}
impl Default for Viewer {
    fn default() -> Self {
        Self {
            command: "imv".into(),
        }
    }
}
impl Default for Recording {
    fn default() -> Self {
        Self {
            backend: "gpu-screen-recorder".into(),
            save_directory: "~/Videos/MatrixShot".into(),
            fps: 60,
            audio: true,
            microphone: false,
            codec: "auto".into(),
        }
    }
}
impl Default for Upload {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "imgur".into(),
            copy_url: true,
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        directories::BaseDirs::new()
            .map(|b| b.config_dir().join("matrixshot/config.toml"))
            .unwrap_or_else(|| PathBuf::from(".config/matrixshot/config.toml"))
    }

    pub fn load_or_init() -> Result<Self> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if !path.exists() {
            let cfg = Self::default();
            fs::write(&path, toml::to_string_pretty(&cfg)?)?;
            return Ok(cfg);
        }
        let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        Ok(toml::from_str(&text)?)
    }

    fn expand(p: &str) -> PathBuf {
        if let Some(rest) = p.strip_prefix("~/") {
            if let Some(home) = directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf()) {
                return home.join(rest);
            }
        }
        PathBuf::from(p)
    }

    pub fn screenshot_dir(&self) -> PathBuf {
        Self::expand(&self.screenshot.save_directory)
    }

    pub fn recording_dir(&self) -> PathBuf {
        Self::expand(&self.recording.save_directory)
    }
}
