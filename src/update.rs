//! Updates, only when asked. `check` reads the latest release's version from GitHub; `install`
//! reads that release's published SHA-256, downloads its exe, and puts it in place of the running
//! one only when the bytes match; `restart` starts it.
//! Nothing here runs unless the user presses the button in the About tab.

use std::path::Path;
use std::{fs, io::Read};
use windows::core::HSTRING;
use windows::Win32::Networking::WinInet::DeleteUrlCacheEntryW;
use windows::Win32::Security::Cryptography::{BCryptHash, BCRYPT_SHA256_ALG_HANDLE};
use windows::Win32::System::Com::Urlmon::URLDownloadToFileW;

const LATEST: &str = "https://api.github.com/repos/NJ-Burrito/NJ-Whip/releases/latest";
const BY_TAG: &str = "https://api.github.com/repos/NJ-Burrito/NJ-Whip/releases/tags";
const ASSET: &str = "NJ-Whip.exe";
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

/// The first string value of `key` in a JSON text.
fn field<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let k = format!("\"{key}\"");
    let rest = &json[json.find(&k)? + k.len()..];
    rest.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?.split('"').next()
}

/// The `tag_name` of GitHub's release answer.
fn tag_of(json: &str) -> Option<&str> {
    field(json, "tag_name")
}

