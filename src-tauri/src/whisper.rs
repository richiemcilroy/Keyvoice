use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_specta::Event;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct ModelDownloadProgress {
    pub progress: f64,
    pub downloaded_bytes: f64,
    pub total_bytes: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct ModelDownloadComplete {
    pub success: bool,
    pub error: Option<String>,
}

pub struct WhisperModel;

impl WhisperModel {
    // Whisper model URL - using the base model for now
    // You can change this to other models like tiny, small, medium, large
    const MODEL_URL: &'static str =
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";
    const MODEL_FILENAME: &'static str = "whisper-base.bin";

    pub fn get_model_dir() -> Result<PathBuf, String> {
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| "Failed to get local data directory".to_string())?;

        let model_dir = data_dir.join("com.talktype.desktop").join("models");

        // Create directory if it doesn't exist
        fs::create_dir_all(&model_dir)
            .map_err(|e| format!("Failed to create model directory: {}", e))?;

        Ok(model_dir)
    }

    pub fn get_model_path() -> Result<PathBuf, String> {
        let model_dir = Self::get_model_dir()?;
        Ok(model_dir.join(Self::MODEL_FILENAME))
    }

    pub fn is_downloaded() -> bool {
        if let Ok(model_path) = Self::get_model_path() {
            model_path.exists() && model_path.is_file()
        } else {
            false
        }
    }

    pub async fn download<R: tauri::Runtime>(app_handle: &AppHandle<R>) -> Result<(), String> {
        let model_path = Self::get_model_path()?;

        // If model already exists, skip download
        if model_path.exists() {
            ModelDownloadComplete {
                success: true,
                error: None,
            }
            .emit(app_handle)
            .ok();
            return Ok(());
        }

        // Create a client
        let client = reqwest::Client::new();

        // Start the download
        let response = client
            .get(Self::MODEL_URL)
            .send()
            .await
            .map_err(|e| format!("Failed to start download: {}", e))?;

        // Get the content length
        let total_size = response
            .content_length()
            .ok_or_else(|| "Failed to get content length".to_string())?;

        // Create a temporary file
        let temp_path = model_path.with_extension("tmp");
        let mut file =
            fs::File::create(&temp_path).map_err(|e| format!("Failed to create file: {}", e))?;

        // Download with progress
        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    file.write_all(&chunk)
                        .map_err(|e| format!("Failed to write chunk: {}", e))?;

                    downloaded += chunk.len() as u64;
                    let progress = (downloaded as f64 / total_size as f64) * 100.0;

                    // Emit progress event
                    ModelDownloadProgress {
                        progress,
                        downloaded_bytes: downloaded as f64,
                        total_bytes: total_size as f64,
                    }
                    .emit(app_handle)
                    .ok();
                }
                Err(e) => {
                    // Clean up temp file on error
                    let _ = fs::remove_file(&temp_path);

                    ModelDownloadComplete {
                        success: false,
                        error: Some(format!("Download failed: {}", e)),
                    }
                    .emit(app_handle)
                    .ok();

                    return Err(format!("Download failed: {}", e));
                }
            }
        }

        // Ensure all data is written
        file.sync_all()
            .map_err(|e| format!("Failed to sync file: {}", e))?;
        drop(file);

        // Rename temp file to final name
        fs::rename(&temp_path, &model_path).map_err(|e| format!("Failed to rename file: {}", e))?;

        // Emit completion event
        ModelDownloadComplete {
            success: true,
            error: None,
        }
        .emit(app_handle)
        .ok();

        Ok(())
    }
}
