use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

pub fn create_main_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let mut builder = WebviewWindow::builder(app, "main", WebviewUrl::App("index.html".into()))
        .title("TalkType")
        .inner_size(600.0, 500.0)
        .resizable(false)
        .center()
        .visible(false)
        .accept_first_mouse(true);

    #[cfg(target_os = "macos")]
    {
        builder = builder
            .hidden_title(true)
            .title_bar_style(tauri::TitleBarStyle::Transparent);
    }

    #[cfg(target_os = "windows")]
    {
        builder = builder.decorations(false);
    }

    builder.build()
}

pub fn setup_window_handlers(window: &WebviewWindow, app_handle: &AppHandle) {
    let app_handle = app_handle.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();

            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.hide();
            }
        }
    });

    #[cfg(target_os = "macos")]
    {
        use objc::runtime::Object;
        use objc::{msg_send, sel, sel_impl};

        if let Ok(ns_window) = window.ns_window() {
            let ns_window = ns_window as *mut Object;
            unsafe {
                let _: () = msg_send![ns_window, setTitlebarAppearsTransparent: true];
                let _: () = msg_send![ns_window, setTitleVisibility: 1];

                let close_button: *mut Object = msg_send![ns_window, standardWindowButton: 0];
                let miniaturize_button: *mut Object = msg_send![ns_window, standardWindowButton: 1];
                let zoom_button: *mut Object = msg_send![ns_window, standardWindowButton: 2];

                if !close_button.is_null() {
                    let _: () = msg_send![close_button, setFrameOrigin: (14.0, 6.0)];
                }

                if !miniaturize_button.is_null() {
                    let _: () = msg_send![miniaturize_button, setFrameOrigin: (34.0, 6.0)];
                }

                if !zoom_button.is_null() {
                    let _: () = msg_send![zoom_button, setFrameOrigin: (54.0, 6.0)];
                    let _: () = msg_send![zoom_button, setEnabled: false];
                }
            }
        }
    }
}

pub fn show_main_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        window.unminimize().map_err(|e| e.to_string())?;
    } else {
        let window = create_main_window(app).map_err(|e| e.to_string())?;
        setup_window_handlers(&window, app);
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn create_bubble_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let monitor = app.primary_monitor().unwrap().unwrap();
    let screen_size = monitor.size();
    let scale_factor = monitor.scale_factor();

    let bubble_width = 70.0;
    let bubble_height = 35.0;

    let horizontal_padding = 20.0;
    let vertical_padding = 20.0;

    let window_width = bubble_width + horizontal_padding;
    let window_height = bubble_height + vertical_padding;

    #[allow(unused_mut)]
    let mut dock_height: f64 = 70.0;

    #[cfg(target_os = "macos")]
    {
        use objc::runtime::Object;
        use objc::{msg_send, sel, sel_impl};

        #[repr(C)]
        #[derive(Clone, Copy)]
        struct NSPoint {
            x: f64,
            y: f64,
        }

        #[repr(C)]
        #[derive(Clone, Copy)]
        struct NSSize {
            width: f64,
            height: f64,
        }

        #[repr(C)]
        #[derive(Clone, Copy)]
        struct NSRect {
            origin: NSPoint,
            size: NSSize,
        }

        unsafe {
            println!("🖥️ Attempting to determine Dock height using NSScreen...");
            if let Some(cls) = objc::runtime::Class::get("NSScreen") {
                let main_screen: *mut Object = msg_send![cls, mainScreen];
                if !main_screen.is_null() {
                    let frame: NSRect = msg_send![main_screen, frame];
                    let visible: NSRect = msg_send![main_screen, visibleFrame];

                    println!(
                        "🖥️ NSScreen frame: origin=({}, {}), size=({}, {})",
                        frame.origin.x, frame.origin.y, frame.size.width, frame.size.height
                    );
                    println!(
                        "🖥️ NSScreen visibleFrame: origin=({}, {}), size=({}, {})",
                        visible.origin.x, visible.origin.y, visible.size.width, visible.size.height
                    );

                    let calculated = visible.origin.y;
                    println!(
                        "🖥️ Calculated dock height (visible.origin.y): {}",
                        calculated
                    );
                    if calculated > 0.0 {
                        dock_height = calculated;
                        println!("✅ Using calculated dock height: {}", dock_height);
                    } else {
                        println!(
                            "⚠️ Calculated dock height not positive, using fallback: {}",
                            dock_height
                        );
                    }
                } else {
                    println!(
                        "❌ NSScreen mainScreen is null, using fallback dock height: {}",
                        dock_height
                    );
                }
            } else {
                println!(
                    "❌ NSScreen class not found, using fallback dock height: {}",
                    dock_height
                );
            }
        }
    }

    let gap_above_dock = 5.0;
    println!("🖥️ gap_above_dock: {}", gap_above_dock);

    let x = (screen_size.width as f64 / scale_factor - window_width) / 2.0;
    let y = screen_size.height as f64 / scale_factor - window_height - dock_height - gap_above_dock;

    let mut builder = WebviewWindow::builder(app, "bubble", WebviewUrl::App("bubble.html".into()))
        .title("TalkType Recording")
        .inner_size(window_width, window_height)
        .position(x, y)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .decorations(false)
        .transparent(true)
        .accept_first_mouse(true);

    #[cfg(target_os = "macos")]
    {
        builder = builder
            .hidden_title(true)
            .title_bar_style(tauri::TitleBarStyle::Transparent);
    }

    builder.build()
}

