//! Updates, only when asked. `check` reads the latest release's version from GitHub; `install`
//! downloads that release's exe and puts it in place of the running one; `restart` starts it.
//! Nothing here runs unless the user presses the button in the About tab.

use std::path::Path;
use std::{fs, io::Read};
use windows::core::HSTRING;
use windows::Win32::Networking::WinInet::DeleteUrlCacheEntryW;
use windows::Win32::System::Com::Urlmon::URLDownloadToFileW;

const LATEST: &str = "https://api.github.com/repos/NJ-Burrito/NJ-Whip/releases/latest";
const RELEASES: &str = "https://github.com/NJ-Burrito/NJ-Whip/releases/download";
/// A download smaller than this is an error page, never the program.
const MIN_EXE: u64 = 1_000_000;
/// Given to the new copy: the old one is still closing, so wait for it instead of handing over.
pub const AFTER_UPDATE: &str = "--updated";

pub type Version = [u32; 3];

/// "v0.4.0" or "0.4.0" is [0, 4, 0]; anything else is not a version.
pub fn version(tag: &str) -> Option<Version> {
    let mut parts = tag.strip_prefix('v').unwrap_or(tag).split('.');
    let mut next = || {
        let p = parts.next()?;
        (!p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())).then(|| p.parse().ok())?
    };
    let v = [next()?, next()?, next()?];
    parts.next().is_none().then_some(v)
}

pub fn current() -> Version {
    version(env!("CARGO_PKG_VERSION")).expect("the package version")
}

pub fn dotted(v: Version) -> String {
    format!("{}.{}.{}", v[0], v[1], v[2])
}

/// The `tag_name` of GitHub's "latest release" answer.
fn tag_of(json: &str) -> Option<&str> {
    let rest = &json[json.find("\"tag_name\"")? + "\"tag_name\"".len()..];
    rest.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?.split('"').next()
}

/// Windows' own downloader (system proxy and certificates, no extra code in the exe).
fn download(url: &str, to: &Path) -> Result<(), String> {
    let (url, to) = (HSTRING::from(url), HSTRING::from(to.as_os_str()));
    unsafe {
        let _ = DeleteUrlCacheEntryW(&url); // a cached answer would hide a new release
        URLDownloadToFileW(None, &url, &to, 0, None).map_err(|e| e.message())
    }
}

/// The latest released version when it is newer than this one.
pub fn check() -> Result<Option<Version>, String> {
    let tmp = std::env::temp_dir().join(format!("nj-whip-latest-{}.json", std::process::id()));
    download(LATEST, &tmp)?;
    let json = fs::read_to_string(&tmp).map_err(|e| e.to_string());
    let _ = fs::remove_file(&tmp);
    let latest = tag_of(&json?).and_then(version).ok_or("GitHub's answer had no version in it")?;
    Ok(newer(latest, current()))
}

/// `latest` only when it is strictly newer than `now` (the same version is never installed again).
fn newer(latest: Version, now: Version) -> Option<Version> {
    (latest > now).then_some(latest)
}

/// Puts the downloaded `new` file in place of `exe` (the running program is moved aside to
/// `.old`, which Windows allows; the next start removes it). Refuses a file that is not a program.
fn put_in_place(new: &Path, exe: &Path) -> Result<(), String> {
    let mut head = [0u8; 2];
    let size = fs::metadata(new).map_err(|e| e.to_string())?.len();
    fs::File::open(new).and_then(|mut f| f.read_exact(&mut head)).map_err(|e| e.to_string())?;
    if &head != b"MZ" || size < MIN_EXE {
        return Err("the download is not the program".into());
    }
    let old = exe.with_extension("old");
    let _ = fs::remove_file(&old);
    fs::rename(exe, &old).map_err(|e| format!("couldn't move the running program aside: {e}"))?;
    fs::rename(new, exe).map_err(|e| {
        let _ = fs::rename(&old, exe);
        format!("couldn't put the new program in place: {e}")
    })
}

/// Downloads version `v` and swaps it in for `exe`. The program on disk is the new one after
/// this; the running one keeps going until `restart`.
pub fn install(v: Version, exe: &Path) -> Result<(), String> {
    let new = exe.with_extension("new");
    let done = download(&format!("{RELEASES}/v{}/NJ-Whip.exe", dotted(v)), &new).and_then(|()| put_in_place(&new, exe));
    if done.is_err() {
        let _ = fs::remove_file(&new);
    }
    done
}

