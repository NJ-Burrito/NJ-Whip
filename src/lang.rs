//! The window's languages. English is the source text; an Arabic line is looked up by its
//! English line. Only the words change: the window's layout stays as it is.
//!
//! egui joins Arabic letters (it shapes text with the font's own rules) but orders nothing: every
//! piece of text it is handed is laid out in ONE direction, guessed from the piece's first letter.
//! So an Arabic row is handed over as pieces (`PIECE` between them) in left-to-right visual order:
//! each Arabic word is a piece of its own, in reading order (egui joins it and lays it right to
//! left), and what stands between the words (spaces, marks, numbers, Latin words) reads left to
//! right as written. The window gives each piece to egui as its own section (`ui.rs`, `job`).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use unicode_bidi::{BidiInfo, Level};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lang {
    #[default]
    En,
    Ar,
}

/// English line, Arabic line. `{}` marks where a value goes (see `Words::fill`).
const AR: &[(&str, &str)] = &[
    // header and footer
    ("On · {} shows the whip", "يعمل · {} يظهر السوط"),
    ("Off", "متوقف"),
    ("NJ-Whip is ON: hotkeys and cracks work", "NJ-Whip يعمل: الاختصارات والفرقعات تعمل"),
    ("NJ-Whip is OFF: nothing on screen, hotkeys released", "NJ-Whip متوقف: لا شيء على الشاشة والاختصارات محررة"),
    ("Quit NJ-Whip", "أغلق NJ-Whip"),
    ("Arabic", "عربي"),
    ("Open NJ-Whip", "افتح NJ-Whip"),
    ("Quit", "إغلاق"),
    // tabs
    ("Whip", "السوط"),
    ("Message", "الرسالة"),
    ("Hotkeys", "الاختصارات"),
    ("Options", "الخيارات"),
    ("About", "حول"),
    ("Problems", "مشكلات"),
    ("Clear", "امسح"),
    // whip
    ("Test crack", "جرب الفرقعة"),
    ("Plays the crack and its effect; types nothing", "يشغل الفرقعة وتأثيرها ولا يكتب شيئا"),
    ("Change tip picture…", "غير صورة الطرف…"),
    ("Use my picture as the tip…", "استخدم صورتي طرفا للسوط…"),
    ("Remove picture", "أزل الصورة"),
    ("Pick a whip to hear its sound.", "اختر سوطا لتسمع صوته."),
    ("Faster", "أسرع"),
    ("Harder", "أشد"),
    ("Careful", "بحذر"),
    ("Happier", "أسعد"),
    ("Focus", "تركيز"),
    ("Navy plaited leather, ice-blue braid, silver fittings.", "جلد كحلي مجدول، ضفيرة زرقاء جليدية وحليات فضية."),
    ("Glowing tube, clean light bead. Floods the screen with colour.", "أنبوب متوهج بخرزة ضوء صافية. يغمر الشاشة باللون."),
    ("Heavy grey hide, tufted tail tip. Thumps the screen.", "جلد رمادي ثقيل بطرف كذيل ذي خصلة. يهز الشاشة بضربته."),
    ("Tan rawhide braid, brass knob. Dust and a lasso swirl.", "ضفيرة جلد خام بلون القمح ومقبض نحاسي. غبار ودوامة حبل."),
    ("Pink-and-gold ribbon, pom-pom tip. Confetti and cheers.", "شريط وردي وذهبي بطرف من الشراشيب. قصاصات ملونة وهتاف."),
    ("Glossy salsa with a hanging drop. Fills Claude up.", "صلصة لامعة بقطرة متدلية. تشبع Claude."),
    ("Olive webbing, brass buckle. A stern red “!”.", "حزام زيتوني بإبزيم نحاسي. علامة «!» حمراء صارمة."),
    ("Silk thread on bamboo. An ink ripple and a falling petal.", "خيط حرير على خيزران. تموج حبر وبتلة تسقط."),
    ("Black cord, steel kunai tip. A clean slash and smoke.", "حبل أسود بطرف كوناي فولاذي. شق نظيف ودخان."),
    ("Tarred hemp, cutlass hilt. Sea spray and a cannon puff.", "قنب مقطرن بمقبض سيف بحارة. رذاذ بحر ونفخة مدفع."),
    ("Kitchen-towel lash, frying-pan tip. Sizzle and steam.", "سوط من منشفة مطبخ بطرف مقلاة. أزيز وبخار."),
    ("A ribbon of starlight, crystal bead. A magic circle.", "شريط من ضوء النجوم بخرزة بلورية. دائرة سحرية."),
    ("A living vine with leaves. The bud blooms on a crack.", "كرمة حية بأوراقها. يتفتح برعمها عند الفرقعة."),
    ("A clay penguin rides the tip. Honk honk, snow puff.", "بطريق من صلصال يركب الطرف. زمور ونفخة ثلج."),
    ("Thundercloud tip. Lightning across the screen.", "طرف من سحابة رعدية. برق عبر الشاشة."),
    // message
    ("Instruction pack", "حزمة التعليمات"),
    ("works with any Claude Code", "تعمل مع أي Claude Code"),
    ("needs the nj-kit skills", "تحتاج مهارات nj-kit"),
    ("minimum code, small careful changes", "أقل شيفرة، تغييرات صغيرة حذرة"),
    ("result first, no narration or recap", "النتيجة أولا، دون سرد أو تلخيص"),
    ("needs the superpowers plugin", "تحتاج إضافة superpowers"),
    ("pauses Claude and asks for a plan", "توقف Claude وتطلب خطة"),
    ("Session", "الجلسة"),
    ("Light: Haiku or Sonnet, low or medium effort", "خفيفة: Haiku أو Sonnet بجهد منخفض أو متوسط"),
    ("Heavy: Opus or Fable, high effort and above", "ثقيلة: Opus أو Fable بجهد عال فأعلى"),
    ("This pack words its messages the same for every session", "هذه الحزمة تصوغ رسائلها بالصيغة نفسها لكل جلسة"),
    ("Sent after Esc", "ترسل بعد Esc"),
    ("Sent queued, no Esc", "ترسل في الطابور دون Esc"),
    ("{} lines", "عبارات {}"),
    ("Remove", "أزل"),
    ("Add your own line", "أضف عبارتك"),
    ("Add", "أضف"),
    ("Back to the whip's own lines", "عد إلى عبارات السوط الأصلية"),
    ("Reset Claude", "أعد ضبط Claude"),
    ("Types “{}”, then “{}”, into the window you were in (ends sticky modes)", "يكتب «{}» ثم «{}» في النافذة التي كنت فيها (ينهي الأوضاع الثابتة)"),
    ("also switches off always-on modes such as ponytail", "يوقف أيضا الأوضاع الدائمة مثل ponytail"),
    // hotkeys
    ("Crack", "فرقعة"),
    ("Show / hide the whip", "إظهار السوط وإخفاؤه"),
    ("Cancel", "إلغاء"),
    ("Record", "سجل"),
    ("Press keys…", "اضغط المفاتيح…"),
    ("Hold Ctrl, Alt or Shift together with the key.", "اضغط Ctrl أو Alt أو Shift مع المفتاح."),
    ("Shift alone isn't enough: add Ctrl or Alt (Shift works alone only with F1–F24).", "Shift وحده لا يكفي: أضف Ctrl أو Alt (يعمل Shift وحده مع F1–F24 فقط)."),
    ("Keys a crack sends", "المفاتيح التي ترسلها الفرقعة"),
    ("Record keys", "سجل المفاتيح"),
    ("Done", "تم"),
    ("Add {message}", "أضف {message}"),
    ("Crack hotkey: {}", "اختصار الفرقعة: {}"),
    ("Show/hide hotkey: {}", "اختصار الإظهار والإخفاء: {}"),
    ("Crack and show/hide need different hotkeys.", "اختصارا الفرقعة والإظهار يجب أن يختلفا."),
    ("The keys a crack sends need at least one step.", "مفاتيح الفرقعة تحتاج خطوة واحدة على الأقل."),
    // options
    ("Crack volume", "مستوى صوت الفرقعة"),
    ("Whip sound volume (trumpet, yeehaw, gong…)", "مستوى صوت السوط الخاص (بوق، صيحة، جرس…)"),
    ("Heat meter and full screen effect (fills in {} cracks)", "مقياس الحرارة وتأثير الشاشة الكامل (يمتلئ بعد {} فرقعات)"),
    ("Crack when I snap the whip back (wind up one way, snap the other)", "فرقع عند رد السوط بسرعة (اسحب في اتجاه ثم رد في الآخر)"),
    ("Always press Esc first", "اضغط Esc أولا دائما"),
    (
        "Off: a whip that only nudges Claude is queued without Esc, so nothing Claude is doing is cancelled (the sample under Message says how the current whip is sent). Turn on if queued messages get lost.",
        "عند الإيقاف: السوط الذي ينبه Claude فقط يرسل في الطابور دون Esc، فلا يلغى ما يعمل عليه Claude (المثال في تبويب الرسالة يبين كيف يرسل السوط الحالي). شغله إذا ضاعت رسائل الطابور.",
    ),
    ("HARDER adds ultrathink (one turn at extra-high effort; costs more)", "«أشد» تضيف ultrathink (دورة واحدة بجهد أعلى وتكلفة أكثر)"),
    ("Start with Windows", "ابدأ مع Windows"),
    ("Theme", "المظهر"),
    ("System", "النظام"),
    ("Couldn't save settings: {}", "تعذر حفظ الإعدادات: {}"),
    ("Couldn't change start-with-Windows: {}", "تعذر تغيير البدء مع Windows: {}"),
    // about
    ("An on-screen whip for Claude Code. Swing it, it cracks, and Claude gets one short instruction.", "سوط على الشاشة لـ Claude Code. لوح به فيفرقع، وتصل Claude تعليمة قصيرة واحدة."),
    ("Version {}", "الإصدار {}"),
    ("Made by Nader Alzghoul (NJ-Burrito).", "من صنع نادر الزغول (NJ-Burrito)."),
    ("Free and open source under the MIT licence.", "مجاني ومفتوح المصدر برخصة MIT."),
    ("Website", "الموقع"),
    ("Source code", "الشيفرة المصدرية"),
    ("More tools", "أدوات أخرى"),
    ("Sound credits", "حقوق الأصوات"),
    ("Updates", "التحديثات"),
    ("Check for updates", "ابحث عن تحديث"),
    ("Checking…", "جار البحث…"),
    ("You have the latest version.", "لديك أحدث إصدار."),
    ("Version {} is available.", "الإصدار {} متاح."),
    ("Install and restart", "ثبت وأعد التشغيل"),
    ("Downloading…", "جار التنزيل…"),
    ("Restarting…", "جار إعادة التشغيل…"),
    ("NJ-Whip goes online only when you press this button.", "لا يتصل NJ-Whip بالإنترنت إلا عند ضغطك هذا الزر."),
    ("Couldn't check: {}", "تعذر البحث: {}"),
    ("Couldn't install: {}", "تعذر التثبيت: {}"),
    ("An independent project, not affiliated with Anthropic. “Claude” and “Claude Code” are trademarks of Anthropic.", "مشروع مستقل غير تابع لشركة Anthropic، وهي مالكة العلامتين التجاريتين «Claude» و«Claude Code»."),
];

