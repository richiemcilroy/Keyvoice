use crate::AudioLevelUpdate;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::sync::Mutex;

struct AudioStream(Option<cpal::Stream>);

unsafe impl Send for AudioStream {}
unsafe impl Sync for AudioStream {}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct AudioDevice {
    pub name: String,
    pub id: String,
    pub is_default: bool,
}

#[derive(Clone)]
pub struct AudioManager {
    current_device: Arc<Mutex<Option<String>>>,
    is_recording: Arc<AtomicBool>,
    audio_buffer: Arc<Mutex<Vec<f32>>>,
    current_stream: Arc<Mutex<AudioStream>>,
    app_handle: Arc<Mutex<Option<AppHandle>>>,
}

impl AudioManager {
    pub fn new() -> Self {
        Self {
            current_device: Arc::new(Mutex::new(None)),
            is_recording: Arc::new(AtomicBool::new(false)),
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
            current_stream: Arc::new(Mutex::new(AudioStream(None))),
            app_handle: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn set_app_handle(&self, handle: AppHandle) {
        let mut app_handle = self.app_handle.lock().await;
        *app_handle = Some(handle);
    }

    pub async fn list_audio_devices() -> Result<Vec<AudioDevice>, String> {
        let host = cpal::default_host();
        let mut devices = Vec::new();

        let default_device = host.default_input_device();
        let default_name = default_device.as_ref().and_then(|d| d.name().ok());

        for device in host.input_devices().map_err(|e| e.to_string())? {
            if let Ok(name) = device.name() {
                devices.push(AudioDevice {
                    id: name.clone(),
                    name: name.clone(),
                    is_default: Some(&name) == default_name.as_ref(),
                });
            }
        }

        Ok(devices)
    }

    pub async fn set_current_device(&self, device_id: String) -> Result<(), String> {
        let mut current = self.current_device.lock().await;
        *current = Some(device_id);
        Ok(())
    }

    pub async fn get_current_device(&self) -> Option<String> {
        self.current_device.lock().await.clone()
    }

    pub async fn start_recording(&self) -> Result<(), String> {
        println!("🎙️ AudioManager: Starting recording");

        if self.is_recording.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.audio_buffer.lock().await.clear();

        let host = cpal::default_host();
        let device = if let Some(device_id) = self.current_device.lock().await.as_ref() {
            host.input_devices()
                .map_err(|e| e.to_string())?
                .find(|d| d.name().ok().as_ref() == Some(device_id))
                .ok_or_else(|| "Selected device not found".to_string())?
        } else {
            host.default_input_device()
                .ok_or_else(|| "No default input device available".to_string())?
        };

        let config = device.default_input_config().map_err(|e| e.to_string())?;

        let audio_buffer = self.audio_buffer.clone();
        let is_recording = self.is_recording.clone();
        let app_handle = self.app_handle.lock().await.clone();

        is_recording.store(true, Ordering::SeqCst);

        let mut current_stream = self.current_stream.lock().await;

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                self.build_input_stream::<f32>(&device, &config.into(), audio_buffer, app_handle)?
            }
            cpal::SampleFormat::I16 => {
                self.build_input_stream::<i16>(&device, &config.into(), audio_buffer, app_handle)?
            }
            cpal::SampleFormat::U16 => {
                self.build_input_stream::<u16>(&device, &config.into(), audio_buffer, app_handle)?
            }
            _ => return Err("Unsupported sample format".to_string()),
        };

        stream.play().map_err(|e| e.to_string())?;
        current_stream.0 = Some(stream);

        Ok(())
    }

    fn build_input_stream<T>(
        &self,
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        audio_buffer: Arc<Mutex<Vec<f32>>>,
        app_handle: Option<AppHandle>,
    ) -> Result<cpal::Stream, String>
    where
        T: cpal::Sample + cpal::SizedSample,
        f32: cpal::FromSample<T>,
    {
        let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

        let stream = device
            .build_input_stream(
                config,
                move |data: &[T], _: &cpal::InputCallbackInfo| {
                    let mut buffer = audio_buffer.blocking_lock();

                    let mut sum = 0.0f32;
                    for &sample in data {
                        let sample_f32 = sample.to_sample::<f32>();
                        buffer.push(sample_f32);
                        sum += sample_f32 * sample_f32;
                    }

                    let rms = (sum / data.len() as f32).sqrt();

                    if let Some(ref handle) = app_handle {
                        AudioLevelUpdate { level: rms }.emit(handle).ok();
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| e.to_string())?;

        Ok(stream)
    }

    pub async fn stop_recording(&self) -> Result<Vec<f32>, String> {
        println!("⏹️ AudioManager: Stopping recording");

        if !self.is_recording.load(Ordering::SeqCst) {
            return Ok(vec![]);
        }

        self.is_recording.store(false, Ordering::SeqCst);

        let mut current_stream = self.current_stream.lock().await;
        current_stream.0 = None;

        let buffer = self.audio_buffer.lock().await;
        Ok(buffer.clone())
    }
}
