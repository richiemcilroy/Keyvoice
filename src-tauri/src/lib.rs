mod audio;
mod permissions;
mod platform;
mod tray;
mod window;
mod whisper;
mod transcripts;

mod fn_key_listener;
mod fn_key_monitor;

use audio::{AudioDevice, AudioManager};
use permissions::Permissions;
use whisper::WhisperModel;
use transcripts::{Transcript, TranscriptStore};

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{State, Manager, Listener, RunEvent};
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

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct RecordingStatsUpdated {
    pub total_words: u32,
    pub total_time_ms: f64,
    pub overall_wpm: f32,
    pub session_words: u32,
    pub session_time_ms: f64,
    pub session_wpm: f32,
}

#[derive(Serialize, Deserialize, specta::Type)]
pub struct AppSettings {
    pub selected_microphone: Option<String>,
    pub word_count: u32,
    pub hotkey: Option<String>,
    pub selected_model: Option<String>,
    pub total_recording_time_ms: f64,
    pub first_recording_time: Option<i64>,
    pub last_recording_time: Option<i64>,
    pub current_session_start: Option<i64>,
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
            selected_model: Some("tiny.en-q8_0".to_string()),
            total_recording_time_ms: 0.0,
            first_recording_time: None,
            last_recording_time: None,
            current_session_start: None,
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
async fn start_recording(
    app: tauri::AppHandle,
    audio_manager: State<'_, Arc<AudioManager>>
) -> Result<(), String> {
    let start_time = chrono::Utc::now().timestamp_millis();
    
    let mut settings = AppSettings::get_or_default(&app);
    settings.current_session_start = Some(start_time);
    
    if settings.first_recording_time.is_none() {
        settings.first_recording_time = Some(start_time);
    }
    
    AppSettings::set(&app, &settings)?;
    
    audio_manager.start_recording().await
}

#[tauri::command]
#[specta::specta]
async fn stop_recording(
    app: tauri::AppHandle,
    audio_manager: State<'_, Arc<AudioManager>>,
    whisper_model: State<'_, Arc<Mutex<WhisperModel>>>,
) -> Result<String, String> {
    let start_time = std::time::Instant::now();
    
    let (audio_data, sample_rate, peak_level) = audio_manager.stop_recording().await?;
    let stop_recording_time = start_time.elapsed();
    println!("⏱️ Stop recording took: {:?}", stop_recording_time);
    
    const SILENCE_THRESHOLD: f32 = 0.01;
    
    if audio_data.is_empty() || peak_level < SILENCE_THRESHOLD {
        println!("🔇 Skipping transcription - no meaningful audio detected (peak level: {:.4})", peak_level);
        return Ok(String::new());
    }
    
    let audio_duration_secs = audio_data.len() as f32 / sample_rate as f32;
    println!("🎙️ Audio duration: {:.2}s ({} samples at {} Hz)", audio_duration_secs, audio_data.len(), sample_rate);
    
    let transcribe_start = std::time::Instant::now();
    let text = {
        let model = whisper_model.lock().unwrap();
        model.transcribe(&audio_data, sample_rate)?
    };
    let transcribe_time = transcribe_start.elapsed();
    println!("⏱️ Transcription took: {:?} (RTF: {:.2}x)", transcribe_time, transcribe_time.as_secs_f32() / audio_duration_secs);
    
    let trimmed_text = text.trim();
    if trimmed_text.chars().all(|c| c.is_whitespace() || c.is_ascii_punctuation()) {
        println!("🔇 Skipping transcription - only contains punctuation/whitespace: '{}'", trimmed_text);
        return Ok(String::new());
    }
    
    let words = text.split_whitespace().count() as u32;
    if words > 0 || audio_data.len() > 0 {
        let end_time = chrono::Utc::now().timestamp_millis();
        let mut settings = AppSettings::get_or_default(&app);
        
        let session_duration_ms = if let Some(start) = settings.current_session_start {
            (end_time - start) as f64
        } else {
            0.0
        };
        
        settings.word_count += words;
        settings.total_recording_time_ms += session_duration_ms;
        settings.last_recording_time = Some(end_time);
        
        let overall_wpm = if settings.total_recording_time_ms > 0.0 {
            (settings.word_count as f32 / (settings.total_recording_time_ms as f32 / 60000.0))
        } else {
            0.0
        };
        
        let session_wpm = if session_duration_ms > 0.0 && words > 10 {
            (words as f32 / (session_duration_ms as f32 / 60000.0))
        } else {
            0.0
        };
        
        settings.current_session_start = None;
        
        AppSettings::set(&app, &settings)?;
        
        RecordingStatsUpdated {
            total_words: settings.word_count,
            total_time_ms: settings.total_recording_time_ms,
            overall_wpm,
            session_words: words,
            session_time_ms: session_duration_ms,
            session_wpm,
        }.emit(&app).ok();
        
        WordCountUpdated { count: settings.word_count }.emit(&app).ok();
        
        if !text.is_empty() {
            let transcript = Transcript {
                id: uuid::Uuid::new_v4().to_string(),
                text: text.clone(),
                timestamp: chrono::Utc::now().timestamp_millis() as f64,
                duration_ms: session_duration_ms,
                word_count: words,
                wpm: session_wpm,
                model_used: settings.selected_model.clone(),
            };
            
            let mut store = TranscriptStore::load(&app).unwrap_or_default();
            store.add_transcript(transcript);
            let _ = store.save(&app);
        }
    }
    
    let total_time = start_time.elapsed();
    println!("⏱️ Total stop_recording command took: {:?}", total_time);
    
    Ok(text)
}

#[tauri::command]
#[specta::specta]
async fn stop_recording_manual(
    app: tauri::AppHandle,
    audio_manager: State<'_, Arc<AudioManager>>,
    whisper_model: State<'_, Arc<Mutex<WhisperModel>>>,
) -> Result<String, String> {
    let start_time = std::time::Instant::now();
    
    let (audio_data, sample_rate, peak_level) = audio_manager.stop_recording().await?;
    let stop_recording_time = start_time.elapsed();
    println!("⏱️ Stop recording took: {:?}", stop_recording_time);
    
    const SILENCE_THRESHOLD: f32 = 0.01;
    
    if audio_data.is_empty() || peak_level < SILENCE_THRESHOLD {
        println!("🔇 Skipping transcription - no meaningful audio detected (peak level: {:.4})", peak_level);
        return Ok(String::new());
    }
    
    let audio_duration_secs = audio_data.len() as f32 / sample_rate as f32;
    println!("🎙️ Audio duration: {:.2}s ({} samples at {} Hz)", audio_duration_secs, audio_data.len(), sample_rate);
    
    let transcribe_start = std::time::Instant::now();
    let text = {
        let model = whisper_model.lock().unwrap();
        model.transcribe(&audio_data, sample_rate)?
    };
    let transcribe_time = transcribe_start.elapsed();
    println!("⏱️ Transcription took: {:?} (RTF: {:.2}x)", transcribe_time, transcribe_time.as_secs_f32() / audio_duration_secs);
    
    let trimmed_text = text.trim();
    if trimmed_text.is_empty() || trimmed_text.chars().all(|c| c.is_ascii_punctuation() || c.is_whitespace()) {
        println!("🔇 Skipping - transcription contains no meaningful text");
        return Ok(String::new());
    }
    
    let word_stats_start = std::time::Instant::now();
    
    let words = text.split_whitespace().count() as u32;
    
    if words > 0 {
        let mut settings = AppSettings::get_or_default(&app);
        let session_duration_ms = if let Some(start_time) = settings.current_session_start {
            (chrono::Utc::now().timestamp_millis() - start_time) as f64
        } else {
            0.0
        };
        
        let session_wpm = if session_duration_ms > 0.0 {
            (words as f64 / (session_duration_ms / 60000.0)) as f32
        } else {
            0.0
        };
        
        settings.word_count += words;
        settings.total_recording_time_ms += session_duration_ms;
        settings.last_recording_time = Some(chrono::Utc::now().timestamp_millis());
        settings.current_session_start = None;
        
        let overall_wpm = if settings.total_recording_time_ms > 0.0 {
            (settings.word_count as f64 / (settings.total_recording_time_ms / 60000.0)) as f32
        } else {
            0.0
        };
        
        AppSettings::set(&app, &settings)?;
        
        let _ = WordCountUpdated { count: settings.word_count }.emit(&app);
        
        let _ = RecordingStatsUpdated {
            total_words: settings.word_count,
            total_time_ms: settings.total_recording_time_ms,
            overall_wpm,
            session_words: words,
            session_time_ms: session_duration_ms,
            session_wpm,
        }.emit(&app);
        
        if words > 0 {
            let transcript = Transcript {
                id: uuid::Uuid::new_v4().to_string(),
                text: text.clone(),
                timestamp: chrono::Utc::now().timestamp_millis() as f64,
                duration_ms: session_duration_ms,
                word_count: words,
                wpm: session_wpm,
                model_used: settings.selected_model.clone(),
            };
            
            let mut store = TranscriptStore::load(&app).unwrap_or_default();
            store.add_transcript(transcript);
            let _ = store.save(&app);
        }
    }
    
    let word_stats_time = word_stats_start.elapsed();
    println!("⏱️ Word stats update took: {:?}", word_stats_time);
    
    let total_time = start_time.elapsed();
    println!("⏱️ Total stop_recording_manual command took: {:?}", total_time);
    
    Ok(text)
}

#[tauri::command]
#[specta::specta]
async fn stop_recording_chunked(
    app: tauri::AppHandle,
    audio_manager: State<'_, Arc<AudioManager>>,
    whisper_model: State<'_, Arc<Mutex<WhisperModel>>>,
) -> Result<String, String> {
    let start_time = std::time::Instant::now();
    
    let (audio_data, sample_rate, peak_level) = audio_manager.stop_recording().await?;
    let stop_recording_time = start_time.elapsed();
    println!("⏱️ Stop recording took: {:?}", stop_recording_time);
    
    const SILENCE_THRESHOLD: f32 = 0.01;
    
    if audio_data.is_empty() || peak_level < SILENCE_THRESHOLD {
        println!("🔇 Skipping transcription - no meaningful audio detected (peak level: {:.4})", peak_level);
        return Ok(String::new());
    }
    
    let audio_duration_secs = audio_data.len() as f32 / sample_rate as f32;
    println!("🎙️ Audio duration: {:.2}s ({} samples at {} Hz)", audio_duration_secs, audio_data.len(), sample_rate);
    
    let transcribe_start = std::time::Instant::now();
    let app_clone = app.clone();
    let text = {
        let model = whisper_model.lock().unwrap();
        if audio_duration_secs < 10.0 {
            let result = model.transcribe(&audio_data, sample_rate)?;
            TranscriptionProgress {
                text: result.clone(),
                is_final: true,
            }.emit(&app_clone).ok();
            result
        } else {
            model.transcribe_chunked(&audio_data, sample_rate, 30.0, |partial_text, is_final| {
                TranscriptionProgress {
                    text: partial_text.to_string(),
                    is_final,
                }.emit(&app_clone).ok();
            })?
        }
    };
    let transcribe_time = transcribe_start.elapsed();
    println!("⏱️ Chunked transcription took: {:?} (RTF: {:.2}x)", transcribe_time, transcribe_time.as_secs_f32() / audio_duration_secs);
    
    let trimmed_text = text.trim();
    if trimmed_text.chars().all(|c| c.is_whitespace() || c.is_ascii_punctuation()) {
        println!("🔇 Skipping transcription - only contains punctuation/whitespace: '{}'", trimmed_text);
        return Ok(String::new());
    }
    
    let words = text.split_whitespace().count() as u32;
    if words > 0 || audio_data.len() > 0 {
        let end_time = chrono::Utc::now().timestamp_millis();
        let mut settings = AppSettings::get_or_default(&app);
        
        let session_duration_ms = if let Some(start) = settings.current_session_start {
            (end_time - start) as f64
        } else {
            0.0
        };
        
        settings.word_count += words;
        settings.total_recording_time_ms += session_duration_ms;
        settings.last_recording_time = Some(end_time);
        
        let overall_wpm = if settings.total_recording_time_ms > 0.0 {
            (settings.word_count as f32 / (settings.total_recording_time_ms as f32 / 60000.0))
        } else {
            0.0
        };
        
        let session_wpm = if session_duration_ms > 0.0 && words > 10 {
            (words as f32 / (session_duration_ms as f32 / 60000.0))
        } else {
            0.0
        };
        
        settings.current_session_start = None;
        
        AppSettings::set(&app, &settings)?;
        
        RecordingStatsUpdated {
            total_words: settings.word_count,
            total_time_ms: settings.total_recording_time_ms,
            overall_wpm,
            session_words: words,
            session_time_ms: session_duration_ms,
            session_wpm,
        }.emit(&app).ok();
        
        WordCountUpdated { count: settings.word_count }.emit(&app).ok();
        
        if !text.is_empty() {
            let transcript = Transcript {
                id: uuid::Uuid::new_v4().to_string(),
                text: text.clone(),
                timestamp: chrono::Utc::now().timestamp_millis() as f64,
                duration_ms: session_duration_ms,
                word_count: words,
                wpm: session_wpm,
                model_used: settings.selected_model.clone(),
            };
            
            let mut store = TranscriptStore::load(&app).unwrap_or_default();
            store.add_transcript(transcript);
            let _ = store.save(&app);
        }
    }
    
    let total_time = start_time.elapsed();
    println!("⏱️ Total stop_recording_chunked command took: {:?}", total_time);
    
    Ok(text)
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
fn get_recording_stats(app: tauri::AppHandle) -> Result<RecordingStatsUpdated, String> {
    let settings = AppSettings::get_or_default(&app);
    
    let overall_wpm = if settings.total_recording_time_ms > 0.0 {
        (settings.word_count as f32 / (settings.total_recording_time_ms as f32 / 60000.0))
    } else {
        0.0
    };
    
    Ok(RecordingStatsUpdated {
        total_words: settings.word_count,
        total_time_ms: settings.total_recording_time_ms,
        overall_wpm,
        session_words: 0,
        session_time_ms: 0.0,
        session_wpm: 0.0,
    })
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
                    let app_handle_for_recording = app_handle.clone();
                    let bubble_handle_clone = bubble_handle.clone();
                    let handle = tauri::async_runtime::spawn(async move {
                        let start_time = chrono::Utc::now().timestamp_millis();
                        let mut settings = AppSettings::get_or_default(&app_handle_for_recording);
                        settings.current_session_start = Some(start_time);
                        if settings.first_recording_time.is_none() {
                            settings.first_recording_time = Some(start_time);
                        }
                        let _ = AppSettings::set(&app_handle_for_recording, &settings);
                        
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
                    let app_handle_clone = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        match app_handle_clone.try_state::<Arc<AudioManager>>() {
                            Some(audio_state) => {
                                match app_handle_clone.try_state::<Arc<Mutex<WhisperModel>>>() {
                                    Some(whisper_state) => {
                                        match stop_recording_chunked(app_handle_clone.clone(), audio_state, whisper_state).await {
                                            Ok(text) => {
                                                if !text.is_empty() {
                                                    let _ = insert_text_at_cursor(text);
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!("Failed to transcribe: {}", e);
                                            }
                                        }
                                    }
                                    None => {
                                        eprintln!("Failed to get whisper model state");
                                    }
                                }
                            }
                            None => {
                                eprintln!("Failed to get audio manager state");
                            }
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
fn insert_text_at_cursor(text: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use cocoa::base::{nil, id};
        use cocoa::foundation::{NSAutoreleasePool, NSString};
        use objc::{msg_send, sel, sel_impl, class};
        use core_graphics::event::{CGEvent, CGEventTapLocation, CGKeyCode};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
        
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            
            let pasteboard_class = class!(NSPasteboard);
            let pasteboard: id = msg_send![pasteboard_class, generalPasteboard];
            
            let _old_types: id = msg_send![pasteboard, types];
            let old_items: id = msg_send![pasteboard, readObjectsForClasses:nil options:nil];
            
            let _: () = msg_send![pasteboard, clearContents];
            let ns_string = NSString::alloc(nil).init_str(&text);
            let array_class = class!(NSArray);
            let string_array: id = msg_send![array_class, arrayWithObject: ns_string];
            let _: () = msg_send![pasteboard, writeObjects: string_array];
            
            let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).unwrap();
            
            if let Ok(cmd_down) = CGEvent::new_keyboard_event(source.clone(), 0x37 as CGKeyCode, true) {
                cmd_down.set_flags(core_graphics::event::CGEventFlags::CGEventFlagCommand);
                cmd_down.post(CGEventTapLocation::HID);
            }
            
            if let Ok(v_down) = CGEvent::new_keyboard_event(source.clone(), 0x09 as CGKeyCode, true) {
                v_down.set_flags(core_graphics::event::CGEventFlags::CGEventFlagCommand);
                v_down.post(CGEventTapLocation::HID);
            }
            
            if let Ok(v_up) = CGEvent::new_keyboard_event(source.clone(), 0x09 as CGKeyCode, false) {
                v_up.set_flags(core_graphics::event::CGEventFlags::CGEventFlagCommand);
                v_up.post(CGEventTapLocation::HID);
            }
            
            if let Ok(cmd_up) = CGEvent::new_keyboard_event(source.clone(), 0x37 as CGKeyCode, false) {
                cmd_up.post(CGEventTapLocation::HID);
            }
            
            std::thread::sleep(std::time::Duration::from_millis(50));
            
            if old_items != nil {
                let _: () = msg_send![pasteboard, clearContents];
                let _: () = msg_send![pasteboard, writeObjects: old_items];
            }
            
            let _: () = msg_send![pool, release];
        }
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        return Err("Text insertion not implemented for this platform".to_string());
    }
    
    Ok(())
}

#[tauri::command]
#[specta::specta]
fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    window::show_main_window(&app)
}

#[tauri::command]
#[specta::specta]
fn get_transcripts(app: tauri::AppHandle, limit: Option<u32>) -> Result<Vec<Transcript>, String> {
    let store = TranscriptStore::load(&app).unwrap_or_default();
    Ok(store.get_transcripts(limit))
}

#[tauri::command]
#[specta::specta]
fn get_transcript_stats(app: tauri::AppHandle) -> Result<transcripts::TranscriptStats, String> {
    let store = TranscriptStore::load(&app).unwrap_or_default();
    Ok(store.calculate_stats())
}

#[tauri::command]
#[specta::specta]
fn delete_transcript(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let mut store = TranscriptStore::load(&app).unwrap_or_default();
    store.delete_transcript(&id)?;
    store.save(&app)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
fn clear_all_transcripts(app: tauri::AppHandle) -> Result<(), String> {
    let mut store = TranscriptStore::load(&app).unwrap_or_default();
    store.clear_all();
    store.save(&app)?;
    Ok(())
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
fn check_model_downloaded(app: tauri::AppHandle) -> Result<bool, String> {
    let settings = AppSettings::get_or_default(&app);
    if let Some(model_id) = settings.selected_model {
        Ok(WhisperModel::is_downloaded(&model_id))
    } else {
        Ok(false)
    }
}

#[tauri::command]
#[specta::specta]
async fn download_whisper_model(
    app: tauri::AppHandle,
    whisper_model: State<'_, Arc<Mutex<WhisperModel>>>,
) -> Result<(), String> {
    let settings = AppSettings::get_or_default(&app);
    let model_id = settings.selected_model.ok_or_else(|| "No model selected".to_string())?;
    
    WhisperModel::download(&app, &model_id).await?;
    
    let mut model = whisper_model.lock().unwrap();
    model.load_model(Some(model_id))?;
    
    Ok(())
}

#[tauri::command]
#[specta::specta]
fn get_available_models() -> Result<Vec<whisper::WhisperModelInfo>, String> {
    Ok(whisper::WhisperModelInfo::all())
}

#[tauri::command]
#[specta::specta]
fn get_downloaded_models() -> Result<Vec<String>, String> {
    Ok(WhisperModel::get_downloaded_models())
}

#[tauri::command]
#[specta::specta]
fn get_selected_model(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let settings = AppSettings::get_or_default(&app);
    Ok(settings.selected_model)
}

#[tauri::command]
#[specta::specta]
async fn set_selected_model(
    app: tauri::AppHandle,
    whisper_model: State<'_, Arc<Mutex<WhisperModel>>>,
    model_id: String,
) -> Result<(), String> {
    if whisper::WhisperModelInfo::get_by_id(&model_id).is_none() {
        return Err(format!("Invalid model ID: {}", model_id));
    }
    
    let mut settings = AppSettings::get_or_default(&app);
    settings.selected_model = Some(model_id.clone());
    AppSettings::set(&app, &settings)?;
    
    if WhisperModel::is_downloaded(&model_id) {
        let mut model = whisper_model.lock().unwrap();
        model.load_model(Some(model_id))?;
    }
    
    Ok(())
}

#[tauri::command]
#[specta::specta]
fn get_model_path(app: tauri::AppHandle) -> Result<String, String> {
    let settings = AppSettings::get_or_default(&app);
    if let Some(model_id) = settings.selected_model {
        if let Some(model_info) = whisper::WhisperModelInfo::get_by_id(&model_id) {
            WhisperModel::get_model_path(&model_info.filename)
                .map(|p| p.to_string_lossy().to_string())
        } else {
            Err("Invalid model ID".to_string())
        }
    } else {
        Err("No model selected".to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let audio_manager = Arc::new(AudioManager::new());
    let fn_listener: Arc<std::sync::Mutex<Option<fn_key_listener::FnKeyListener>>> = Arc::new(std::sync::Mutex::new(None));
    let whisper_model = WhisperModel::new();
    
    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            get_audio_devices,
            set_recording_device,
            get_current_device,
            check_permissions,
            start_recording,
            stop_recording,
            stop_recording_chunked,
            stop_recording_manual,
            request_microphone_permission,
            request_accessibility_permission,
            refresh_permissions,
            get_word_count,
            update_word_count,
            get_recording_stats,
            get_hotkey,
            set_hotkey,
            validate_hotkey,
            insert_text_at_cursor,
            show_main_window,
            get_transcripts,
            get_transcript_stats,
            delete_transcript,
            clear_all_transcripts,
            is_fn_key_pressed,
            test_fn_key,
            check_model_downloaded,
            download_whisper_model,
            get_model_path,
            get_available_models,
            get_downloaded_models,
            get_selected_model,
            set_selected_model
        ])
        .events(collect_events![
            TranscriptionProgress,
            RecordingStateChanged,
            WordCountUpdated,
            HotkeyPressed,
            FnKeyStateChanged,
            AudioLevelUpdate,
            RecordingStatsUpdated,
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
        .manage(Arc::new(Mutex::new(whisper_model)))
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            println!("🚀 TalkType starting up...");
            
            let audio_manager = app.state::<Arc<AudioManager>>();
            let app_handle = app.handle().clone();
            tauri::async_runtime::block_on(async {
                audio_manager.set_app_handle(app_handle).await;
            });
            
            let settings = AppSettings::get_or_default(&app.handle());
            if let Some(model_id) = settings.selected_model {
                if WhisperModel::is_downloaded(&model_id) {
                    println!("🔄 Loading Whisper model: {}...", model_id);
                    let whisper_state = app.state::<Arc<Mutex<WhisperModel>>>();
                    let mut model = whisper_state.lock().unwrap();
                    match model.load_model(Some(model_id)) {
                        Ok(_) => println!("✅ Whisper model loaded successfully"),
                        Err(e) => println!("❌ Failed to load Whisper model: {}", e),
                    }
                } else {
                    println!("⚠️ Whisper model {} not downloaded yet", model_id);
                }
            } else {
                println!("⚠️ No Whisper model selected");
            }
            
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
                                let app_handle_for_recording = app_handle_fn.clone();
                                let bubble_show_handle_clone = bubble_show_handle.clone();
                                let handle = tauri::async_runtime::spawn(async move {
                                    let start_time = chrono::Utc::now().timestamp_millis();
                                    let mut settings = AppSettings::get_or_default(&app_handle_for_recording);
                                    settings.current_session_start = Some(start_time);
                                    if settings.first_recording_time.is_none() {
                                        settings.first_recording_time = Some(start_time);
                                    }
                                    let _ = AppSettings::set(&app_handle_for_recording, &settings);
                                    
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
                                let app_handle_clone = app_handle_fn.clone();
                                tauri::async_runtime::spawn(async move {
                                    match app_handle_clone.try_state::<Arc<AudioManager>>() {
                                        Some(audio_state) => {
                                            match app_handle_clone.try_state::<Arc<Mutex<WhisperModel>>>() {
                                                Some(whisper_state) => {
                                                    match stop_recording_chunked(app_handle_clone.clone(), audio_state, whisper_state).await {
                                                        Ok(text) => {
                                                            if !text.is_empty() {
                                                                let _ = insert_text_at_cursor(text);
                                                            }
                                                        }
                                                        Err(e) => {
                                                            eprintln!("Failed to transcribe: {}", e);
                                                        }
                                                    }
                                                }
                                                None => {
                                                    eprintln!("Failed to get whisper model state");
                                                }
                                            }
                                        }
                                        None => {
                                            eprintln!("Failed to get audio manager state");
                                        }
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
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
            }
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(move |app_handle, event| match event {
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => {
                println!("🔄 Dock icon clicked - reopening window");
                let _ = window::show_main_window(&app_handle);
            }
            _ => {}
        });
}