/// The SHA-256 GitHub published for the release's `NJ-Whip.exe` (its `"digest": "sha256:<hex>"`).
/// Read only inside that asset's own object, so another asset's digest is never taken for it.
fn digest_of(json: &str) -> Option<[u8; 32]> {
    let at = json.find(&format!("\"name\": \"{ASSET}\"")).or_else(|| json.find(&format!("\"name\":\"{ASSET}\"")))?;
    let asset = &json[at..];
    let asset = &asset[..asset.find("\"browser_download_url\"").unwrap_or(asset.len())];
    let hex = field(asset, "digest")?.strip_prefix("sha256:")?;
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

/// SHA-256 with Windows' own CNG (no hashing code shipped in the exe).
fn sha256(data: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    unsafe { BCryptHash(BCRYPT_SHA256_ALG_HANDLE, None, data, &mut out) }.ok().expect("SHA-256");
    out
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
/// `.old`, which Windows allows; the next start removes it). Refuses a file that is not a program,
/// and a program whose SHA-256 is not the one the release published.
fn put_in_place(new: &Path, exe: &Path, sha: &[u8; 32]) -> Result<(), String> {
    let mut bytes = Vec::new();
    fs::File::open(new).and_then(|mut f| f.read_to_end(&mut bytes)).map_err(|e| e.to_string())?;
    if !bytes.starts_with(b"MZ") || (bytes.len() as u64) < MIN_EXE {
        return Err("the download is not the program".into());
    }
    if &sha256(&bytes) != sha {
        return Err("the download does not match the release's SHA-256".into());
    }
    let old = exe.with_extension("old");
    let _ = fs::remove_file(&old);
    fs::rename(exe, &old).map_err(|e| format!("couldn't move the running program aside: {e}"))?;
    fs::rename(new, exe).map_err(|e| {
        let _ = fs::rename(&old, exe);
        format!("couldn't put the new program in place: {e}")
    })
}

/// The SHA-256 published for release `v` (refused when GitHub's answer is for another tag or has none).
fn published_sha(v: Version) -> Result<[u8; 32], String> {
    let tag = format!("v{}", dotted(v));
    let tmp = std::env::temp_dir().join(format!("nj-whip-release-{}.json", std::process::id()));
    download(&format!("{BY_TAG}/{tag}"), &tmp)?;
    let json = fs::read_to_string(&tmp).map_err(|e| e.to_string());
    let _ = fs::remove_file(&tmp);
    sha_for(&json?, &tag)
}

fn sha_for(json: &str, tag: &str) -> Result<[u8; 32], String> {
    if tag_of(json) != Some(tag) {
        return Err(format!("GitHub's answer is not release {tag}"));
    }
    digest_of(json).ok_or_else(|| format!("release {tag} has no SHA-256 for {ASSET}; not installing it"))
}

/// Downloads version `v`, checks it against the SHA-256 its release published, and swaps it in for
/// `exe`. The program on disk is the new one after this; the running one keeps going until `restart`.
pub fn install(v: Version, exe: &Path) -> Result<(), String> {
    let sha = published_sha(v)?;
    let new = exe.with_extension("new");
    let done = download(&format!("{RELEASES}/v{}/{ASSET}", dotted(v)), &new).and_then(|()| put_in_place(&new, exe, &sha));
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
        assert!(put_in_place(&new, &exe, &sha256(b"<html>Not Found</html>")).is_err());
        assert_eq!(fs::read(&exe).unwrap(), b"old program", "a refused download changes nothing");
        let mut program = b"MZ".to_vec();
        program.resize(MIN_EXE as usize, 7);
        fs::write(&new, &program).unwrap();
        let mut wrong = sha256(&program);
        wrong[31] ^= 1;
        assert_eq!(put_in_place(&new, &exe, &wrong), Err("the download does not match the release's SHA-256".into()));
        assert_eq!(fs::read(&exe).unwrap(), b"old program", "a mismatched download changes nothing");
        put_in_place(&new, &exe, &sha256(&program)).unwrap();
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

    /// The published SHA-256 of the running release, from GitHub: `cargo test --release live_published_sha -- --ignored`.
    #[test]
    #[ignore]
    fn live_published_sha() {
        assert_eq!(published_sha([0, 5, 1]).unwrap()[..2], [0xb6, 0xde]);
    }

    /// The whole install of this version from GitHub into a temp folder (after its release is out):
    /// `cargo test --release live_install -- --ignored`.
    #[test]
    #[ignore]
    fn live_install() {
        let dir = std::env::temp_dir().join(format!("njw-live-install-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("NJ-Whip.exe");
        fs::write(&exe, b"old program").unwrap();
        install(current(), &exe).unwrap();
        assert_eq!(sha256(&fs::read(&exe).unwrap()), published_sha(current()).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sha256_is_the_standard_one() {
        let hex = |d: [u8; 32]| d.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(hex(sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(hex(sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[test]
    fn the_published_digest_is_read_from_the_real_release_answer() {
        let json = include_str!("testdata/release-v0.5.1.json");
        let d = sha_for(json, "v0.5.1").unwrap();
        assert_eq!(d[..4], [0xb6, 0xde, 0x28, 0xf1]);
        assert_eq!(d[28..], [0x6c, 0x14, 0x63, 0x9d]);
        assert!(sha_for(json, "v0.5.2").is_err(), "an answer for another tag is refused");
    }

    #[test]
    fn no_or_a_bad_digest_means_no_install() {
        let rel = |asset: &str| format!(r#"{{"tag_name": "v9.9.9", "assets": [{asset}]}}"#);
        let ok = "a".repeat(64);
        assert!(sha_for(&rel(&format!(r#"{{"name": "NJ-Whip.exe", "digest": "sha256:{ok}", "browser_download_url": "x"}}"#)), "v9.9.9").is_ok());
        assert!(sha_for(&rel(r#"{"name":"NJ-Whip.exe","digest":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}"#), "v9.9.9").is_ok());
        for bad in [
            r#"{"name": "NJ-Whip.exe", "browser_download_url": "x"}"#.to_string(),
            format!(r#"{{"name": "NJ-Whip.exe", "digest": "md5:{ok}"}}"#),
            format!(r#"{{"name": "NJ-Whip.exe", "digest": "sha256:{}"}}"#, &ok[1..]),
            format!(r#"{{"name": "NJ-Whip.exe", "digest": "sha256:{}zz"}}"#, &ok[2..]),
            format!(r#"{{"name": "other.zip", "digest": "sha256:{ok}"}}"#),
            format!(r#"{{"name": "NJ-Whip.exe", "browser_download_url": "x"}}, {{"name": "other.zip", "digest": "sha256:{ok}"}}"#),
        ] {
            assert!(sha_for(&rel(&bad), "v9.9.9").is_err(), "{bad}");
        }
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
        assert!(put_in_place(&new, &exe, &sha256(&small)).is_err(), "MZ but too small: a cut-off download");
        assert_eq!(fs::read(&exe).unwrap(), b"old program");
        let _ = fs::remove_dir_all(&dir);
    }
}
