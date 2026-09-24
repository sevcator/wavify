// The shared modules come from the library crate (lib.rs), so they are
// compiled once for the app and the test binaries.
use wavify::{
    alt_source, api, auth, config, console, cookie_auth, discord_rpc, dlog, log, proxy_pool, sc_web,
};

mod dropdown;
mod dsp;
mod login_window;
mod media_keys;
mod player;
mod selftest;
mod ui;
mod updater;
mod virtual_list;
mod webview_debug;
mod yt_music;
mod yt_web;

fn main() {
    console::attach();
    let args: Vec<String> = std::env::args().collect();

    // --debug (or a parent in debug mode): this process logs to a file of
    // its own under <config>/debug, and its children do too
    if args.iter().any(|a| a == "--debug") || std::env::var_os(console::DEBUG_ENV).is_some() {
        let role = [
            ("--selftest", "selftest"),
            ("--login", "login"),
            ("--yt-login", "yt-login"),
            ("--yt-player", "yt-player"),
            ("--sc-bridge", "sc-bridge"),
        ]
        .iter()
        .find(|(flag, _)| args.iter().any(|a| a == flag))
        .map(|(_, role)| *role)
        .unwrap_or("app");
        if let Some(path) = console::start_debug(role) {
            log!("debug log: {}", path.display());
            log!(
                "wavify {} ({role}) {:?}, args {:?}",
                env!("CARGO_PKG_VERSION"),
                std::env::current_exe().ok(),
                &args[1..]
            );
        }
    }
    log!("wavify starting");

    // Hidden selftest harness: exercises every API action, then exits.
    if args.iter().any(|a| a == "--selftest") {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        match rt.block_on(selftest::run()) {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                crate::log!("selftest fatal: {e}");
                std::process::exit(1);
            }
        }
    }

    // Child processes: YouTube Music's sign-in window and hidden player.
    if args.iter().any(|a| a == "--yt-login") {
        yt_web::run_login();
    }
    if args.iter().any(|a| a == "--yt-player") {
        yt_web::run_player();
    }

    // Child process: hidden SoundCloud WebView2 bridge for mutation operations
    if args.iter().any(|a| a == "--sc-bridge") {
        sc_web::run_bridge();
    }

    // Child process: embedded WebView2 login window. Writes token.json then exits.
    if args.iter().any(|a| a == "--login") {
        match login_window::login_window() {
            Ok(_) => {
                std::process::exit(0);
            }
            Err(e) => {
                let _ = std::fs::write(config::auth_error_path(), e.to_string());
                std::process::exit(1);
            }
        }
    }

    // Protocol-handler invocation from external browser (sc://auth?code=...).
    if args.len() > 1 {
        if let Some(code) = auth::parse_code_from_arg(&args[1]) {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            match rt.block_on(auth::complete_login(code)) {
                Ok(()) => return,
                Err(e) => {
                    let _ = std::fs::write(config::auth_error_path(), e.to_string());
                    return;
                }
            }
        }
    }

    let _ = config::register_sc_protocol();

    let settings = config::Settings::load();
    // a proxy URL can hold a password: only whether there is one
    log!(
        "settings loaded (proxy={}, quality={:?})",
        if settings.proxy.is_some() { "set" } else { "-" },
        settings.audio_quality
    );
    // started by Windows at login ("Open Wavify automatically": Minimized)
    let minimized = args.iter().any(|a| a == "--minimized");
    ui::run(settings, minimized);
}