/// Stands between the pieces of a row that is ready to draw.
pub const PIECE: char = '\u{1F}';

fn is_arabic(c: char) -> bool {
    ('\u{600}'..='\u{6FF}').contains(&c)
}

fn mirrored(c: char) -> char {
    match c {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '«' => '»',
        '»' => '«',
        c => c,
    }
}

/// One row in reading order as its pieces in left-to-right visual order (the row reads right to left).
fn ready(row: &str) -> String {
    let info = BidiInfo::new(row, Some(Level::rtl()));
    let Some(para) = info.paragraphs.first() else { return String::new() };
    let (levels, runs) = info.visual_runs(para, para.range.clone());
    let mut out = String::with_capacity(row.len() + 16);
    // an Arabic word comes by last letter first: it is collected and put back in reading order
    let mut word: Vec<char> = Vec::new();
    let flush = |word: &mut Vec<char>, out: &mut String| {
        if !word.is_empty() {
            out.push(PIECE);
            out.extend(word.drain(..).rev());
            out.push(PIECE);
        }
    };
    for run in runs {
        let rtl = levels[run.start].is_rtl();
        let text = &row[run];
        let chars: Box<dyn Iterator<Item = char>> = if rtl { Box::new(text.chars().rev().map(mirrored)) } else { Box::new(text.chars()) };
        for c in chars {
            if is_arabic(c) {
                word.push(c);
            } else {
                flush(&mut word, &mut out);
                out.push(c);
            }
        }
        flush(&mut word, &mut out);
    }
    out
}

