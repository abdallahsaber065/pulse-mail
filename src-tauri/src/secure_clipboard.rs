//! Copying secrets (verification codes) without leaving them in Windows
//! clipboard history or the cloud clipboard.
//!
//! Windows honors three registered formats placed next to the text:
//! `ExcludeClipboardContentFromMonitorProcessing` (any data), and
//! `CanIncludeInClipboardHistory` / `CanUploadToCloudClipboard` (DWORD 0).

/// Put `text` on the clipboard, marked as excluded from history and cloud
/// sync. Returns false if the clipboard couldn't be opened (caller falls back).
#[cfg(windows)]
pub fn write_secret(text: &str) -> bool {
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    use windows_sys::Win32::System::Ole::CF_UNICODETEXT;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    /// Copies `bytes` into a movable global block owned by the clipboard on success.
    unsafe fn set(format: u32, bytes: &[u8]) -> bool {
        unsafe {
            let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1));
            if h.is_null() {
                return false;
            }
            let p = GlobalLock(h) as *mut u8;
            if p.is_null() {
                GlobalFree(h);
                return false;
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
            GlobalUnlock(h);
            if SetClipboardData(format, h).is_null() {
                GlobalFree(h);
                return false;
            }
            true
        }
    }

    let text16: Vec<u8> = wide(text).iter().flat_map(|u| u.to_le_bytes()).collect();
    unsafe {
        // The clipboard can be held briefly by another app; retry a few times.
        let mut opened = false;
        for _ in 0..5 {
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if !opened {
            return false;
        }
        let ok = EmptyClipboard() != 0 && set(CF_UNICODETEXT as u32, &text16);
        if ok {
            let zero = 0u32.to_le_bytes();
            for (name, data) in [
                ("ExcludeClipboardContentFromMonitorProcessing", &zero[..]),
                ("CanIncludeInClipboardHistory", &zero[..]),
                ("CanUploadToCloudClipboard", &zero[..]),
            ] {
                let f = RegisterClipboardFormatW(wide(name).as_ptr());
                if f != 0 {
                    set(f, data);
                }
            }
        }
        CloseClipboard();
        ok
    }
}

/// Empty the clipboard (used to clear a code after the timeout).
#[cfg(windows)]
pub fn clear() -> bool {
    use windows_sys::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard};
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return false;
        }
        let ok = EmptyClipboard() != 0;
        CloseClipboard();
        ok
    }
}

#[cfg(not(windows))]
pub fn write_secret(_text: &str) -> bool {
    false
}

#[cfg(not(windows))]
pub fn clear() -> bool {
    false
}
