#[cfg(target_os = "windows")]
use crate::deck::{DeckState, index_for_numpad_key};
#[cfg(target_os = "windows")]
use rune::AppHandle;

#[derive(Clone, Default)]
pub struct HotkeyHandle {
    #[cfg(target_os = "windows")]
    thread_id: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

impl HotkeyHandle {
    pub fn request_hide(&self) {
        #[cfg(target_os = "windows")]
        win32::request_hide(&self.thread_id);
    }
}

#[cfg(target_os = "windows")]
mod win32 {
    use super::{AppHandle, DeckState, index_for_numpad_key};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, Ordering};
    use std::time::Duration;

    const WM_HOTKEY: u32 = 0x0312;
    const WM_TIMER: u32 = 0x0113;
    const WM_KEYDOWN: usize = 0x0100;
    const WM_KEYUP: usize = 0x0101;
    const WM_SYSKEYDOWN: usize = 0x0104;
    const WM_SYSKEYUP: usize = 0x0105;
    const WM_APP_HIDE: u32 = 0x8001;
    const WM_APP_HOTKEY: u32 = 0x8002;
    const MOD_NOREPEAT: u32 = 0x4000;
    const WH_KEYBOARD_LL: i32 = 13;
    const LLKHF_INJECTED: u32 = 0x0010;
    const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
    const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;
    const GA_ROOT: u32 = 2;
    const VK_NUMPAD0: u32 = 0x60;
    const VK_NUMPAD1: u32 = 0x61;
    const VK_NUMPAD9: u32 = 0x69;
    const VK_ADD: u32 = 0x6B;
    const VK_SUBTRACT: u32 = 0x6D;
    const ID_TOGGLE: i32 = 0;
    const ID_LAYER_UP: i32 = 10;
    const ID_LAYER_DOWN: i32 = 11;

    static HOTKEY_THREAD_ID: AtomicU32 = AtomicU32::new(0);
    static LL_HOOK: AtomicIsize = AtomicIsize::new(0);
    static DECK_VISIBLE: AtomicBool = AtomicBool::new(true);
    static NUMPAD0_DOWN: AtomicBool = AtomicBool::new(false);
    static SWALLOWED_N0: AtomicBool = AtomicBool::new(false);

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct POINT {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct MSG {
        hwnd: isize,
        message: u32,
        w_param: usize,
        l_param: isize,
        time: u32,
        pt: POINT,
    }

    #[repr(C)]
    struct KBDLLHOOKSTRUCT {
        vk_code: u32,
        scan_code: u32,
        flags: u32,
        time: u32,
        dw_extra_info: usize,
    }

    type HookProc = unsafe extern "system" fn(i32, usize, isize) -> isize;
    type WinEventProc =
        unsafe extern "system" fn(isize, u32, isize, i32, i32, u32, u32);

    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterHotKey(hwnd: isize, id: i32, fs_modifiers: u32, vk: u32) -> i32;
        fn UnregisterHotKey(hwnd: isize, id: i32) -> i32;
        fn GetMessageW(
            lp_msg: *mut MSG,
            hwnd: isize,
            w_msg_filter_min: u32,
            w_msg_filter_max: u32,
        ) -> i32;
        fn PostThreadMessageW(id_thread: u32, msg: u32, w_param: usize, l_param: isize) -> i32;
        fn SetWindowsHookExW(
            id_hook: i32,
            lpfn: Option<HookProc>,
            hmod: isize,
            dw_thread_id: u32,
        ) -> isize;
        fn UnhookWindowsHookEx(hhk: isize) -> i32;
        fn CallNextHookEx(hhk: isize, n_code: i32, w_param: usize, l_param: isize) -> isize;
        fn SetWinEventHook(
            event_min: u32,
            event_max: u32,
            hmod_win_event_proc: isize,
            pfn_win_event_proc: Option<WinEventProc>,
            id_process: u32,
            id_thread: u32,
            dw_flags: u32,
        ) -> isize;
        fn UnhookWinEvent(h_win_event_hook: isize) -> i32;
        fn GetForegroundWindow() -> isize;
        fn GetAncestor(hwnd: isize, ga_flags: u32) -> isize;
        fn GetClassNameW(hwnd: isize, lp_class_name: *mut u16, n_max_count: i32) -> i32;
        fn SetTimer(hwnd: isize, n_id_event: usize, u_elapse: u32, lp_timer_func: usize) -> usize;
        fn KillTimer(hwnd: isize, u_id_event: usize) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }

