// The sound clips, by name. The crate that includes this file says what a clip is with its own
// `clip!` macro: the app embeds the file's bytes, the website build keeps the file's name.

clip!(CRACK_1, "crack-1.mp3");
clip!(CRACK_2, "crack-2.mp3");
clip!(CRACK_3, "crack-3.mp3");
clip!(CRACK_4, "crack-4.mp3");
clip!(CRACK_5, "crack-5.mp3");
/// The leather cracks: one at random per crack of the Classic whip.
pub const LEATHER: [&[u8]; 5] = [CRACK_1, CRACK_2, CRACK_3, CRACK_4, CRACK_5];

// The whips' own clips (a whip lists them in `WhipDef::sounds`), played on top of the crack.
clip!(NEON, "neon.mp3");
clip!(ELEPHANT, "elephant.mp3");
clip!(COWBOY, "cowboy.mp3");
clip!(CHEER, "cheer.mp3");
clip!(DRILL, "drill.mp3");
clip!(GONG, "gong.mp3");
clip!(NINJA, "ninja.mp3");
clip!(PIRATE, "pirate.mp3");

// Each whip's own crack (listed in `WhipDef::cracks`). The Classic whip cracks with the leather ones.
clip!(NEON_CRACK, "neon-crack.mp3");
clip!(ELEPHANT_CRACK, "elephant-crack.mp3");
clip!(COWBOY_CRACK, "cowboy-crack.mp3");
clip!(CHEER_CRACK, "cheer-crack.mp3");
clip!(SAUCE_CRACK, "sauce-crack.mp3");
clip!(DRILL_CRACK, "drill-crack.mp3");
clip!(ZEN_CRACK, "zen-crack.mp3");
clip!(NINJA_CRACK, "ninja-crack.mp3");
clip!(PIRATE_CRACK, "pirate-crack.mp3");
clip!(CHEF_CRACK, "chef-crack.mp3");
clip!(WIZARD_CRACK, "wizard-crack.mp3");
clip!(VINE_CRACK, "vine-crack.mp3");
clip!(PENGUIN_CRACK, "penguin-crack.mp3");
clip!(STORM_CRACK, "storm-crack.mp3");
