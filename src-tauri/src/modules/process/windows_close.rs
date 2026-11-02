#[cfg(target_os = "windows")]
pub(super) fn request_windows_close(pids: &[u32]) -> Result<(), String> {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE,
    };

    struct CloseRequest<'a> {
        pids: &'a [u32],
        failed: bool,
    }
    unsafe extern "system" fn visit(window: HWND, data: LPARAM) -> i32 {
        // EnumWindows invokes this synchronously while the request is alive.
        let request = &mut *(data as *mut CloseRequest<'_>);
        let mut pid = 0;
        GetWindowThreadProcessId(window, &mut pid);
        if pid != 0 && request.pids.contains(&pid) && IsWindowVisible(window) != 0
            && PostMessageW(window, WM_CLOSE, 0, 0) == 0
        {
            request.failed = true;
        }
        1
    }

    let mut request = CloseRequest { pids, failed: false };
    let enumerated = unsafe { EnumWindows(Some(visit), &mut request as *mut _ as LPARAM) };
    if enumerated == 0 || request.failed {
        return Err(
            "Unable to request Antigravity to close; please close it manually and retry".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    fn close_request_targets_only_windows_owned_by_the_requested_process() {
        use std::ptr::{null, null_mut};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, PeekMessageW, MSG, PM_REMOVE, WM_CLOSE, WS_POPUP,
            WS_VISIBLE,
        };

        // The fixture owns a tiny off-screen native window; no Antigravity process or
        // account is involved. Inspect the posted message without dispatching it.
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let window = unsafe {
            CreateWindowExW(
                0, class.as_ptr(), null(), WS_POPUP | WS_VISIBLE, -32000, -32000, 1, 1,
                null_mut(), null_mut(), null_mut(), null(),
            )
        };
        assert!(!window.is_null());
        let hidden = unsafe {
            CreateWindowExW(
                0, class.as_ptr(), null(), WS_POPUP, -32000, -32000, 1, 1,
                null_mut(), null_mut(), null_mut(), null(),
            )
        };
        assert!(!hidden.is_null());
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        let has_close = |handle, msg: &mut MSG| unsafe {
            PeekMessageW(msg, handle, WM_CLOSE, WM_CLOSE, PM_REMOVE) != 0
        };
        let unrelated = request_windows_close(&[u32::MAX]);
        let wrongly_posted = has_close(window, &mut msg);
        let targeted = request_windows_close(&[std::process::id()]);
        let correctly_posted = has_close(window, &mut msg);
        let hidden_posted = has_close(hidden, &mut msg);
        unsafe {
            DestroyWindow(hidden);
            DestroyWindow(window);
        }

        assert!(unrelated.is_ok());
        assert!(!wrongly_posted);
        assert!(targeted.is_ok());
        assert!(correctly_posted);
        assert!(!hidden_posted);
    }
}