/// The words of the window in one language.
pub struct Words {
    /// Arabic only: English line to its Arabic line (reading order, unshaped).
    ar: Option<HashMap<&'static str, &'static str>>,
    /// The same lines ready to draw on one row.
    ready: HashMap<&'static str, String>,
}

impl Words {
    pub fn new(lang: Lang) -> Words {
        match lang {
            Lang::En => Words { ar: None, ready: HashMap::new() },
            Lang::Ar => Words {
                ar: Some(AR.iter().copied().collect()),
                ready: AR.iter().map(|&(en, ar)| (en, ready(ar))).collect(),
            },
        }
    }

    fn arabic(&self, en: &str) -> Option<&'static str> {
        self.ar.as_ref()?.get(en).copied()
    }

    /// A line for one row (a button, a tab, a title). A line with no Arabic stays English.
    pub fn get<'a>(&'a self, en: &'a str) -> &'a str {
        self.ready.get(en).map_or(en, String::as_str)
    }

    /// A line with each `{}` replaced by the next of `values`, in reading order (not yet ready
    /// to draw: keep it, show it with `row` or `wrap`). Native menus take it as it is.
    pub fn text(&self, en: &str, values: &[&str]) -> String {
        let mut line = self.arabic(en).unwrap_or(en).to_string();
        for v in values {
            line = line.replacen("{}", v, 1);
        }
        line
    }

    /// A line in reading order made ready to draw on one row.
    pub fn row(&self, line: &str) -> String {
        if self.ar.is_some() && has_arabic(line) { ready(line) } else { line.to_string() }
    }

    /// `text`, then `row`.
    pub fn fill(&self, en: &str, values: &[&str]) -> String {
        self.row(&self.text(en, values))
    }

    /// A line that may need several rows (an English line of the table, or any line in reading
    /// order). Arabic rows are broken here (`width` wide, measured by `measure`, which is given
    /// words in reading order), because a row must be put in visual order after it is broken.
    /// A line with no Arabic is left to egui.
    pub fn wrap(&self, line: &str, width: f32, measure: impl Fn(&str) -> f32) -> String {
        let line = self.arabic(line).unwrap_or(line);
        if self.ar.is_none() || !has_arabic(line) {
            return line.to_string();
        }
        let space = measure(" ");
        let (mut rows, mut row, mut used) = (Vec::new(), String::new(), 0.0);
        for word in line.split(' ') {
            let w = measure(word);
            if !row.is_empty() && used + space + w > width {
                rows.push(ready(&row));
                row.clear();
                used = 0.0;
            }
            if !row.is_empty() {
                row.push(' ');
                used += space;
            }
            row.push_str(word);
            used += w;
        }
        rows.push(ready(&row));
        rows.join("\n")
    }
}

