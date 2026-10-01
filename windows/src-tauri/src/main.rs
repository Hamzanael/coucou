// Coucou runs without a console window: Mochi is the whole UI.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // The island needs to place itself at the top of the screen, stay above
    // everything and read the global cursor. Wayland lets a client do none of
    // that, so on Linux we run through XWayland unless told otherwise.
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("GDK_BACKEND").is_none() {
            std::env::set_var("GDK_BACKEND", "x11");
        }
        // WebKitGTK's DMA-BUF renderer draws blank or opaque windows on a number
        // of drivers (NVIDIA especially); the island must stay transparent.
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }
    coucou_lib::run()
}
