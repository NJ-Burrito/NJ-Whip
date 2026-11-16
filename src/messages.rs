//! What a crack types into Claude: a whip's fun line plus a real instruction for the whip's intent.
//! The wording follows published guidance on steering Claude Code; it is not yet measured on live sessions.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Intent {
    Faster,
    Harder,
    Careful,
    Happier,
    Focus,
}

impl Intent {
    pub fn name(self) -> &'static str {
        match self {
            Intent::Faster => "Faster",
            Intent::Harder => "Harder",
            Intent::Careful => "Careful",
            Intent::Happier => "Happier",
            Intent::Focus => "Focus",
        }
    }
}

/// `Plain` and `NjKit` keep their names: config files written by older versions still load.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pack {
    #[default]
    Plain,
    NjKit,
    Surgical,
    Quiet,
    Superpowers,
    PlanFirst,
}

impl Pack {
    pub const ALL: [Pack; 6] = [Pack::Plain, Pack::NjKit, Pack::Surgical, Pack::Quiet, Pack::Superpowers, Pack::PlanFirst];

    pub fn name(self) -> &'static str {
        match self {
            Pack::Plain => "Plain",
            Pack::NjKit => "nj-kit",
            Pack::Surgical => "Surgical",
            Pack::Quiet => "Quiet",
            Pack::Superpowers => "Superpowers",
            Pack::PlanFirst => "Plan-first",
        }
    }

    /// Does the pack word its instructions per session `Profile`?
    pub fn uses_profile(self) -> bool {
        self == Pack::Plain
    }

    /// One line for the picker.
    pub fn note(self) -> &'static str {
        match self {
            Pack::Plain => "works with any Claude Code",
            Pack::NjKit => "needs the nj-kit skills",
            Pack::Surgical => "minimum code, small careful changes",
            Pack::Quiet => "result first, no narration or recap",
            Pack::Superpowers => "needs the superpowers plugin",
            Pack::PlanFirst => "pauses Claude and asks for a plan",
        }
    }
}

/// The kind of session the whip talks to (it cannot see the model or the effort itself).
/// Only the Plain pack has a wording per profile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile {
    #[default]
    Light,
    Heavy,
}

impl Profile {
    pub fn label(self) -> &'static str {
        match self {
            Profile::Light => "Light: Haiku or Sonnet, low or medium effort",
            Profile::Heavy => "Heavy: Opus or Fable, high effort and above",
        }
    }
}

/// Everything in Settings that shapes a crack's message and its delivery.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub pack: Pack,
    pub profile: Profile,
    /// "Always press Esc first": no intent is queued.
    pub always_esc: bool,
    /// "HARDER adds ultrathink".
    pub harder_ultrathink: bool,
}

/// Reset is two messages: the phrase only works as the whole message, and it carries no
/// instruction, so a second message tells Claude to go on.
pub const RESET: &str = "normal mode";
pub const RESUME: &str = "Continue.";