fn has_arabic(line: &str) -> bool {
    line.chars().any(is_arabic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_left_alone() {
        let w = Words::new(Lang::En);
        assert_eq!(w.get("Whip"), "Whip");
        assert_eq!(w.fill("Version {}", &["0.4.0"]), "Version 0.4.0");
        assert_eq!(w.row("Ctrl+X: taken"), "Ctrl+X: taken");
        assert_eq!(w.wrap("a b c", 1.0, |s| s.len() as f32), "a b c");
    }

    #[test]
    fn every_arabic_line_is_there_once_with_its_values() {
        let mut seen = std::collections::HashSet::new();
        for (en, ar) in AR {
            assert!(seen.insert(en), "twice: {en}");
            assert!(!ar.trim().is_empty(), "{en}");
            assert_eq!(en.matches("{}").count(), ar.matches("{}").count(), "{en}");
            assert!(has_arabic(ar), "no Arabic: {en}");
        }
    }

    #[test]
    fn arabic_is_handed_over_in_pieces_in_visual_order() {
        let w = Words::new(Lang::Ar);
        // one Arabic word: one piece, its letters in reading order (egui lays it right to left)
        assert_eq!(w.get("Whip"), format!("{PIECE}السوط{PIECE}"));
        // two words: the first word of the sentence is the right-hand piece
        assert_eq!(w.get("Start with Windows"), format!("Windows {PIECE}مع{PIECE} {PIECE}ابدأ{PIECE}"));
        // a Latin word inside an Arabic line keeps its own order, at the far side
        assert!(w.get("Quit NJ-Whip").starts_with("NJ-Whip "));
        // brackets are turned with the direction
        assert!(w.fill("Heat meter and full screen effect (fills in {} cracks)", &["4"]).starts_with('('));
        // a number stays readable and sits to the left of the word before it
        assert_eq!(w.fill("Version {}", &["0.4.0"]), format!("0.4.0 {PIECE}الإصدار{PIECE}"));
        // a line with no Arabic in it is not turned around
        assert_eq!(w.row("Ctrl+X: taken."), "Ctrl+X: taken.");
        assert_eq!(w.wrap("Ctrl+X: taken.", 1.0, |s| s.len() as f32), "Ctrl+X: taken.");
    }

    #[test]
    fn arabic_rows_are_broken_before_they_are_reordered() {
        let w = Words::new(Lang::Ar);
        let text = "Glowing tube, clean light bead. Floods the screen with colour.";
        let one = w.wrap(text, 1e9, |s| s.chars().count() as f32);
        assert!(!one.contains('\n'));
        let two = w.wrap(text, one.chars().count() as f32 * 0.6, |s| s.chars().count() as f32);
        let rows: Vec<&str> = two.split('\n').collect();
        assert_eq!(rows.len(), 2);
        // the first row holds the start of the sentence, which is the END of the one-row form
        assert!(one.ends_with(rows[0]), "{one:?} / {rows:?}");
    }

    /// Every Arabic letter the window draws is in Thmanyah (checked where the face is installed).
    #[test]
    fn thmanyah_has_every_arabic_letter_the_window_draws() {
        use skrifa::MetadataProvider;
        let Some(bytes) = crate::glyphs::user_font("thmanyahsans-Regular.otf") else { return };
        let font = skrifa::FontRef::new(&bytes).unwrap();
        let map = font.charmap();
        for (en, ar) in AR {
            for c in ar.chars().filter(|&c| is_arabic(c)) {
                assert!(map.map(c).is_some(), "U+{:04X} in {en:?}", c as u32);
            }
        }
    }
}
