use crate::config::MacroStep;
use crate::keys::key_vk;
use std::time::{Duration, Instant};
use windows::Win32::UI::Input::KeyboardAndMouse::*;

#[derive(Debug, PartialEq)]
pub enum KeyAction {
    Vk { vk: u16, up: bool },
    Unicode { ch: u16, up: bool },
    Pause(u64),
}

/// Expand macro steps into key actions. A `Key` with an unknown name is skipped.
pub fn build_actions(steps: &[MacroStep], phrase: &str) -> Vec<KeyAction> {
    let mut out = Vec::new();
    for step in steps {
        match step {
            MacroStep::Key(name) => {
                let Some(vk) = key_vk(name) else { continue };
                out.push(KeyAction::Vk { vk, up: false });
                out.push(KeyAction::Vk { vk, up: true });
            }
            MacroStep::Phrase => {
                for ch in phrase.encode_utf16() {
                    out.push(KeyAction::Unicode { ch, up: false });
                    out.push(KeyAction::Unicode { ch, up: true });
                }
            }
        }
        out.push(KeyAction::Pause(30));
    }
    out
}

/// The macro for a queued message: the Esc steps before the message are left out, nothing else
/// changes. Claude then reads the line at its next tool boundary and nothing in flight is cancelled.
pub fn queued(steps: &[MacroStep]) -> Vec<MacroStep> {
    let phrase = steps.iter().position(|s| *s == MacroStep::Phrase).unwrap_or(0);
    let is_esc = |s: &MacroStep| matches!(s, MacroStep::Key(k) if key_vk(k) == Some(VK_ESCAPE.0));
    steps.iter().enumerate().filter(|(i, s)| *i >= phrase || !is_esc(s)).map(|(_, s)| s.clone()).collect()
}

/// Pause between Reset's two messages, so the first is taken before the second is typed.
const RESET_GAP_MS: u64 = 600;

/// Reset: `first` through the macro as it is (after Esc, as its own whole message), a short
/// pause, then `second` queued. No second Esc: it would cancel the answer to the first.
pub fn reset_actions(steps: &[MacroStep], first: &str, second: &str) -> Vec<KeyAction> {
    let mut out = build_actions(steps, first);
    out.push(KeyAction::Pause(RESET_GAP_MS));
    out.extend(build_actions(&queued(steps), second));
    out
}

/// Poll every 10 ms until Ctrl/Alt/Shift/Win are all up; false on timeout.
pub fn wait_modifiers_released(is_down: impl Fn(u16) -> bool, timeout: Duration) -> bool {
    const MODS: [VIRTUAL_KEY; 5] = [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN];
    let start = Instant::now();
    loop {
        if !MODS.iter().any(|vk| is_down(vk.0)) {
            return true;
        }
        if start.elapsed() >= timeout {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn wait_modifiers_released_real(timeout: Duration) -> bool {
    wait_modifiers_released(
        |vk| unsafe { GetAsyncKeyState(vk as i32) } as u16 & 0x8000 != 0,
        timeout,
    )
}

/// Inject the actions into the focused window. Not unit-tested: it types for real.
/// Returns how many key events Windows refused (e.g. UIPI blocks an elevated window).
pub fn send(actions: &[KeyAction]) -> usize {
    let mut failed = 0;
    for a in actions {
        let (vk, scan, flags) = match *a {
            KeyAction::Pause(ms) => {
                std::thread::sleep(Duration::from_millis(ms));
                continue;
            }
            KeyAction::Vk { vk, up } => (vk, 0, if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }),
            KeyAction::Unicode { ch, up } => (
                0,
                ch,
                KEYEVENTF_UNICODE | if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
            ),
        };
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT { wVk: VIRTUAL_KEY(vk), wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 },
            },
        };
        if unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) } == 0 {
            failed += 1;
        }
    }
    failed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn vk(v: u16) -> [KeyAction; 2] {
        [KeyAction::Vk { vk: v, up: false }, KeyAction::Vk { vk: v, up: true }]
    }
    fn uni(c: char) -> [KeyAction; 2] {
        let ch = c as u16;
        [KeyAction::Unicode { ch, up: false }, KeyAction::Unicode { ch, up: true }]
    }
    fn default_steps() -> Vec<MacroStep> {
        vec![MacroStep::Key("Esc".into()), MacroStep::Phrase, MacroStep::Key("Enter".into())]
    }

    #[test]
    fn default_macro_actions() {
        let got = build_actions(&default_steps(), "GO");
        let mut want = Vec::new();
        want.extend(vk(0x1B));
        want.push(KeyAction::Pause(30));
        want.extend(uni('G'));
        want.extend(uni('O'));
        want.push(KeyAction::Pause(30));
        want.extend(vk(0x0D));
        want.push(KeyAction::Pause(30));
        assert_eq!(got, want);
    }

    #[test]
    fn queued_drops_only_the_esc_steps_before_the_phrase() {
        let key = |k: &str| MacroStep::Key(k.into());
        assert_eq!(queued(&default_steps()), vec![MacroStep::Phrase, key("Enter")]);
        // Other keys before the message stay, every spelling of Esc goes, an Esc after it stays.
        let steps = vec![key("Esc"), key("Tab"), key("escape"), MacroStep::Phrase, key("Esc"), key("Enter")];
        assert_eq!(queued(&steps), vec![key("Tab"), MacroStep::Phrase, key("Esc"), key("Enter")]);
        // A macro with no message step is left as it is.
        assert_eq!(queued(&[key("Esc"), key("Enter")]), vec![key("Esc"), key("Enter")]);
        let got = build_actions(&queued(&default_steps()), "GO");
        assert!(!got.contains(&KeyAction::Vk { vk: 0x1B, up: false }));
    }

    #[test]
    fn reset_is_two_messages() {
        let got = reset_actions(&default_steps(), "A", "B");
        let mut want = build_actions(&default_steps(), "A");
        want.push(KeyAction::Pause(RESET_GAP_MS));
        want.extend(uni('B'));
        want.push(KeyAction::Pause(30));
        want.extend(vk(0x0D));
        want.push(KeyAction::Pause(30));
        assert_eq!(got, want);
        let enters = got.iter().filter(|a| **a == KeyAction::Vk { vk: 0x0D, up: false }).count();
        let escs = got.iter().filter(|a| **a == KeyAction::Vk { vk: 0x1B, up: false }).count();
        assert_eq!((enters, escs), (2, 1));
    }

    #[test]
    fn phrase_never_uses_vk() {
        for phrase in ["FASTER", "أسرع", "😀"] {
            let got = build_actions(&[MacroStep::Phrase], phrase);
            let n = phrase.encode_utf16().count();
            assert_eq!(got.len(), n * 2 + 1);
            assert!(got[..n * 2].iter().all(|a| matches!(a, KeyAction::Unicode { .. })));
            assert_eq!(got[n * 2], KeyAction::Pause(30));
        }
    }

    #[test]
    fn wait_returns_when_released() {
        let polls = Cell::new(0);
        let ok = wait_modifiers_released(
            |_| {
                polls.set(polls.get() + 1);
                polls.get() <= 3
            },
            Duration::from_secs(2),
        );
        assert!(ok);
    }

    #[test]
    fn wait_times_out() {
        assert!(!wait_modifiers_released(|_| true, Duration::from_millis(50)));
    }
}
