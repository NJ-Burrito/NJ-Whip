fn main() {
    // The exe's own icon (Explorer, taskbar): the NJ monogram.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("FileDescription", "NJ-Whip");
        res.set("ProductName", "NJ-Whip");
        res.set("LegalCopyright", "Copyright (c) 2026 NJ-Burrito. MIT License.");
        if let Err(e) = res.compile() {
            println!("cargo:warning=icon not embedded: {e}");
        }
    }
    println!("cargo:rerun-if-changed=assets/icon.ico");
}
