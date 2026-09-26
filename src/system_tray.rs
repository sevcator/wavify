//! Windows notification-area icon. A tiny private window owns the shell icon
//! and forwards its menu actions back to the iced UI thread.

use std::sync::{
    mpsc::{self, Receiver, Sender},
    Mutex, OnceLock,
};
use std::thread::{self, JoinHandle};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMessageW, LoadImageW, PostMessageW, PostQuitMessage,
    RegisterClassW, SetForegroundWindow, TrackPopupMenu, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
    HICON, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED, MF_STRING, TPM_LEFTALIGN, TPM_NONOTIFY,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP, WM_CLOSE, WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONUP,
    WM_RBUTTONUP, WNDCLASSW, WS_POPUP,
};

const TRAY_CALLBACK: u32 = WM_APP + 0x51;
const MENU_OPEN: i32 = 1001;
const MENU_QUIT: i32 = 1002;
const TRAY_ICON_ID: u32 = 1;

#[derive(Debug, Clone, Copy)]
pub enum TrayAction {
    Open,
    Quit,
}

pub struct TrayController {
    hwnd: HWND,
    actions: Receiver<TrayAction>,
    thread: Option<JoinHandle<()>>,
}

static ACTION_SENDER: OnceLock<Mutex<Option<Sender<TrayAction>>>> = OnceLock::new();

impl TrayController {
    pub fn start() -> Result<Self, String> {
        let (action_tx, actions) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("wavify-system-tray".into())
            .spawn(move || tray_thread(action_tx, ready_tx))
            .map_err(|e| format!("could not start tray thread: {e}"))?;

        match ready_rx.recv() {
            Ok(Ok(hwnd)) => Ok(Self {
                hwnd: hwnd as HWND,
                actions,
                thread: Some(thread),
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(e) => {
                let _ = thread.join();
                Err(format!("tray thread did not initialize: {e}"))
            }
        }
    }

    pub fn try_action(&self) -> Option<TrayAction> {
        self.actions.try_recv().ok()
    }
}

impl Drop for TrayController {
    fn drop(&mut self) {
        unsafe { PostMessageW(self.hwnd, WM_CLOSE, 0, 0) };
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn tray_thread(action_tx: Sender<TrayAction>, ready: mpsc::SyncSender<Result<isize, String>>) {
    *ACTION_SENDER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = Some(action_tx);

    let result = unsafe { create_tray_window() };
    let hwnd = match result {
        Ok(hwnd) => hwnd,
        Err(error) => {
            *ACTION_SENDER
                .get_or_init(|| Mutex::new(None))
                .lock()
                .unwrap_or_else(|p| p.into_inner()) = None;
            let _ = ready.send(Err(error));
            return;
        }
    };
    if ready.send(Ok(hwnd as isize)).is_err() {
        unsafe { DestroyWindow(hwnd) };
        return;
    }

    unsafe {
        let mut message = std::mem::zeroed();
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    *ACTION_SENDER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = None;
}

unsafe fn create_tray_window() -> Result<HWND, String> {
    let instance = GetModuleHandleW(std::ptr::null());
    if instance.is_null() {
        return Err("could not get Wavify module handle".into());
    }
    let class_name = wide("WavifyTrayMessageWindow");
    let class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: std::ptr::null_mut(),
        hCursor: std::ptr::null_mut(),
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_name.as_ptr(),
    };
    if RegisterClassW(&class) == 0 {
        return Err("could not register tray message window".into());
    }
    let hwnd = CreateWindowExW(
        0,
        class_name.as_ptr(),
        class_name.as_ptr(),
        WS_POPUP,
        0,
        0,
        0,
        0,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        instance,
        std::ptr::null(),
    );
    if hwnd.is_null() {
        return Err("could not create tray message window".into());
    }

    // The executable embeds the same multi-size .ico used by Explorer and the
    // taskbar (resource group 1, emitted by build.rs).
    let icon: HICON = LoadImageW(
        instance,
        1usize as *const u16,
        IMAGE_ICON,
        0,
        0,
        LR_DEFAULTSIZE | LR_SHARED,
    ) as _;
    if icon.is_null() {
        DestroyWindow(hwnd);
        return Err("could not load Wavify's embedded tray icon".into());
    }
    let mut data: NOTIFYICONDATAW = std::mem::zeroed();
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = TRAY_ICON_ID;
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = TRAY_CALLBACK;
    data.hIcon = icon;
    for (dst, src) in data.szTip.iter_mut().zip("Wavify".encode_utf16()) {
        *dst = src;
    }
    if Shell_NotifyIconW(NIM_ADD, &data) == 0 {
        DestroyWindow(hwnd);
        return Err("Windows could not add Wavify to the system tray".into());
    }
    Ok(hwnd)
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        TRAY_CALLBACK if lparam as u32 == WM_LBUTTONUP => {
            send_action(TrayAction::Open);
            0
        }
        TRAY_CALLBACK if lparam as u32 == WM_RBUTTONUP || lparam as u32 == WM_CONTEXTMENU => {
            show_tray_menu(hwnd);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            let mut data: NOTIFYICONDATAW = std::mem::zeroed();
            data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            data.hWnd = hwnd;
            data.uID = TRAY_ICON_ID;
            Shell_NotifyIconW(NIM_DELETE, &data);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn show_tray_menu(hwnd: HWND) {
    let menu = CreatePopupMenu();
    if menu.is_null() {
        return;
    }
    let open = wide("Open Wavify");
    let quit = wide("Quit");
    AppendMenuW(menu, MF_STRING, MENU_OPEN as usize, open.as_ptr());
    AppendMenuW(menu, MF_STRING, MENU_QUIT as usize, quit.as_ptr());
    let mut point: POINT = std::mem::zeroed();
    GetCursorPos(&mut point);
    SetForegroundWindow(hwnd);
    let selected = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | TPM_LEFTALIGN,
        point.x,
        point.y,
        0,
        hwnd,
        std::ptr::null(),
    );
    DestroyMenu(menu);
    PostMessageW(hwnd, 0, 0, 0);
    match selected {
        MENU_OPEN => send_action(TrayAction::Open),
        MENU_QUIT => send_action(TrayAction::Quit),
        _ => {}
    }
}

fn send_action(action: TrayAction) {
    if let Some(sender) = ACTION_SENDER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
    {
        let _ = sender.send(action);
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