/// The instruction in the form for its default delivery: CAREFUL and FOCUS (sent after Esc)
/// begin with "Continue"; FASTER, HARDER and HAPPIER (queued) hold no resume word.
/// Plan-first stops Claude instead, on every intent.
pub fn instruction(intent: Intent, pack: Pack, profile: Profile) -> &'static str {
    use Intent::*;
    use Profile::*;
    match (pack, intent) {
        (Pack::Plain, Faster) => match profile {
            Light => "Smallest change that works; no extra files, docs or refactors. When it's done and checked, stop and report.",
            Heavy => "You have enough information: act now. Smallest change that works; when it's done and checked, stop and report.",
        },
        (Pack::Plain, Harder) => match profile {
            Light => "Before you say done, run a check that exercises the change and show the output; say what you couldn't run.",
            Heavy => "Be ambitious: handle edge cases, error states and every caller of your change. Then tell me which claims you verified.",
        },
        (Pack::Plain, Careful) => match profile {
            Light => "Continue, carefully: read each file and its callers before you change it, state the change in two lines, then proceed.",
            Heavy => "Continue, but before any command that changes state, check the evidence supports that exact action. Ask before deleting; otherwise proceed.",
        },
        (Pack::Plain, Happier) => "Good progress, no rush. If something fails or can't be done, say so plainly with the output, then carry on.",
        (Pack::Plain, Focus) => match profile {
            Light => "Continue only the task I asked for. When it's done and checked, stop and report; list ideas at the end.",
            Heavy => "Continue only the task I asked for, at that scope: no cleanup or refactors. Report side issues in one line.",
        },

        (Pack::NjKit, Faster) => "Just do it: no exploration, smallest change that meets my request; ask only before irreversible steps.",
        (Pack::NjKit, Harder) => "Follow nj-method steps 4 and 5: run every check, show real output, review each item against my request.",
        (Pack::NjKit, Careful) => "Continue, but read before editing, ask me before anything irreversible, and delete only through nj-safe-delete.",
        (Pack::NjKit, Happier) => "No rush: one short encouraging line, then work; say plainly what failed or was skipped, no workarounds.",
        (Pack::NjKit, Focus) => "Continue on my original request only (nj-method step 1): name done in a line, note side issues, skip them.",

        (Pack::Surgical, Faster) => "Minimum code that solves the problem; nothing speculative, no new abstractions, no extra files.",
        (Pack::Surgical, Harder) => "Define the success criteria in one line, then loop until verified; show the evidence.",
        (Pack::Surgical, Careful) => "Continue, but don't assume: state your assumptions and tradeoffs in two lines; ask if one is risky.",
        (Pack::Surgical, Happier) => "Good progress; stay calm and honest, surface confusion instead of hiding it, no rush.",
        (Pack::Surgical, Focus) => "Continue. Surgical changes only: touch what the request needs, clean up only your own mess.",

        (Pack::Quiet, Faster) => "Lead with the result; no preamble, narration or recap. When it's done and checked, stop and report.",
        (Pack::Quiet, Harder) => "Keep going until the check passes; then report only the command you ran and its real output.",
        (Pack::Quiet, Careful) => "Continue, but keep full detail for errors, failing output and anything destructive; ask before deleting or overwriting.",
        (Pack::Quiet, Happier) => "One short friendly line is welcome, no flattery; if something failed, say that first.",
        (Pack::Quiet, Focus) => "Continue with the original request only; answer in one to three sentences, park side issues in one line.",

        (Pack::Superpowers, Faster) => "Keep executing the plan inline, no subagents: one task at a time, no detours, verify at the end.",
        (Pack::Superpowers, Harder) => "Use systematic-debugging: find the root cause before any fix; no guessing, no symptom patches.",
        (Pack::Superpowers, Careful) => "Continue under verification-before-completion: run the proving command fresh and show its output before claiming anything.",
        (Pack::Superpowers, Happier) => "Nice work, partner; keep evidence before claims, and tell me plainly when something fails.",
        (Pack::Superpowers, Focus) => "Continue the current plan task only; YAGNI: nothing the spec did not ask for.",

        (Pack::PlanFirst, Faster) => "Stop editing. Give me the plan in five bullets or fewer, then wait for my go.",
        (Pack::PlanFirst, Harder) => "Stop editing. Propose two approaches with tradeoffs, recommend one, list the files it touches; wait for my go.",
        (Pack::PlanFirst, Careful) => "Make no changes yet: read the code involved, tell me what could break, then wait for approval.",
        (Pack::PlanFirst, Happier) => "Good so far. Pause and tell me where we are, what is left, and anything you are unsure about.",
        (Pack::PlanFirst, Focus) => "Stop. Restate my request in one sentence and what done means; then continue on that alone.",
    }
}

fn queued_form(intent: Intent, pack: Pack) -> bool {
    pack != Pack::PlanFirst && matches!(intent, Intent::Faster | Intent::Harder | Intent::Happier)
}

/// Delivery. After Esc: the macro runs as the user set it (Claude is stopped first).
/// Queued: the Esc steps are left out, so Claude reads the line at its next tool boundary and
/// nothing in flight is cancelled.
pub fn after_esc(intent: Intent, style: Style) -> bool {
    style.always_esc || !queued_form(intent, style.pack)
}

