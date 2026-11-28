use super::*;

#[test]
fn key_event_names_parse() {
    let m = egui::Modifiers { ctrl: true, alt: true, ..Default::default() };
    assert_eq!(key_event_name(egui::Key::W, m), "Ctrl+Alt+W");
    for k in egui::Key::ALL {
        if macro_key_name(*k).is_some() {
            assert!(keys::parse_hotkey(&key_event_name(*k, m)).is_ok(), "{k:?}");
        }
    }
}

#[test]
fn shift_alone_only_with_f_keys() {
    let s = egui::Modifiers { shift: true, ..Default::default() };
    assert!(shift_only_plain_key(egui::Key::A, s));
    assert!(!shift_only_plain_key(egui::Key::F9, s));
}

#[test]
fn every_whip_has_a_thumbnail() {
    for id in ALL {
        assert!(whip_thumb(id).pixels().iter().any(|p| p.alpha() > 200), "{id:?}");
    }
}