    fn register_deck_keys(register: bool) {
        for num in 1u32..=9 {
            let id = num as i32;
            let vk = VK_NUMPAD0 + num;
            unsafe {
                if register {
                    RegisterHotKey(0, id, MOD_NOREPEAT, vk);
                } else {
                    UnregisterHotKey(0, id);
                }
            }
        }
        unsafe {
            if register {
                RegisterHotKey(0, ID_LAYER_UP, MOD_NOREPEAT, VK_ADD);
                RegisterHotKey(0, ID_LAYER_DOWN, MOD_NOREPEAT, VK_SUBTRACT);
            } else {
                UnregisterHotKey(0, ID_LAYER_UP);
                UnregisterHotKey(0, ID_LAYER_DOWN);
            }
        }
    }

    pub fn request_hide(thread_id: &AtomicU32) {
        let tid = thread_id.load(Ordering::Relaxed);
        if tid != 0 {
            unsafe {
                PostThreadMessageW(tid, WM_APP_HIDE, 0, 0);
            }
        }
    }

    pub fn place_bottom_right(handle: &AppHandle<DeckState>, _window_size: (f32, f32)) {
        handle.place(
            rune::DisplayTarget::UnderPointer,
            rune::OverlayPlacement::Edge {
                edge: rune::ScreenEdge::Bottom,
                margin: 0.0,
                align: rune::Align::End,
                inset: 8.0,
            },
        );
    }

