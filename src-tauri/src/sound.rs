use rodio::{Decoder, OutputStream, Source};
use std::fs::File;
use std::io::BufReader;
use tauri::{AppHandle, Manager};

pub fn play_start_sound(app_handle: &AppHandle) {
    println!("🎵 play_start_sound called");
    let app_handle = app_handle.clone();

    std::thread::spawn(move || {
        if let Err(e) = play_sound_file(&app_handle, "start.mp3") {
            eprintln!("❌ Failed to play start sound: {}", e);
        }
    });
}

pub fn play_complete_sound(app_handle: &AppHandle) {
    println!("🎵 play_complete_sound called");
    let app_handle = app_handle.clone();

    std::thread::spawn(move || {
        if let Err(e) = play_sound_file(&app_handle, "complete.mp3") {
            eprintln!("❌ Failed to play complete sound: {}", e);
        }
    });
}

fn play_sound_file(
    app_handle: &AppHandle,
    sound_file: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let dev_path = std::env::current_dir()
        .unwrap_or_default()
        .join("src-tauri")
        .join("sounds")
        .join(sound_file);

    let sound_path = if dev_path.exists() {
        println!("🔊 Using development sound path: {:?}", dev_path);
        dev_path
    } else {
        let prod_path = app_handle
            .path()
            .resource_dir()
            .map_err(|e| format!("Failed to get resource dir: {}", e))?
            .join("sounds")
            .join(sound_file);
        println!("🔊 Using production sound path: {:?}", prod_path);
        prod_path
    };

    if !sound_path.exists() {
        return Err(format!("Sound file not found: {:?}", sound_path).into());
    }

    let (_stream, stream_handle) = OutputStream::try_default()
        .map_err(|e| format!("Failed to create audio output stream: {}", e))?;

    let file = File::open(&sound_path)?;
    let source = Decoder::new(BufReader::new(file))
        .map_err(|e| format!("Failed to decode audio file: {}", e))?;

    if let Err(e) = stream_handle.play_raw(source.convert_samples()) {
        return Err(format!("Failed to play sound: {}", e).into());
    }

    std::thread::sleep(std::time::Duration::from_millis(500));

    println!("✅ Sound playback completed: {}", sound_file);
    Ok(())
}