/// `line` (fun) + space + instruction. A queued-form instruction forced after Esc gets the
/// resume word in front; the ultrathink switch adds the bare word to the end of HARDER only.
pub fn compose(line: &str, intent: Intent, style: Style) -> String {
    let mut s = format!("{} ", line.trim());
    if style.always_esc && queued_form(intent, style.pack) {
        s.push_str("Continue. ");
    }
    s.push_str(instruction(intent, style.pack, style.profile));
    if style.harder_ultrathink && intent == Intent::Harder {
        s.push_str(" ultrathink");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    const INTENTS: [Intent; 5] = [Intent::Faster, Intent::Harder, Intent::Careful, Intent::Happier, Intent::Focus];
    const PROFILES: [Profile; 2] = [Profile::Light, Profile::Heavy];
    /// Kept out of every instruction: hook triggers of other plugins, the reset phrase, words
    /// new models read too literally, and the keyword that is a switch of its own.
    const BANNED: [&str; 10] = [
        "normal mode", "be brief", "be terse", "less tokens", "fewer tokens", "shorter answers",
        "fewer tool calls", "fewest tool calls", "never", "ultrathink",
    ];

    fn every() -> impl Iterator<Item = (Pack, Profile, Intent)> {
        Pack::ALL.into_iter().flat_map(|p| PROFILES.into_iter().flat_map(move |f| INTENTS.into_iter().map(move |i| (p, f, i))))
    }

    #[test]
    fn instructions_are_safe_to_send() {
        for (p, f, i) in every() {
            let s = instruction(i, p, f);
            let at = format!("{p:?} {f:?} {i:?}: {s}");
            assert!(s.split_whitespace().count() <= 20, "more than 20 words: {at}");
            assert!(s.is_ascii(), "not ASCII: {at}");
            assert!(s.starts_with(|c: char| c.is_ascii_alphabetic()), "first character is not a letter: {at}");
            assert!(!s.contains('@'), "holds an @: {at}");
            let low = s.to_ascii_lowercase();
            for b in BANNED {
                assert!(!low.contains(b), "holds \"{b}\": {at}");
            }
        }
    }

    #[test]
    fn resume_word_matches_the_default_delivery() {
        for (p, f, i) in every() {
            let s = instruction(i, p, f);
            match i {
                Intent::Careful | Intent::Focus => {
                    assert!(after_esc(i, Style { pack: p, profile: f, ..Style::default() }), "{p:?} {i:?}");
                    if p != Pack::PlanFirst {
                        assert!(s.starts_with("Continue"), "{p:?} {f:?} {i:?}: {s}");
                    }
                }
                _ => assert!(!s.to_ascii_lowercase().contains("continue"), "{p:?} {f:?} {i:?}: {s}"),
            }
        }
    }

    #[test]
    fn every_pack_differs_from_plain() {
        for p in Pack::ALL.into_iter().filter(|&p| p != Pack::Plain) {
            for f in PROFILES {
                for i in INTENTS {
                    for plain in PROFILES {
                        assert_ne!(instruction(i, p, f), instruction(i, Pack::Plain, plain), "{p:?} {i:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_pack_words_per_profile_only_when_it_says_so() {
        for p in Pack::ALL {
            let varies = INTENTS.into_iter().any(|i| instruction(i, p, Profile::Light) != instruction(i, p, Profile::Heavy));
            assert_eq!(varies, p.uses_profile(), "{p:?}: the Settings window offers the session choice by this");
        }
    }

    #[test]
    fn forced_esc_adds_the_resume_word() {
        for (p, f, i) in every() {
            let forced = Style { pack: p, profile: f, always_esc: true, harder_ultrathink: false };
            assert!(after_esc(i, forced), "{p:?} {i:?}");
            let s = compose("Zap!", i, forced);
            let text = instruction(i, p, f);
            if p == Pack::PlanFirst {
                assert_eq!(s, format!("Zap! {text}"), "Plan-first is sent as it is");
            } else {
                assert!(s.starts_with("Zap! Continue"), "{p:?} {i:?}: {s}");
                assert_eq!(s.matches("Continue").count(), 1, "one resume word: {s}");
            }
        }
    }

    #[test]
    fn ultrathink_switch_touches_harder_only() {
        for (p, f, i) in every() {
            for always_esc in [false, true] {
                let off = Style { pack: p, profile: f, always_esc, harder_ultrathink: false };
                let on = Style { harder_ultrathink: true, ..off };
                assert!(!compose("Zap!", i, off).contains("ultrathink"));
                if i == Intent::Harder {
                    assert_eq!(compose("Zap!", i, on), compose("Zap!", i, off) + " ultrathink");
                } else {
                    assert_eq!(compose("Zap!", i, on), compose("Zap!", i, off));
                }
            }
        }
    }

    #[test]
    fn compose_puts_the_fun_line_first() {
        for (p, f, i) in every() {
            let style = Style { pack: p, profile: f, ..Style::default() };
            assert_eq!(compose(" Zap! ", i, style), format!("Zap! {}", instruction(i, p, f)), "{p:?} {f:?} {i:?}");
        }
    }
}
