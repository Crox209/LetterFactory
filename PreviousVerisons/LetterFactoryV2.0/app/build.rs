fn main() {
    // Embed the application icon into the Windows .exe (and taskbar).
    // No-op on non-Windows targets.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let _ = embed_resource::compile("assets/icon.rc", embed_resource::NONE);
    }
}