pub fn show_bubble_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("bubble") {
        println!("🫧 Showing bubble window");
        window.show().map_err(|e| {
            println!("❌ Failed to show bubble window: {}", e);
            e.to_string()
        })?;
        println!("✅ Bubble window shown successfully");
    } else {
        println!("❌ Bubble window not found. It should have been created at startup.");
        return Err("Bubble window not found. It should have been created at startup.".to_string());
    }
    Ok(())
}

pub fn hide_bubble_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("bubble") {
        println!("🫧 Hiding bubble window");
        window.hide().map_err(|e| {
            println!("❌ Failed to hide bubble window: {}", e);
            e.to_string()
        })?;
        println!("✅ Bubble window hidden successfully");
    } else {
        println!("⚠️ Bubble window not found when trying to hide");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn current_dock_height() -> f64 {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSPoint {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSSize {
        width: f64,
        height: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NSRect {
        origin: NSPoint,
        size: NSSize,
    }

    let mut dock_height: f64 = 70.0;

    unsafe {
        if let Some(cls) = objc::runtime::Class::get("NSScreen") {
            let main_screen: *mut Object = msg_send![cls, mainScreen];
            if !main_screen.is_null() {
                let visible: NSRect = msg_send![main_screen, visibleFrame];
                let calculated = visible.origin.y;
                if calculated > 0.0 {
                    dock_height = calculated;
                }
            }
        }
    }
    dock_height
}

#[cfg(target_os = "macos")]
pub fn start_dock_monitor(app: &AppHandle) {
    use tauri::Manager;
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut previous_height = current_dock_height();
        loop {
            let height = current_dock_height();
            if (height - previous_height).abs() > 1.0 {
                if let Some(monitor) = app_handle.primary_monitor().unwrap_or(None) {
                    let scale_factor = monitor.scale_factor();
                    let screen_size = monitor.size();
                    let bubble_width = 70.0;
                    let bubble_height = 35.0;
                    let horizontal_padding = 20.0;
                    let vertical_padding = 20.0;
                    let window_width = bubble_width + horizontal_padding;
                    let window_height = bubble_height + vertical_padding;
                    let gap_above_dock = 5.0;
                    let x = (screen_size.width as f64 / scale_factor - window_width) / 2.0;
                    let y = screen_size.height as f64 / scale_factor
                        - window_height
                        - height
                        - gap_above_dock;
                    if let Some(window) = app_handle.get_webview_window("bubble") {
                        let _ = window.set_position(LogicalPosition::new(x, y));
                    }
                }
                previous_height = height;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    });
}
