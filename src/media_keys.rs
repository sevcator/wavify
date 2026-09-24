//! Global media keys (play/pause, next, previous, stop). A window only gets
//! key events while it has focus; a low-level keyboard hook sees the media
//! keys whatever window is active, and passes them on, so other apps still
//! get them too.

use iced::futures::channel::mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKey {
    PlayPause,
    Next,
    Previous,
    Stop,
}

/// Set once the hook is installed: the window then leaves media keys to it,
/// or a focused window would act on every press twice.
pub static HOOKED: AtomicBool = AtomicBool::new(false);

static SENDER: Mutex<Option<mpsc::UnboundedSender<MediaKey>>> = Mutex::new(None);
static STARTED: AtomicBool = AtomicBool::new(false);

/// Media key presses, for as long as the subscription runs.
pub fn subscription() -> iced::Subscription<MediaKey> {
    iced::Subscription::run(listen)
}

fn listen() -> mpsc::UnboundedReceiver<MediaKey> {
    let (tx, rx) = mpsc::unbounded();
    // a restarted subscription takes over the one hook thread
    if let Ok(mut s) = SENDER.lock() {
        *s = Some(tx);
    }
    if !STARTED.swap(true, Ordering::SeqCst) {
        hook::start();
    }
    rx
}

fn send(key: MediaKey) {
    if let Ok(s) = SENDER.lock() {
        if let Some(tx) = s.as_ref() {
            let _ = tx.unbounded_send(key);
        }
    }
}

#[cfg(windows)]
mod hook {
    use super::{send, MediaKey, HOOKED};
    use std::sync::atomic::{AtomicU32, Ordering};
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
        HC_ACTION, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
        WM_SYSKEYUP,
    };

    const VK_MEDIA_NEXT_TRACK: u32 = 0xB0;
    const VK_MEDIA_PREV_TRACK: u32 = 0xB1;
    const VK_MEDIA_STOP: u32 = 0xB2;
    const VK_MEDIA_PLAY_PAUSE: u32 = 0xB3;

    /// Media keys held down (bit per key): a held key fires once, not on
    /// every auto-repeat.
    static DOWN: AtomicU32 = AtomicU32::new(0);

    pub fn start() {
        std::thread::spawn(|| unsafe {
            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(proc),
                GetModuleHandleW(std::ptr::null()),
                0,
            );
            if hook.is_null() {
                crate::log!("media keys: hook not installed");
                return;
            }
            HOOKED.store(true, Ordering::SeqCst);
            // a low-level hook is called through this thread's message loop
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });
    }

    unsafe extern "system" fn proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
            let key = match kb.vkCode {
                VK_MEDIA_PLAY_PAUSE => Some(MediaKey::PlayPause),
                VK_MEDIA_NEXT_TRACK => Some(MediaKey::Next),
                VK_MEDIA_PREV_TRACK => Some(MediaKey::Previous),
                VK_MEDIA_STOP => Some(MediaKey::Stop),
                _ => None,
            };
            if let Some(key) = key {
                let bit = 1u32 << (kb.vkCode - VK_MEDIA_NEXT_TRACK);
                let msg = wparam as u32;
                if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                    if DOWN.fetch_or(bit, Ordering::SeqCst) & bit == 0 {
                        send(key);
                    }
                } else if msg == WM_KEYUP || msg == WM_SYSKEYUP {
                    DOWN.fetch_and(!bit, Ordering::SeqCst);
                }
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
    }
}

#[cfg(not(windows))]
mod hook {
    /// No global hook here: the window handles media keys while focused.
    pub fn start() {}
}
