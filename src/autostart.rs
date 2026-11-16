//! Start-with-Windows: one value `NJ-Whip` under HKCU\...\CurrentVersion\Run, no admin needed.

use std::io;
use std::path::Path;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::*;

/// The Run value: the exe path in quotes (it may hold spaces).
fn command(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

fn err(e: windows::core::Error) -> io::Error {
    io::Error::other(e.message())
}

/// Add (`true`) or remove (`false`) the Run value for the running exe.
pub fn set(enabled: bool) -> io::Result<()> {
    let key_path = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
    let name = w!("NJ-Whip");
    // Resolved before the key is opened, so an error here cannot leak the handle.
    let cmd = if enabled { Some(command(&std::env::current_exe()?)) } else { None };
    unsafe {
        let mut key = HKEY::default();
        RegCreateKeyExW(HKEY_CURRENT_USER, key_path, 0, PCWSTR::null(), REG_OPTION_NON_VOLATILE, KEY_SET_VALUE, None, &mut key, None)
            .ok()
            .map_err(err)?;
        let res = if let Some(cmd) = cmd {
            let bytes: Vec<u8> = cmd.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
            RegSetValueExW(key, name, 0, REG_SZ, Some(&bytes)).ok().map_err(err)
        } else {
            match RegDeleteValueW(key, name) {
                e if e == ERROR_FILE_NOT_FOUND => Ok(()),
                e => e.ok().map_err(err),
            }
        };
        let _ = RegCloseKey(key);
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_is_quoted_exe_only() {
        let c = command(Path::new(r"C:\Program Files\NJ-Whip\nj-whip.exe"));
        assert_eq!(c, r#""C:\Program Files\NJ-Whip\nj-whip.exe""#);
    }
}
