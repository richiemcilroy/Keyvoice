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
    sample_rate: Arc<Mutex<u32>>,
}

impl AudioManager {
    pub fn new() -> Self {
        Self {
            current_device: Arc::new(Mutex::new(None)),
            is_recording: Arc::new(AtomicBool::new(false)),
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
            current_stream: Arc::new(Mutex::new(AudioStream(None))),
            app_handle: Arc::new(Mutex::new(None)),
            sample_rate: Arc::new(Mutex::new(16000)),
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

        let default_config = device.default_input_config().map_err(|e| e.to_string())?;

        let preferred_config = cpal::StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(16_000),
            buffer_size: cpal::BufferSize::Default,
        };

        let audio_buffer_clone = self.audio_buffer.clone();
        let app_handle_clone = self.app_handle.lock().await.clone();

        let sample_format = default_config.sample_format();

        let mut build_for_config = |cfg: cpal::StreamConfig| -> Result<cpal::Stream, String> {
            match sample_format {
                cpal::SampleFormat::F32 => self.build_input_stream::<f32>(
                    &device,
                    cfg,
                    audio_buffer_clone.clone(),
                    app_handle_clone.clone(),
                ),
                cpal::SampleFormat::I16 => self.build_input_stream::<i16>(
                    &device,
                    cfg,
                    audio_buffer_clone.clone(),
                    app_handle_clone.clone(),
                ),
                cpal::SampleFormat::U16 => self.build_input_stream::<u16>(
                    &device,
                    cfg,
                    audio_buffer_clone.clone(),
                    app_handle_clone.clone(),
                ),
                _ => Err("Unsupported sample format".to_string()),
            }
        };

        let actual_sample_rate = match build_for_config(preferred_config.clone()) {
            Ok(_) => {
                println!("🎤 Using preferred config: 16 kHz mono");
                16_000
            }
            Err(_) => {
                let sample_rate = default_config.sample_rate().0;
                let channels = default_config.channels();
                println!(
                    "⚠️ Preferred 16 kHz unsupported – using device default ({} Hz, {}ch)",
                    sample_rate, channels
                );
                sample_rate
            }
        };

        *self.sample_rate.lock().await = actual_sample_rate;

        let mut current_stream = self.current_stream.lock().await;

        let stream = if actual_sample_rate == 16_000 {
            build_for_config(preferred_config)?
        } else {
            build_for_config(default_config.into())?
        };

        let is_recording = self.is_recording.clone();
        is_recording.store(true, Ordering::SeqCst);

        stream.play().map_err(|e| e.to_string())?;

        current_stream.0 = Some(stream);

        Ok(())
    }

    fn build_input_stream<T>(
        &self,
        device: &cpal::Device,
        config: cpal::StreamConfig,
        audio_buffer: Arc<Mutex<Vec<f32>>>,
        app_handle: Option<AppHandle>,
    ) -> Result<cpal::Stream, String>
    where
        T: cpal::Sample + cpal::SizedSample,
        f32: cpal::FromSample<T>,
    {
        let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

        let channels = config.channels as usize;

        let stream = device
            .build_input_stream(
                &config,
                move |data: &[T], _: &cpal::InputCallbackInfo| {
                    let mut buffer = audio_buffer.blocking_lock();

                    let mut sum = 0.0f32;
                    let frames = data.len() / channels;

                    for frame_idx in 0..frames {
                        let mut mono_sample = 0.0f32;
                        for ch in 0..channels {
                            let sample = data[frame_idx * channels + ch].to_sample::<f32>();
                            mono_sample += sample;
                        }
                        mono_sample /= channels as f32;
                        buffer.push(mono_sample);
                        sum += mono_sample * mono_sample;
                    }

                    let rms = (sum / frames as f32).sqrt();

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

    pub async fn stop_recording(&self) -> Result<(Vec<f32>, u32), String> {
        println!("⏹️ AudioManager: Stopping recording");

        if !self.is_recording.load(Ordering::SeqCst) {
            return Ok((vec![], 16000));
        }

        self.is_recording.store(false, Ordering::SeqCst);

        let mut current_stream = self.current_stream.lock().await;
        current_stream.0 = None;

        let buffer = self.audio_buffer.lock().await;
        let sample_rate = *self.sample_rate.lock().await;

        println!("📊 Recorded {} samples at {} Hz", buffer.len(), sample_rate);

        Ok((buffer.clone(), sample_rate))
    }
}