    fn hide_deck(handle: &AppHandle<DeckState>, hide_seq: &Arc<AtomicU64>, reset_grace_ms: u64) {
        handle.update(|s, _| {
            s.visible = false;
            s.set_pressed_key(None);
        });
        let seq = hide_seq.fetch_add(1, Ordering::Relaxed) + 1;

        // Let the slide-down spring finish before hiding the native window.
        let hide_seq_ref = Arc::clone(hide_seq);
        let hide_handle = handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(260));
            if hide_seq_ref.load(Ordering::Relaxed) == seq {
                hide_handle.hide();
            }
        });

        if reset_grace_ms == 0 {
            handle.update(|s, _| s.reset_to_root());
        } else {
            let timer_seq = Arc::clone(hide_seq);
            let timer_handle = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(reset_grace_ms));
                if timer_seq.load(Ordering::Relaxed) == seq {
                    timer_handle.update(|s, _| s.reset_to_root());
                }
            });
        }
    }

    fn is_rdp_window(hwnd: isize) -> bool {
        if hwnd == 0 {
            return false;
        }
        let check_class = |h: isize| -> bool {
            if h == 0 {
                return false;
            }
            let mut buf = [0u16; 64];
            let len = unsafe { GetClassNameW(h, buf.as_mut_ptr(), buf.len() as i32) };
            if len <= 0 {
                return false;
            }
            let name = String::from_utf16_lossy(&buf[..len as usize]);
            matches!(
                name.as_str(),
                "TscShellContainerClass" | "IHWindowClass" | "UIMainClass" | "OPWindowClass"
            )
        };
        check_class(hwnd) || check_class(unsafe { GetAncestor(hwnd, GA_ROOT) })
    }

    fn is_rdp_foreground() -> bool {
        is_rdp_window(unsafe { GetForegroundWindow() })
    }

    fn reinstall_ll_hook() {
        let prev = LL_HOOK.swap(0, Ordering::SeqCst);
        if prev != 0 {
            unsafe {
                UnhookWindowsHookEx(prev);
            }
        }
        let next = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard_proc), 0, 0) };
        LL_HOOK.store(next, Ordering::SeqCst);
    }

    unsafe extern "system" fn win_event_proc(
        _hook: isize,
        _event: u32,
        hwnd: isize,
        _id_object: i32,
        _id_child: i32,
        _id_event_thread: u32,
        _time: u32,
    ) {
        if is_rdp_window(hwnd) {
            reinstall_ll_hook();
            // mstsc may install/re-arm its own hook right after focus settles.
            unsafe {
                SetTimer(0, 0, 150, 0);
            }
        }
    }

    unsafe extern "system" fn ll_keyboard_proc(
        n_code: i32,
        w_param: usize,
        l_param: isize,
    ) -> isize {
        if n_code >= 0 && l_param != 0 {
            let kbd = unsafe { &*(l_param as *const KBDLLHOOKSTRUCT) };
            if (kbd.flags & LLKHF_INJECTED) == 0 {
                let is_down = w_param == WM_KEYDOWN || w_param == WM_SYSKEYDOWN;
                let is_up = w_param == WM_KEYUP || w_param == WM_SYSKEYUP;

                if kbd.vk_code == VK_NUMPAD0 {
                    if is_up {
                        NUMPAD0_DOWN.store(false, Ordering::Relaxed);
                        if SWALLOWED_N0.swap(false, Ordering::Relaxed) {
                            return 1;
                        }
                    } else if is_down && is_rdp_foreground() {
                        if !NUMPAD0_DOWN.swap(true, Ordering::Relaxed) {
                            let tid = HOTKEY_THREAD_ID.load(Ordering::Relaxed);
                            if tid != 0 {
                                unsafe {
                                    PostThreadMessageW(tid, WM_APP_HOTKEY, ID_TOGGLE as usize, 0);
                                }
                            }
                        }
                        SWALLOWED_N0.store(true, Ordering::Relaxed);
                        return 1;
                    }
                } else if is_down
                    && DECK_VISIBLE.load(Ordering::Relaxed)
                    && is_rdp_foreground()
                {
                    let hotkey_id = match kbd.vk_code {
                        vk if (VK_NUMPAD1..=VK_NUMPAD9).contains(&vk) => {
                            Some((vk - VK_NUMPAD0) as usize)
                        }
                        VK_ADD => Some(ID_LAYER_UP as usize),
                        VK_SUBTRACT => Some(ID_LAYER_DOWN as usize),
                        _ => None,
                    };
                    if let Some(id) = hotkey_id {
                        let tid = HOTKEY_THREAD_ID.load(Ordering::Relaxed);
                        if tid != 0 {
                            unsafe {
                                PostThreadMessageW(tid, WM_APP_HOTKEY, id, 0);
                            }
                        }
                        return 1;
                    }
                }
            }
        }
        unsafe { CallNextHookEx(LL_HOOK.load(Ordering::Relaxed), n_code, w_param, l_param) }
    }

    fn handle_hotkey_id(
        id: i32,
        handle: &AppHandle<DeckState>,
        visible: &mut bool,
        hide_seq: &Arc<AtomicU64>,
        reset_grace_ms: u64,
        window_size: (f32, f32),
    ) {
        match id {
            ID_TOGGLE => {
                *visible = !*visible;
                DECK_VISIBLE.store(*visible, Ordering::Relaxed);
                if *visible {
                    // Cancel any pending hide or grace-period reset if reopened in time.
                    hide_seq.fetch_add(1, Ordering::Relaxed);
                    place_bottom_right(handle, window_size);
                    handle.show();
                    handle.update(|s, _| s.visible = true);
                } else {
                    hide_deck(handle, hide_seq, reset_grace_ms);
                }
            }
            ID_LAYER_UP => handle.update(|s, _| s.layer_up()),
            ID_LAYER_DOWN => handle.update(|s, _| s.layer_down()),
            id if (1..=9).contains(&id) => {
                if let Some(idx) = index_for_numpad_key(id as u8) {
                    if *visible {
                        let press_handle = handle.clone();
                        press_handle.update(move |s, _| s.set_pressed_key(Some(idx)));
                        std::thread::spawn(move || {
                            std::thread::sleep(Duration::from_millis(110));
                            press_handle.update(move |s, _| {
                                s.set_pressed_key(None);
                                s.press_key(idx);
                            });
                        });
                    } else {
                        handle.update(move |s, _| s.press_key(idx));
                    }
                }
            }
            _ => {}
        }
    }

    pub fn spawn(
        handle: AppHandle<DeckState>,
        reset_grace_ms: u64,
        window_size: (f32, f32),
        thread_id: Arc<AtomicU32>,
    ) {
        std::thread::spawn(move || {
            let tid = unsafe { GetCurrentThreadId() };
            thread_id.store(tid, Ordering::Relaxed);
            HOTKEY_THREAD_ID.store(tid, Ordering::Relaxed);
            let ok = unsafe { RegisterHotKey(0, ID_TOGGLE, MOD_NOREPEAT, VK_NUMPAD0) };
            if ok == 0 {
                return;
            }

            let mut visible = true;
            DECK_VISIBLE.store(true, Ordering::Relaxed);
            let hide_seq = Arc::new(AtomicU64::new(0));
            register_deck_keys(true);
            reinstall_ll_hook();
            let fg_hook = unsafe {
                SetWinEventHook(
                    EVENT_SYSTEM_FOREGROUND,
                    EVENT_SYSTEM_FOREGROUND,
                    0,
                    Some(win_event_proc),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                )
            };

            let mut msg = MSG::default();
            while unsafe { GetMessageW(&mut msg, 0, 0, 0) } > 0 {
                if msg.message == WM_TIMER {
                    unsafe {
                        KillTimer(0, msg.w_param);
                    }
                    reinstall_ll_hook();
                    continue;
                }
                if msg.message == WM_APP_HIDE {
                    if visible {
                        visible = false;
                        DECK_VISIBLE.store(false, Ordering::Relaxed);
                        hide_deck(&handle, &hide_seq, reset_grace_ms);
                    }
                    continue;
                }
                if msg.message == WM_HOTKEY || msg.message == WM_APP_HOTKEY {
                    handle_hotkey_id(
                        msg.w_param as i32,
                        &handle,
                        &mut visible,
                        &hide_seq,
                        reset_grace_ms,
                        window_size,
                    );
                }
            }

            if fg_hook != 0 {
                unsafe {
                    UnhookWinEvent(fg_hook);
                }
            }
            let ll = LL_HOOK.swap(0, Ordering::SeqCst);
            if ll != 0 {
                unsafe {
                    UnhookWindowsHookEx(ll);
                }
            }
            register_deck_keys(false);
            unsafe {
                UnregisterHotKey(0, ID_TOGGLE);
            }
        });
    }
}

pub fn place_bottom_right(
    #[allow(unused)] handle: &rune::AppHandle<crate::deck::DeckState>,
    #[allow(unused)] window_size: (f32, f32),
) {
    #[cfg(target_os = "windows")]
    win32::place_bottom_right(handle, window_size);
}

pub fn spawn(
    #[allow(unused)] handle: rune::AppHandle<crate::deck::DeckState>,
    #[allow(unused)] reset_grace_ms: u64,
    #[allow(unused)] window_size: (f32, f32),
) -> HotkeyHandle {
    let hotkey_handle = HotkeyHandle::default();
    #[cfg(target_os = "windows")]
    win32::spawn(
        handle,
        reset_grace_ms,
        window_size,
        std::sync::Arc::clone(&hotkey_handle.thread_id),
    );
    hotkey_handle
}