pub fn restart(exe: &Path) -> Result<(), String> {
    std::process::Command::new(exe).arg(AFTER_UPDATE).spawn().map(drop).map_err(|e| e.to_string())
}

/// Removes the copy an update moved aside (it could not be deleted while it was running).
pub fn tidy() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = fs::remove_file(exe.with_extension("old"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_read_and_compare() {
        assert_eq!(version("v0.4.0"), Some([0, 4, 0]));
        assert_eq!(version("1.12.3"), Some([1, 12, 3]));
        for bad in ["", "v1.2", "v1.2.3.4", "v1.2.x", "v+1.2.3", "v1..3", "latest", "v1.2.3/../x"] {
            assert_eq!(version(bad), None, "{bad}");
        }
        assert!(version("v0.10.0") > version("v0.9.9"));
        assert_eq!(version(&dotted(current())), Some(current()));
    }

    #[test]
    fn the_tag_is_read_from_githubs_answer() {
        assert_eq!(tag_of(r#"{"url":"x","tag_name": "v0.4.0","name":"NJ-Whip 0.4.0"}"#), Some("v0.4.0"));
        assert_eq!(tag_of(r#"{"message":"Not Found"}"#), None);
    }

    #[test]
    fn only_a_program_is_put_in_place() {
        let dir = std::env::temp_dir().join(format!("njw-update-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let (exe, new) = (dir.join("NJ-Whip.exe"), dir.join("NJ-Whip.new"));
        fs::write(&exe, b"old program").unwrap();
        fs::write(&new, b"<html>Not Found</html>").unwrap();
        assert!(put_in_place(&new, &exe).is_err());
        assert_eq!(fs::read(&exe).unwrap(), b"old program", "a refused download changes nothing");
        let mut program = b"MZ".to_vec();
        program.resize(MIN_EXE as usize, 7);
        fs::write(&new, &program).unwrap();
        put_in_place(&new, &exe).unwrap();
        assert_eq!(fs::read(&exe).unwrap(), program);
        assert_eq!(fs::read(dir.join("NJ-Whip.old")).unwrap(), b"old program");
        assert!(!new.exists());
        for f in ["NJ-Whip.exe", "NJ-Whip.old"] {
            fs::remove_file(dir.join(f)).unwrap();
        }
        fs::remove_dir(&dir).unwrap();
    }

    /// Asks GitHub for real: `cargo test --release live_check -- --ignored`.
    #[test]
    #[ignore]
    fn live_check() {
        println!("newer than {}: {:?}", dotted(current()), check().unwrap());
    }

    // mutation testing (cargo-mutants, 2026-11-24)
    #[test]
    fn only_a_strictly_newer_release_is_offered() {
        assert_eq!(newer([0, 5, 1], [0, 5, 0]), Some([0, 5, 1]));
        assert_eq!(newer([0, 5, 0], [0, 5, 0]), None, "the same version is not installed again");
        assert_eq!(newer([0, 4, 9], [0, 5, 0]), None, "an older release is never offered");
        assert_eq!(newer([1, 0, 0], [0, 99, 99]), Some([1, 0, 0]));
    }

    #[test]
    fn the_running_version_is_the_package_version() {
        assert_eq!(Some(current()), version(env!("CARGO_PKG_VERSION")));
        assert_ne!(current(), [0, 0, 0]);
    }

    #[test]
    fn a_program_one_byte_under_the_minimum_is_refused() {
        let dir = std::env::temp_dir().join(format!("njw-update-min-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let (exe, new) = (dir.join("NJ-Whip.exe"), dir.join("NJ-Whip.new"));
        fs::write(&exe, b"old program").unwrap();
        let mut small = b"MZ".to_vec();
        small.resize(MIN_EXE as usize - 1, 7);
        fs::write(&new, &small).unwrap();
        assert!(put_in_place(&new, &exe).is_err(), "MZ but too small: a cut-off download");
        assert_eq!(fs::read(&exe).unwrap(), b"old program");
        let _ = fs::remove_dir_all(&dir);
    }
}
