use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Manager, Runtime, Emitter,
    image::Image,
};
use crate::audio::{AudioManager};
use crate::permissions::{Permissions, PermissionState};
use crate::AppSettings;
use std::sync::Arc;

pub fn create_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let devices = std::thread::spawn(|| {
        tauri::async_runtime::block_on(async {
            AudioManager::list_audio_devices().await.unwrap_or_default()
        })
    }).join().unwrap_or_default();
    
    let settings = AppSettings::get_or_default(app);
    let audio_manager = app.state::<Arc<AudioManager>>();
    let current_device = if let Some(device_id) = settings.selected_microphone {
        Some(device_id)
    } else {
        let audio_manager_clone = audio_manager.inner().clone();
        std::thread::spawn(move || {
            tauri::async_runtime::block_on(async {
                audio_manager_clone.get_current_device().await
            })
        }).join().unwrap_or(None)
    };
    
    let open_main_window = MenuItem::with_id(app, "open_main_window", "Open Main Window", true, None::<&str>)?;
    
    let mut mic_items = vec![];
    if devices.is_empty() {
        let no_devices = MenuItem::with_id(app, "no_devices", "No devices found", false, None::<&str>)?;
        mic_items.push(no_devices);
    } else {
        for device in &devices {
            let label = if current_device.as_ref() == Some(&device.id) {
                format!("✓ {}", device.name)
            } else {
                device.name.clone()
            };
            mic_items.push(MenuItem::with_id(app, &format!("mic_{}", device.id), label, true, None::<&str>)?);
        }
    }
    
    let mic_menu_items: Vec<&dyn tauri::menu::IsMenuItem<R>> = mic_items.iter().map(|item| item as &dyn tauri::menu::IsMenuItem<R>).collect();
    let select_microphone = Submenu::with_id_and_items(
        app,
        "select_microphone",
        "Select Microphone",
        true,
        &mic_menu_items,
    )?;
    
    let permissions = Permissions::check();
    
    let enable_accessibility = if !matches!(permissions.accessibility.state, PermissionState::Granted) {
        Some(MenuItem::with_id(app, "enable_accessibility", "Enable Accessibility", true, None::<&str>)?)
    } else {
        None
    };
    
    let enable_microphone = if !matches!(permissions.microphone.state, PermissionState::Granted) {
        Some(MenuItem::with_id(app, "enable_microphone", "Enable Microphone", true, None::<&str>)?)
    } else {
        None
    };
    
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    
    let sep1 = if enable_accessibility.is_some() || enable_microphone.is_some() {
        Some(PredefinedMenuItem::separator(app)?)
    } else {
        None
    };
    let sep2 = PredefinedMenuItem::separator(app)?;
    
    let mut menu_items: Vec<&dyn tauri::menu::IsMenuItem<R>> = vec![&open_main_window, &select_microphone];
    
    if let Some(ref sep) = sep1 {
        menu_items.push(sep);
        
        if let Some(ref item) = enable_accessibility {
            menu_items.push(item);
        }
        
        if let Some(ref item) = enable_microphone {
            menu_items.push(item);
        }
    }
    
    menu_items.push(&sep2);
    menu_items.push(&quit);
    
    let menu = Menu::with_items(app, &menu_items)?;
    
    let tray_icon = if let Ok(icon_path) = app.path().resolve("icons/tray-icon.png", tauri::path::BaseDirectory::Resource) {
        if let Ok(icon_bytes) = std::fs::read(&icon_path) {
            if let Ok(rgba) = image::load_from_memory(&icon_bytes) {
                let rgba_data = rgba.to_rgba8();
                let (width, height) = rgba_data.dimensions();
                let raw_data = rgba_data.into_raw();
                Some(Image::new_owned(raw_data, width, height))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    
    let _ = TrayIconBuilder::with_id("main")
        .icon(tray_icon.unwrap_or_else(|| app.default_window_icon().unwrap().clone()))
        .icon_as_template(false)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app: &AppHandle<R>, event| {
            let event_id = event.id.as_ref();
            
            match event_id {
                "open_main_window" => {
                    let _ = app.emit("show-main-window", ());
                }
                "enable_accessibility" => {
                    #[cfg(target_os = "macos")]
                    {
                        let _ = std::process::Command::new("open")
                            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                            .spawn();
                    }
                }
                "enable_microphone" => {
                    #[cfg(target_os = "macos")]
                    {
                        let _ = std::process::Command::new("open")
                            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
                            .spawn();
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                id if id.starts_with("mic_") => {
                    let device_id = id.strip_prefix("mic_").unwrap().to_string();
                    let audio_manager = app.state::<Arc<AudioManager>>();
                    let audio_manager_clone = audio_manager.inner().clone();
                    let app_handle = app.clone();
                    
                    tauri::async_runtime::spawn(async move {
                        let _ = audio_manager_clone.set_current_device(device_id.clone()).await;
                        
                        let mut settings = AppSettings::get_or_default(&app_handle);
                        settings.selected_microphone = Some(device_id);
                        let _ = AppSettings::set(&app_handle, &settings);
                        
                        let _ = update_tray_menu(&app_handle);
                    });
                }
                _ => {}
            }
        })
        .build(app)?;
    
    Ok(())
}

pub fn update_tray_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id("main") {
        let devices = std::thread::spawn(|| {
            tauri::async_runtime::block_on(async {
                AudioManager::list_audio_devices().await.unwrap_or_default()
            })
        }).join().unwrap_or_default();
        
        let settings = AppSettings::get_or_default(app);
        let audio_manager = app.state::<Arc<AudioManager>>();
        let current_device = if let Some(device_id) = settings.selected_microphone {
            Some(device_id)
        } else {
            let audio_manager_clone = audio_manager.inner().clone();
            std::thread::spawn(move || {
                tauri::async_runtime::block_on(async {
                    audio_manager_clone.get_current_device().await
                })
            }).join().unwrap_or(None)
        };
        
        let open_main_window = MenuItem::with_id(app, "open_main_window", "Open Main Window", true, None::<&str>)?;
        
        let mut mic_items = vec![];
        if devices.is_empty() {
            let no_devices = MenuItem::with_id(app, "no_devices", "No devices found", false, None::<&str>)?;
            mic_items.push(no_devices);
        } else {
            for device in &devices {
                let label = if current_device.as_ref() == Some(&device.id) {
                    format!("✓ {}", device.name)
                } else {
                    device.name.clone()
                };
                mic_items.push(MenuItem::with_id(app, &format!("mic_{}", device.id), label, true, None::<&str>)?);
            }
        }
        
        let mic_menu_items: Vec<&dyn tauri::menu::IsMenuItem<R>> = mic_items.iter().map(|item| item as &dyn tauri::menu::IsMenuItem<R>).collect();
        let select_microphone = Submenu::with_id_and_items(
            app,
            "select_microphone",
            "Select Microphone",
            true,
            &mic_menu_items,
        )?;
        
        let permissions = Permissions::check();
        
        let enable_accessibility = if !matches!(permissions.accessibility.state, PermissionState::Granted) {
            Some(MenuItem::with_id(app, "enable_accessibility", "Enable Accessibility", true, None::<&str>)?)
        } else {
            None
        };
        
        let enable_microphone = if !matches!(permissions.microphone.state, PermissionState::Granted) {
            Some(MenuItem::with_id(app, "enable_microphone", "Enable Microphone", true, None::<&str>)?)
        } else {
            None
        };
        
        let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
        
        let sep1 = if enable_accessibility.is_some() || enable_microphone.is_some() {
            Some(PredefinedMenuItem::separator(app)?)
        } else {
            None
        };
        let sep2 = PredefinedMenuItem::separator(app)?;
        
        let mut menu_items: Vec<&dyn tauri::menu::IsMenuItem<R>> = vec![&open_main_window, &select_microphone];
        
        if let Some(ref sep) = sep1 {
            menu_items.push(sep);
            
            if let Some(ref item) = enable_accessibility {
                menu_items.push(item);
            }
            
            if let Some(ref item) = enable_microphone {
                menu_items.push(item);
            }
        }
        
        menu_items.push(&sep2);
        menu_items.push(&quit);
        
        let menu = Menu::with_items(app, &menu_items)?;
        tray.set_menu(Some(menu))?;
    }
    
    Ok(())
}