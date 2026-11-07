//! The window's Windows abilities. The window has no title bar of its own (the app's header takes
//! its place, Main.qml), and Qt then leaves out Windows' minimize and maximize abilities. Without
//! them Windows ignores a double-click on the header and a click on the taskbar button doesn't
//! minimize. They're added back here; the buttons themselves are the app's own.

#[cfg(windows)]
pub fn restore_window_abilities() {
    use std::ffi::c_void;
    type Hwnd = *mut c_void;
    const GWL_STYLE: i32 = -16;
    const WS_MINIMIZEBOX: isize = 0x0002_0000;
    const WS_MAXIMIZEBOX: isize = 0x0001_0000;
    const WS_VISIBLE: isize = 0x1000_0000;
    const SWP_FLAGS: u32 = 0x0001 | 0x0002 | 0x0004 | 0x0010 | 0x0020; // no size, move, z-order, activate; frame changed
    #[link(name = "user32")]
    extern "system" {
        fn EnumThreadWindows(thread: u32, f: extern "system" fn(Hwnd, isize) -> i32, data: isize) -> i32;
        fn GetWindowLongPtrW(w: Hwnd, index: i32) -> isize;
        fn SetWindowLongPtrW(w: Hwnd, index: i32, value: isize) -> isize;
        fn SetWindowPos(w: Hwnd, after: Hwnd, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThreadId() -> u32;
    }
    extern "system" fn each(w: Hwnd, _: isize) -> i32 {
        unsafe {
            let style = GetWindowLongPtrW(w, GWL_STYLE);
            if style & WS_VISIBLE != 0 {
                SetWindowLongPtrW(w, GWL_STYLE, style | WS_MINIMIZEBOX | WS_MAXIMIZEBOX);
                SetWindowPos(w, std::ptr::null_mut(), 0, 0, 0, 0, SWP_FLAGS);
            }
        }
        1
    }
    // The window was made on this (the GUI) thread when Main.qml loaded.
    unsafe { EnumThreadWindows(GetCurrentThreadId(), each, 0) };
}

#[cfg(not(windows))]
pub fn restore_window_abilities() {}
