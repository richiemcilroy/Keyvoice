mod audio;
mod permissions;
mod platform;
mod tray;
mod window;
mod whisper;

mod fn_key_listener;
mod fn_key_monitor;

use audio::{AudioDevice, AudioManager};
use permissions::Permissions;
use whisper::WhisperModel;

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{State, Manager, Listener};
use tauri_plugin_store::StoreExt;
use tauri_specta::{collect_commands, collect_events, Builder, Event};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct TranscriptionProgress {
    pub text: String,
    pub is_final: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct RecordingStateChanged {
    pub is_recording: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct WordCountUpdated {
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct HotkeyPressed {
    pub pressed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct FnKeyStateChanged {
    pub is_pressed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct AudioLevelUpdate {
    pub level: f32,
}

#[derive(Serialize, Deserialize, specta::Type)]
pub struct AppSettings {
    pub selected_microphone: Option<String>,
    pub word_count: u32,
    pub hotkey: Option<String>,
}

pub struct BubbleShowTaskState {
    pub handle: Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>>,
}

impl AppSettings {
    pub fn get<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<Option<Self>, String> {
        let store = app.store("settings.json").map_err(|e| e.to_string())?;
        if let Some(value) = store.get("app_settings") {
            let settings: Self = serde_json::from_value(value).map_err(|e| e.to_string())?;
            Ok(Some(settings))
        } else {
            Ok(None)
        }
    }

    pub fn set<R: tauri::Runtime>(app: &tauri::AppHandle<R>, settings: &Self) -> Result<(), String> {
        let store = app.store("settings.json").map_err(|e| e.to_string())?;
        let value = serde_json::to_value(settings).map_err(|e| e.to_string())?;
        store.set("app_settings", value);
        store.save().map_err(|e| e.to_string())
    }

    pub fn get_or_default<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Self {
        Self::get(app).unwrap_or(None).unwrap_or_default()
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            selected_microphone: None,
            word_count: 0,
            hotkey: None,
        }
    }
}


#[tauri::command]
#[specta::specta]
async fn get_audio_devices() -> Result<Vec<AudioDevice>, String> {
    AudioManager::list_audio_devices().await
}

#[tauri::command]
#[specta::specta]
async fn set_recording_device(
    app: tauri::AppHandle,
    audio_manager: State<'_, Arc<AudioManager>>,
    device_id: String,
) -> Result<(), String> {
    audio_manager.set_current_device(device_id.clone()).await?;
    
    let mut settings = AppSettings::get_or_default(&app);
    settings.selected_microphone = Some(device_id);
    AppSettings::set(&app, &settings)?;
    
    let _ = tray::update_tray_menu(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn get_current_device(
    app: tauri::AppHandle,
    audio_manager: State<'_, Arc<AudioManager>>,
) -> Result<Option<String>, String> {
    let settings = AppSettings::get_or_default(&app);
    if let Some(device_id) = settings.selected_microphone {
        if audio_manager.get_current_device().await.is_none() {
            let _ = audio_manager.set_current_device(device_id.clone()).await;
        }
        return Ok(Some(device_id));
    }
    
    Ok(audio_manager.get_current_device().await)
}

#[tauri::command]
#[specta::specta]
fn check_permissions() -> Result<Permissions, String> {
    Ok(Permissions::check())
}

#[tauri::command]
#[specta::specta]
async fn start_recording(audio_manager: State<'_, Arc<AudioManager>>) -> Result<(), String> {
    audio_manager.start_recording().await
}

#[tauri::command]
#[specta::specta]
async fn stop_recording(audio_manager: State<'_, Arc<AudioManager>>) -> Result<(), String> {
    let _audio_data = audio_manager.stop_recording().await?;
    
    Ok(())
}

#[tauri::command]
#[specta::specta]
fn request_microphone_permission(app: tauri::AppHandle) -> Result<bool, String> {
    Permissions::request_permission("microphone")?;
    let _ = tray::update_tray_menu(&app);
    Ok(true)
}

#[tauri::command]
#[specta::specta]
fn request_accessibility_permission(app: tauri::AppHandle) -> Result<bool, String> {
    Permissions::request_permission("accessibility")?;
    let _ = tray::update_tray_menu(&app);
    Ok(true)
}

#[tauri::command]
#[specta::specta]
fn refresh_permissions(app: tauri::AppHandle) -> Result<Permissions, String> {
    let permissions = Permissions::check();
    let _ = tray::update_tray_menu(&app);
    Ok(permissions)
}

#[tauri::command]
#[specta::specta]
fn get_word_count(app: tauri::AppHandle) -> Result<u32, String> {
    let settings = AppSettings::get_or_default(&app);
    Ok(settings.word_count)
}

#[tauri::command]
#[specta::specta]
fn update_word_count(app: tauri::AppHandle, count: u32) -> Result<(), String> {
    let mut settings = AppSettings::get_or_default(&app);
    settings.word_count = count;
    AppSettings::set(&app, &settings)
}

#[tauri::command]
#[specta::specta]
fn get_hotkey(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let settings = AppSettings::get_or_default(&app);
    Ok(settings.hotkey)
}

#[tauri::command]
#[specta::specta]
fn set_hotkey(
    app: tauri::AppHandle, 
    hotkey: String, 
    audio_manager: State<'_, Arc<AudioManager>>,
    bubble_task_state: State<'_, BubbleShowTaskState>
) -> Result<(), String> {
    println!("📌 Setting hotkey: {}", &hotkey);
    
    let mut settings = AppSettings::get_or_default(&app);
    settings.hotkey = Some(hotkey.clone());
    AppSettings::set(&app, &settings)?;
    
    if hotkey == "fn" {
        println!("✅ Fn key selected - will use FnKeyStateChanged events");
        return Ok(());
    }
    
    let shortcut_key = match hotkey.as_str() {
        "rightOption" => "Alt",
        "leftOption" => "Alt", 
        "leftControl" => "Control",
        "rightControl" => "Control",
        "rightCommand" => "Meta",
        "rightShift" => "Shift",
        _ => return Err(format!("Unsupported hotkey: {}", hotkey))
    };
    
    let shortcut_manager = app.global_shortcut();
    
    let settings = AppSettings::get_or_default(&app);
    if let Some(old_hotkey) = settings.hotkey {
        if old_hotkey != "fn" {
            let old_shortcut_key = match old_hotkey.as_str() {
                "rightOption" | "leftOption" => "Alt",
                "leftControl" | "rightControl" => "Control",
                "rightCommand" => "Meta",
                "rightShift" => "Shift",
                _ => ""
            };
            if !old_shortcut_key.is_empty() && shortcut_manager.is_registered(old_shortcut_key) {
                println!("🔓 Unregistering old hotkey: {}", old_shortcut_key);
                shortcut_manager.unregister(old_shortcut_key).map_err(|e| e.to_string())?;
            }
        }
    }
    
    let app_handle = app.clone();
    let audio_manager_clone = audio_manager.inner().clone();
    let hotkey_str = hotkey.clone();
    let bubble_handle = bubble_task_state.handle.clone();
    
    shortcut_manager
        .on_shortcut(shortcut_key, move |_app, _shortcut, event| {
            match event.state() {
                ShortcutState::Pressed => {
                    println!("🎤 Hotkey pressed: {} - Starting recording", &hotkey_str);
                    HotkeyPressed { pressed: true }.emit(&app_handle).ok();
                    RecordingStateChanged { is_recording: true }.emit(&app_handle).ok();
                    let audio_manager = audio_manager_clone.clone();
                    let app_handle_for_bubble = app_handle.clone();
                    let bubble_handle_clone = bubble_handle.clone();
                    let handle = tauri::async_runtime::spawn(async move {
                        let _ = audio_manager.start_recording().await;
                        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                        let _ = window::show_bubble_window(&app_handle_for_bubble);
                    });
                    *bubble_handle_clone.lock().unwrap() = Some(handle);
                }
                ShortcutState::Released => {
                    println!("🛑 Hotkey released: {} - Stopping recording", &hotkey_str);
                    HotkeyPressed { pressed: false }.emit(&app_handle).ok();
                    RecordingStateChanged { is_recording: false }.emit(&app_handle).ok();
                    
                    if let Some(handle) = bubble_handle.lock().unwrap().take() {
                        handle.abort();
                        println!("🚫 Cancelled bubble show task");
                    }
                    
                    let app_handle_hide = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                        let _ = window::hide_bubble_window(&app_handle_hide);
                    });
                    let audio_manager = audio_manager_clone.clone();
                    let app_handle_clone = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Ok(_audio_data) = audio_manager.stop_recording().await {
                            TranscriptionProgress {
                                text: "Test transcription".to_string(),
                                is_final: true,
                            }.emit(&app_handle_clone).ok();
                        }
                    });
                }
            }
        })
        .map_err(|e| format!("Failed to register hotkey: {}", e))?;
    
    println!("✅ Hotkey registered successfully: {}", &hotkey);
    
    Ok(())
}

#[tauri::command]
#[specta::specta]
fn validate_hotkey(_app: tauri::AppHandle, hotkey: String) -> Result<bool, String> {
    match hotkey.as_str() {
        "rightOption" | "leftOption" | "leftControl" | "rightControl" | "fn" | "rightCommand" | "rightShift" => {
            Ok(true)
        }
        _ => Ok(false)
    }
}

#[tauri::command]
#[specta::specta]
fn insert_text_at_cursor(_text: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
#[specta::specta]
fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    window::show_main_window(&app)
}



#[tauri::command]
#[specta::specta]
fn is_fn_key_pressed(_app: tauri::AppHandle) -> Result<bool, String> {
    let is_pressed = fn_key_monitor::is_fn_pressed();
    println!("🔍 is_fn_key_pressed command called - result: {}", is_pressed);
    Ok(is_pressed)
}

#[tauri::command]
#[specta::specta]
fn test_fn_key(app: tauri::AppHandle) -> Result<String, String> {
    let new_state = fn_key_monitor::toggle_fn_pressed();
    println!("🔑 Fn key toggled to: {}", new_state);
    
    FnKeyStateChanged { is_pressed: new_state }.emit(&app).ok();
    
    Ok(format!("Fn key state toggled to: {}", new_state))
}

#[tauri::command]
#[specta::specta]
fn check_model_downloaded() -> Result<bool, String> {
    Ok(WhisperModel::is_downloaded())
}

#[tauri::command]
#[specta::specta]
async fn download_whisper_model(app: tauri::AppHandle) -> Result<(), String> {
    WhisperModel::download(&app).await
}

#[tauri::command]
#[specta::specta]
fn get_model_path() -> Result<String, String> {
    WhisperModel::get_model_path()
        .map(|p| p.to_string_lossy().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let audio_manager = Arc::new(AudioManager::new());
    let fn_listener: Arc<std::sync::Mutex<Option<fn_key_listener::FnKeyListener>>> = Arc::new(std::sync::Mutex::new(None));
    
    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            get_audio_devices,
            set_recording_device,
            get_current_device,
            check_permissions,
            start_recording,
            stop_recording,
            request_microphone_permission,
            request_accessibility_permission,
            refresh_permissions,
            get_word_count,
            update_word_count,
            get_hotkey,
            set_hotkey,
            validate_hotkey,
            insert_text_at_cursor,
            show_main_window,
            is_fn_key_pressed,
            test_fn_key,
            check_model_downloaded,
            download_whisper_model,
            get_model_path
        ])
        .events(collect_events![
            TranscriptionProgress,
            RecordingStateChanged,
            WordCountUpdated,
            HotkeyPressed,
            FnKeyStateChanged,
            AudioLevelUpdate,
            whisper::ModelDownloadProgress,
            whisper::ModelDownloadComplete
        ]);
    
    #[cfg(debug_assertions)]
    builder
        .export(specta_typescript::Typescript::default(), "../src/bindings/index.ts")
        .expect("Failed to export typescript bindings");
    
    let bubble_task_state = BubbleShowTaskState {
        handle: Arc::new(Mutex::new(None)),
    };
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(audio_manager)
        .manage(fn_listener.clone())
        .manage(bubble_task_state)
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            println!("🚀 TalkType starting up...");
            
            let audio_manager = app.state::<Arc<AudioManager>>();
            let app_handle = app.handle().clone();
            tauri::async_runtime::block_on(async {
                audio_manager.set_app_handle(app_handle).await;
            });
            
            tray::create_tray(&app.handle())?;
            
            let window = window::create_main_window(&app.handle())?;
            window::setup_window_handlers(&window, &app.handle());
            
            window.show()?;
            
            let settings = AppSettings::get_or_default(&app.handle());
            if let Some(saved_hotkey) = settings.hotkey {
                println!("📥 Found saved hotkey: {}", &saved_hotkey);
            } else {
                println!("❌ No saved hotkey found");
            }
            
            #[cfg(target_os = "macos")]
            {
                let fn_listener_state = app.state::<Arc<std::sync::Mutex<Option<fn_key_listener::FnKeyListener>>>>();
                let mut listener = fn_key_listener::FnKeyListener::new(app.handle().clone());
                match listener.start() {
                    Ok(_) => {
                        println!("✅ Fn key listener started successfully");
                        *fn_listener_state.inner().lock().unwrap() = Some(listener);
                    }
                    Err(e) => {
                        println!("⚠️ Failed to start Fn key listener: {}", e);
                        println!("⚠️ Fn key support will be unavailable. Please ensure Input Monitoring permission is granted.");
                    }
                }
            }
            
            let app_handle = app.handle().clone();
            app.handle().listen("show-main-window", move |_event| {
                let _ = window::show_main_window(&app_handle);
            });
            
            let app_handle_fn = app.handle().clone();
            let bubble_task_state = app.state::<BubbleShowTaskState>();
            let bubble_show_handle = bubble_task_state.handle.clone();
            app.handle().listen("fn-key-state-changed", move |event| {
                let settings = AppSettings::get_or_default(&app_handle_fn);
                if let Some(hotkey) = settings.hotkey {
                    if hotkey == "fn" {
                        if let Ok(payload) = serde_json::from_str::<FnKeyStateChanged>(event.payload()) {
                            let audio_manager = app_handle_fn.state::<Arc<AudioManager>>().inner().clone();
                            if payload.is_pressed {
                                println!("🎤 Fn key pressed - Starting recording");
                                HotkeyPressed { pressed: true }.emit(&app_handle_fn).ok();
                                RecordingStateChanged { is_recording: true }.emit(&app_handle_fn).ok();
                                let audio_manager = audio_manager.clone();
                                let app_handle_for_bubble = app_handle_fn.clone();
                                let bubble_show_handle_clone = bubble_show_handle.clone();
                                let handle = tauri::async_runtime::spawn(async move {
                                    let _ = audio_manager.start_recording().await;
                                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                                    let _ = window::show_bubble_window(&app_handle_for_bubble);
                                });
                                *bubble_show_handle_clone.lock().unwrap() = Some(handle);
                            } else {
                                println!("🛑 Fn key released - Stopping recording");
                                HotkeyPressed { pressed: false }.emit(&app_handle_fn).ok();
                                RecordingStateChanged { is_recording: false }.emit(&app_handle_fn).ok();
                                
                                if let Some(handle) = bubble_show_handle.lock().unwrap().take() {
                                    handle.abort();
                                    println!("🚫 Cancelled bubble show task");
                                }
                                
                                let app_handle_hide = app_handle_fn.clone();
                                tauri::async_runtime::spawn(async move {
                                    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                                    let _ = window::hide_bubble_window(&app_handle_hide);
                                });
                                let audio_manager = audio_manager.clone();
                                let app_handle_clone = app_handle_fn.clone();
                                tauri::async_runtime::spawn(async move {
                                    if let Ok(_audio_data) = audio_manager.stop_recording().await {
                                        TranscriptionProgress {
                                            text: "Test transcription".to_string(),
                                            is_final: true,
                                        }.emit(&app_handle_clone).ok();
                                    }
                                });
                            }
                        }
                    }
                }
            });
            
            let _ = window::create_bubble_window(app.handle());
            #[cfg(target_os = "macos")]
            {
                window::start_dock_monitor(&app.handle());
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}