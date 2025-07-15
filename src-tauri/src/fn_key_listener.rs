#[cfg(target_os = "macos")]
use crate::FnKeyStateChanged;
#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use cocoa::foundation::NSAutoreleasePool;
#[cfg(target_os = "macos")]
use objc::runtime::{Object, BOOL, NO, YES};
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "macos")]
use tauri::AppHandle;
#[cfg(target_os = "macos")]
use tauri_specta::Event;

#[cfg(target_os = "macos")]
static FN_KEY_PRESSED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
pub struct FnKeyListener {
    app_handle: AppHandle,
    global_monitor: Option<id>,
    local_monitor: Option<id>,
}

#[cfg(target_os = "macos")]
impl FnKeyListener {
    pub fn new(app_handle: AppHandle) -> Self {
        FnKeyListener {
            app_handle,
            global_monitor: None,
            local_monitor: None,
        }
    }

    pub fn start(&mut self) -> Result<(), String> {
        unsafe {
            let _pool = NSAutoreleasePool::new(nil);
            let app_handle = self.app_handle.clone();

            let monitor_block = move |event: id| {
                let flags: u64 = msg_send![event, modifierFlags];
                let keycode: u16 = msg_send![event, keyCode];

                println!("🔍 Global monitor: keycode={}, flags={:#x}", keycode, flags);

                let current_state = FN_KEY_PRESSED.load(Ordering::SeqCst);

                let mut fn_pressed = (flags & 0x800000) != 0;

                if (keycode == 63 || keycode == 179) && (flags & 0x800000) == 0 {
                    fn_pressed = !current_state;
                    println!(
                        "🎯 Detected Fn via keycode fallback: {} ({} )",
                        keycode,
                        if fn_pressed { "pressed" } else { "released" }
                    );
                }

                if fn_pressed != current_state {
                    FN_KEY_PRESSED.store(fn_pressed, Ordering::SeqCst);
                    crate::fn_key_monitor::set_fn_pressed(fn_pressed);
                    println!(
                        "🎯 Fn key {} (global monitor)",
                        if fn_pressed { "pressed" } else { "released" }
                    );

                    FnKeyStateChanged {
                        is_pressed: fn_pressed,
                    }
                    .emit(&app_handle)
                    .ok();
                    println!(
                        "📤 Emitted FnKeyStateChanged event: is_pressed={}",
                        fn_pressed
                    );
                } else {
                    println!("🔸 Fn state unchanged: {}", fn_pressed);
                }
            };
            let global_block = ConcreteBlock::new(monitor_block).copy();
            std::mem::forget(global_block.clone());
            let mask_flags_changed = 1u64 << 12;
            let global_monitor: id = msg_send![
                class!(NSEvent),
                addGlobalMonitorForEventsMatchingMask: mask_flags_changed
                handler: &*global_block
            ];

            let app_handle_local = self.app_handle.clone();
            let local_block = move |event: id| -> id {
                let flags: u64 = msg_send![event, modifierFlags];
                let keycode: u16 = msg_send![event, keyCode];
                println!("🔍 Local monitor: keycode={}, flags={:#x}", keycode, flags);

                let is_fn_key = (flags & 0x800000) != 0 || keycode == 63 || keycode == 179;

                if (keycode == 63 || keycode == 179) && (flags & 0x800000) == 0 {
                    println!("🎯 Detected Fn via keycode fallback: {}", keycode);
                }

                if is_fn_key {
                    let fn_pressed = (flags & 0x800000) != 0;

                    let current_state = FN_KEY_PRESSED.load(Ordering::SeqCst);
                    if fn_pressed != current_state {
                        FN_KEY_PRESSED.store(fn_pressed, Ordering::SeqCst);
                        crate::fn_key_monitor::set_fn_pressed(fn_pressed);
                        println!(
                            "🎯 Fn key {} (local monitor)",
                            if fn_pressed { "pressed" } else { "released" }
                        );

                        FnKeyStateChanged {
                            is_pressed: fn_pressed,
                        }
                        .emit(&app_handle_local)
                        .ok();
                        println!(
                            "📤 Emitted FnKeyStateChanged event: is_pressed={}",
                            fn_pressed
                        );
                    } else {
                        println!("🔸 Fn state unchanged: {}", fn_pressed);
                    }

                    println!("📍 Local monitor: Processed Fn event");
                }

                event
            };
            let local_block = ConcreteBlock::new(local_block).copy();
            std::mem::forget(local_block.clone());
            let local_monitor: id = msg_send![
                class!(NSEvent),
                addLocalMonitorForEventsMatchingMask: mask_flags_changed
                handler: &*local_block
            ];

            if global_monitor != nil {
                println!("✅ Global NSEvent monitor installed");
            }
            if local_monitor != nil {
                println!("✅ Local NSEvent monitor installed (Fn will be swallowed)");
            }

            self.global_monitor = if global_monitor == nil {
                None
            } else {
                Some(global_monitor)
            };
            self.local_monitor = if local_monitor == nil {
                None
            } else {
                Some(local_monitor)
            };
        }

        Ok(())
    }

    pub fn stop(&mut self) {
        unsafe {
            if let Some(m) = self.global_monitor.take() {
                let _: () = msg_send![class!(NSEvent), removeMonitor: m];
            }
            if let Some(m) = self.local_monitor.take() {
                let _: () = msg_send![class!(NSEvent), removeMonitor: m];
            }
        }
    }

    pub fn is_fn_pressed(&self) -> bool {
        FN_KEY_PRESSED.load(Ordering::SeqCst)
    }
}

#[cfg(target_os = "macos")]
use block::ConcreteBlock;

unsafe impl Send for FnKeyListener {}
unsafe impl Sync for FnKeyListener {}
