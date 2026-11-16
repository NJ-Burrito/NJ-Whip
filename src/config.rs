use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::messages::{Pack, Profile, Style};
use crate::whips::WhipId;
use std::time::SystemTime;
use std::{fs, io};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum MacroStep {
    Key(String),
    Phrase,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub crack_hotkey: String,
    pub toggle_hotkey: String,
    pub macro_steps: Vec<MacroStep>,
    /// Crack volume, 0..1.
    pub volume: f32,
    /// Volume of the special whips' own sounds (trumpet, yeehaw, gong...), 0..1.
    pub sfx_volume: f32,
    pub rage: bool,
    pub swing_crack: bool,
    pub autostart: bool,
    /// ON = whip on screen and hotkeys active; OFF = nothing on screen, hotkeys released.
    pub enabled: bool,
    pub whip: WhipId,
    pub pack: Pack,
    /// Light or Heavy session: picks the Plain pack's wording.
    pub profile: Profile,
    /// Send every intent after Esc (off: FASTER, HARDER and HAPPIER are queued without it).
    pub always_esc: bool,
    /// HARDER messages end with the word `ultrathink`.
    pub harder_ultrathink: bool,
    pub theme: Theme,
    /// Per-whip replacement fun lines (missing = the whip's own lines).
    pub custom_lines: HashMap<WhipId, Vec<String>>,
    /// Per-whip custom tip image (a processed PNG under APPDATA/NJ-Whip/tips).
    pub custom_tips: HashMap<WhipId, PathBuf>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            crack_hotkey: "Ctrl+Alt+W".into(),
            toggle_hotkey: "Ctrl+Alt+Q".into(),
            macro_steps: vec![
                MacroStep::Key("Esc".into()),
                MacroStep::Phrase,
                MacroStep::Key("Enter".into()),
            ],
            volume: 0.8,
            sfx_volume: 0.8,
            rage: true,
            swing_crack: true,
            autostart: false,
            enabled: true,
            whip: WhipId::Classic,
            pack: Pack::Plain,
            profile: Profile::Light,
            always_esc: false,
            harder_ultrathink: false,
            theme: Theme::System,
            custom_lines: HashMap::new(),
            custom_tips: HashMap::new(),
        }
    }
}

impl Config {
    pub fn style(&self) -> Style {
        Style { pack: self.pack, profile: self.profile, always_esc: self.always_esc, harder_ultrathink: self.harder_ultrathink }
    }

    /// The user's own fun lines for the current whip; None (the whip's own lines are used)
    /// when there are none.
    pub fn user_lines(&self) -> Option<&[String]> {
        self.custom_lines.get(&self.whip).filter(|l| !l.is_empty()).map(Vec::as_slice)
    }
}

pub fn config_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("NJ-Whip").join("config.toml")
}

/// When config.toml was last written (None while it does not exist): the engine and the window
/// each watch it for the other's and for outside edits.
pub fn mtime() -> Option<SystemTime> {
    fs::metadata(config_path()).and_then(|m| m.modified()).ok()
}

/// config.toml and when it was written. The time is read first: a write that lands in between
/// then shows up as a change at the next check instead of being missed.
/// A file that cannot be read is an error and a copy of it is kept as `config.broken.toml`:
/// the caller must not carry on with defaults as if they were the user's settings, because
/// the next save would replace the file for good.
pub fn load() -> (Result<Config, String>, Option<SystemTime>) {
    let at = mtime();
    let path = config_path();
    let cfg = load_from(&path).inspect_err(|_| {
        let _ = fs::copy(&path, path.with_file_name("config.broken.toml"));
    });
    (cfg, at)
}

/// A missing file is the defaults (first run); a file that does not parse is an error.
pub fn load_from(path: &Path) -> Result<Config, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => return Err(e.to_string()),
    };
    let mut c: Config = toml::from_str(&text).map_err(|e| e.message().to_string())?;
    c.volume = if c.volume.is_nan() { Config::default().volume } else { c.volume.clamp(0.0, 1.0) };
    c.sfx_volume = if c.sfx_volume.is_nan() { Config::default().sfx_volume } else { c.sfx_volume.clamp(0.0, 1.0) };
    Ok(c)
}

pub fn save_to(path: &Path, c: &Config) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let s = toml::to_string_pretty(c).map_err(io::Error::other)?;
    fs::write(path, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("nj-whip-test-{}-{name}", std::process::id()))
    }

    #[test]
    fn whip_and_maps_roundtrip() {
        let dir = std::env::temp_dir().join(format!("njw-maps-{}", std::process::id()));
        let path = dir.join("c.toml");
        let mut c = Config {
            whip: WhipId::Sauce,
            pack: Pack::PlanFirst,
            profile: Profile::Heavy,
            always_esc: true,
            harder_ultrathink: true,
            theme: Theme::Dark,
            enabled: false,
            ..Config::default()
        };
        c.custom_lines.insert(WhipId::Neon, vec!["Zap".into()]);
        c.custom_tips.insert(WhipId::Penguin, PathBuf::from("C:/x/p.png"));
        save_to(&path, &c).unwrap();
        assert_eq!(load_from(&path), Ok(c));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn roundtrip() {
        let dir = tmp("rt");
        let p = dir.join("sub").join("config.toml");
        let c = Config { volume: 0.3, rage: false, ..Config::default() };
        save_to(&p, &c).unwrap();
        assert_eq!(load_from(&p), Ok(c));
        let _ = fs::remove_dir_all(dir);
    }

    /// A config file from before the six packs: the old pack name and no new fields.
    #[test]
    fn old_config_file_loads() {
        let dir = tmp("old");
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config.toml");
        fs::write(&p, "volume = 0.4\nwhip = \"Neon\"\npack = \"NjKit\"\ntheme = \"Dark\"\n").unwrap();
        let c = load_from(&p).unwrap();
        let want = Config { volume: 0.4, whip: WhipId::Neon, pack: Pack::NjKit, theme: Theme::Dark, ..Config::default() };
        assert_eq!(c, want);
        assert_eq!((c.profile, c.always_esc, c.harder_ultrathink), (Profile::Light, false, false));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_file_gives_default() {
        assert_eq!(load_from(&tmp("missing").join("none.toml")), Ok(Config::default()));
    }

    #[test]
    fn volume_is_clamped() {
        let dir = tmp("vol");
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config.toml");
        for (text, want) in [("volume = 7.5", 1.0), ("volume = -2.0", 0.0), ("volume = nan", Config::default().volume), ("volume = 0.4", 0.4)] {
            fs::write(&p, text).unwrap();
            assert_eq!(load_from(&p).unwrap().volume, want, "{text}");
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_file_is_an_error() {
        let dir = tmp("bad");
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config.toml");
        fs::write(&p, "not = [toml").unwrap();
        assert!(load_from(&p).is_err(), "defaults here would be saved over the user's settings");
        let _ = fs::remove_dir_all(dir);
    }
}
