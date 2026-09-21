//! While Hangar is minimised (typically: a game is running), ask WebView2 to give memory back.

use tauri::{Runtime, WebviewWindow, WindowEvent};

pub fn install<R: Runtime>(window: &WebviewWindow<R>) {
    let target = window.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Resized(_)) {
            let low = target.is_minimized().unwrap_or(false);
            set_low_memory(&target, low);
        }
    });
}

#[cfg(windows)]
fn set_low_memory<R: Runtime>(window: &WebviewWindow<R>, low: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use windows_core_webview::Interface;

    let _ = window.with_webview(move |webview| unsafe {
        let Ok(core) = webview.controller().CoreWebView2() else { return };
        // Needs WebView2 runtime 1.0.1823+; older runtimes simply keep the normal level.
        if let Ok(core) = core.cast::<ICoreWebView2_19>() {
            let level = if low {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
            } else {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
            };
            let _ = core.SetMemoryUsageTargetLevel(level);
        }
    });
}

#[cfg(not(windows))]
fn set_low_memory<R: Runtime>(_window: &WebviewWindow<R>, _low: bool) {}
