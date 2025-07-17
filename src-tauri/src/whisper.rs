use futures_util::StreamExt;
use once_cell::sync::Lazy;
use rubato::{FftFixedInOut, Resampler};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::AppHandle;
use tauri_specta::Event;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const WHISPER_SAMPLE_RATE: u32 = 16_000;

static RESAMPLER_CACHE: Lazy<Mutex<HashMap<(u32, u32, usize), Arc<Mutex<FftFixedInOut<f32>>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

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

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
pub struct TranscriptionProgress {
    pub text: String,
    pub is_final: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct WhisperModelInfo {
    pub id: String,
    pub name: String,
    pub size_mb: u32,
    pub description: String,
    pub url: String,
    pub filename: String,
    pub recommended_for: Vec<String>,
}

impl WhisperModelInfo {
    pub fn all() -> Vec<Self> {
        vec![
            Self {
                id: "large-v3-turbo-q8_0".to_string(),
                name: "Large v3 Turbo Q8".to_string(),
                size_mb: 809,
                description: "Best quality and performance".to_string(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q8_0.bin".to_string(),
                filename: "ggml-large-v3-turbo-q8_0.bin".to_string(),
                recommended_for: vec!["accuracy".to_string(), "performance".to_string()],
            },
            Self {
                id: "large-v3-turbo-q5_0".to_string(),
                name: "Large v3 Turbo Q5".to_string(),
                size_mb: 540,
                description: "Good quality for slower machines".to_string(),
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin".to_string(),
                filename: "ggml-large-v3-turbo-q5_0.bin".to_string(),
                recommended_for: vec!["slower_machines".to_string()],
            },
            Self {
                id: "distil-large-v3.5-q8_0".to_string(),
                name: "Distil-Large v3.5 Q8".to_string(),
                size_mb: 1520,
                description: "4-6× faster than Large-v3 with near-equal accuracy".to_string(),
                url: "https://huggingface.co/distil-whisper/distil-large-v3.5-ggml/resolve/main/ggml-model.bin".to_string(),
                filename: "ggml-model.bin".to_string(),
                recommended_for: vec!["accuracy".to_string(), "speed".to_string()],
            },
        ]
    }

    pub fn get_by_id(id: &str) -> Option<Self> {
        Self::all().into_iter().find(|m| m.id == id)
    }
}

pub struct WhisperModel {
    context: Option<Arc<WhisperContext>>,
    current_model_id: Option<String>,
}

impl Default for WhisperModel {
    fn default() -> Self {
        Self::new()
    }
}

impl WhisperModel {
    pub fn new() -> Self {
        Self {
            context: None,
            current_model_id: None,
        }
    }

    pub fn load_model(&mut self, model_id: Option<String>) -> Result<(), String> {
        let model_id = model_id
            .or_else(|| self.current_model_id.clone())
            .ok_or_else(|| "No model specified".to_string())?;

        let model_info = WhisperModelInfo::get_by_id(&model_id)
            .ok_or_else(|| format!("Model not found: {}", model_id))?;

        let model_path = Self::get_model_path(&model_info.filename)?;

        if !model_path.exists() {
            return Err("Model not downloaded".to_string());
        }

        #[cfg(target_os = "macos")]
        {
            let possible_paths = vec![
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|p| p.join("../Resources"))),
                Some(std::env::current_dir().unwrap()),
                Some(std::env::current_dir().unwrap().join("src-tauri")),
                Some(std::env::current_dir().unwrap().join("src-tauri/src-tauri")),
                Some(std::env::current_dir().unwrap().join("target/debug/build")
                    .read_dir()
                    .ok()
                    .and_then(|entries| {
                        entries
                            .filter_map(|e| e.ok())
                            .find(|e| e.file_name().to_string_lossy().starts_with("whisper-rs-sys-"))
                            .map(|e| e.path().join("out/build/bin"))
                    })
                    .unwrap_or_default()),
            ];

            for path_opt in possible_paths {
                if let Some(path) = path_opt {
                    let metal_file = path.join("ggml-metal.metal");
                    if metal_file.exists() {
                        std::env::set_var(
                            "GGML_METAL_PATH_RESOURCES",
                            path.to_string_lossy().to_string(),
                        );
                        println!("🎨 Set Metal resources path to: {}", path.display());
                        break;
                    }
                }
            }
        }

        let mut params = WhisperContextParameters::default();
        params.use_gpu = true;

        let ctx = WhisperContext::new_with_params(model_path.to_str().unwrap(), params)
            .map_err(|e| format!("Failed to load model: {:?}", e))?;

        self.context = Some(Arc::new(ctx));
        self.current_model_id = Some(model_id);

        println!("✅ Model loaded successfully");
        Ok(())
    }

    pub fn transcribe(&self, audio_data: &[f32], sample_rate: u32) -> Result<String, String> {
        let start_time = std::time::Instant::now();

        let context = self
            .context
            .as_ref()
            .ok_or_else(|| "Model not loaded".to_string())?;

        let resample_start = std::time::Instant::now();
        let resampled_audio = if sample_rate != WHISPER_SAMPLE_RATE {
            println!("🔄 Resampling from {} Hz to 16000 Hz", sample_rate);
            let result = Self::resample_audio(audio_data, sample_rate, WHISPER_SAMPLE_RATE)?;
            println!("⏱️ Resampling took: {:?}", resample_start.elapsed());
            result
        } else {
            audio_data.to_vec()
        };

        println!("🎯 Transcribing {} samples", resampled_audio.len());

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        params.set_suppress_non_speech_tokens(true);

        params.set_temperature_inc(0.0);
        params.set_temperature(0.0);

        params.set_single_segment(true);
        params.set_no_timestamps(true);

        params.set_max_initial_ts(0.0);
        params.set_max_len(0);
        params.set_split_on_word(false);
        
        params.set_token_timestamps(false);
        params.set_n_max_text_ctx(16384);

        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4) as i32;
        println!("📦 Using {} threads for transcription", num_threads);
        params.set_n_threads(num_threads);
        params.set_language(Some("en"));
        
        params.set_no_context(true);

        let mut state = context
            .create_state()
            .map_err(|e| format!("Failed to create state: {:?}", e))?;

        let process_start = std::time::Instant::now();
        state
            .full(params, &resampled_audio)
            .map_err(|e| format!("Failed to transcribe: {:?}", e))?;
        println!("⏱️ Whisper processing took: {:?}", process_start.elapsed());

        let extract_start = std::time::Instant::now();
        let num_segments = state
            .full_n_segments()
            .map_err(|e| format!("Failed to get segments: {:?}", e))?;
        let mut text = String::new();

        for i in 0..num_segments {
            let segment = state
                .full_get_segment_text(i)
                .map_err(|e| format!("Failed to get segment text: {:?}", e))?;
            text.push_str(&segment);
        }
        println!("⏱️ Extracting text took: {:?}", extract_start.elapsed());

        println!("📝 Transcribed text: {:?}", text.trim());
        println!("⏱️ Total transcribe() took: {:?}", start_time.elapsed());

        Ok(text.trim().to_string())
    }

    fn resample_audio(input: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>, String> {
        let channels = 1;

        let ratio = from_rate as f64 / to_rate as f64;
        let chunk_size = if ratio > 1.0 {
            let base_chunk = 1024;
            let adjusted_chunk =
                ((base_chunk as f64 / ratio).round() as usize * ratio as usize).max(64);
            adjusted_chunk
        } else {
            512
        };

        let key = (from_rate, to_rate, chunk_size);
        let mut cache = RESAMPLER_CACHE.lock().unwrap();
        let resampler_arc = cache
            .entry(key)
            .or_insert_with(|| {
                let r = FftFixedInOut::<f32>::new(
                    from_rate as usize,
                    to_rate as usize,
                    chunk_size,
                    channels,
                )
                .expect("Failed to create resampler");
                Arc::new(Mutex::new(r))
            })
            .clone();
        drop(cache);

        let mut resampler = resampler_arc.lock().unwrap();

        let mut output = Vec::new();
        let mut input_pos = 0;

        while input_pos < input.len() {
            let remaining = input.len() - input_pos;
            let chunk_len = remaining.min(chunk_size);

            if chunk_len < chunk_size {
                let mut padded = vec![0.0f32; chunk_size];
                padded[..chunk_len].copy_from_slice(&input[input_pos..input_pos + chunk_len]);

                let input_buffer = vec![padded];
                let mut output_buffer = resampler.output_buffer_allocate(true);

                resampler
                    .process_into_buffer(&input_buffer, &mut output_buffer, None)
                    .map_err(|e| format!("Failed to resample final chunk: {:?}", e))?;

                let output_len = (chunk_len as f64 * to_rate as f64 / from_rate as f64) as usize;
                let actual_output_len = output_len.min(output_buffer[0].len());
                output.extend_from_slice(&output_buffer[0][..actual_output_len]);
                break;
            } else {
                let chunk = &input[input_pos..input_pos + chunk_size];
                let input_buffer = vec![chunk.to_vec()];
                let mut output_buffer = resampler.output_buffer_allocate(true);

                resampler
                    .process_into_buffer(&input_buffer, &mut output_buffer, None)
                    .map_err(|e| format!("Failed to resample chunk: {:?}", e))?;

                output.extend_from_slice(&output_buffer[0]);
                input_pos += chunk_size;
            }
        }

        Ok(output)
    }

    pub fn get_model_dir() -> Result<PathBuf, String> {
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| "Failed to get local data directory".to_string())?;

        let model_dir = data_dir.join("com.talktype.desktop").join("models");

        fs::create_dir_all(&model_dir)
            .map_err(|e| format!("Failed to create model directory: {}", e))?;

        Ok(model_dir)
    }

    pub fn get_model_path(filename: &str) -> Result<PathBuf, String> {
        let model_dir = Self::get_model_dir()?;
        Ok(model_dir.join(filename))
    }

    pub fn get_current_model_id(&self) -> Option<String> {
        self.current_model_id.clone()
    }

    pub fn is_downloaded(model_id: &str) -> bool {
        if let Some(model_info) = WhisperModelInfo::get_by_id(model_id) {
            if let Ok(model_path) = Self::get_model_path(&model_info.filename) {
                return model_path.exists() && model_path.is_file();
            }
        }
        false
    }

    pub fn get_downloaded_models() -> Vec<String> {
        WhisperModelInfo::all()
            .into_iter()
            .filter(|m| Self::is_downloaded(&m.id))
            .map(|m| m.id)
            .collect()
    }

    pub async fn download<R: tauri::Runtime>(
        app_handle: &AppHandle<R>,
        model_id: &str,
    ) -> Result<(), String> {
        let model_info = WhisperModelInfo::get_by_id(model_id)
            .ok_or_else(|| format!("Model not found: {}", model_id))?;

        let model_path = Self::get_model_path(&model_info.filename)?;

        if model_path.exists() {
            ModelDownloadComplete {
                success: true,
                error: None,
            }
            .emit(app_handle)
            .ok();
            return Ok(());
        }

        let client = reqwest::Client::new();

        let response = client
            .get(&model_info.url)
            .send()
            .await
            .map_err(|e| format!("Failed to start download: {}", e))?;

        let total_size = response
            .content_length()
            .ok_or_else(|| "Failed to get content length".to_string())?;

        let temp_path = model_path.with_extension("tmp");
        let mut file =
            fs::File::create(&temp_path).map_err(|e| format!("Failed to create file: {}", e))?;

        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    file.write_all(&chunk)
                        .map_err(|e| format!("Failed to write chunk: {}", e))?;

                    downloaded += chunk.len() as u64;
                    let progress = (downloaded as f64 / total_size as f64) * 100.0;

                    ModelDownloadProgress {
                        progress,
                        downloaded_bytes: downloaded as f64,
                        total_bytes: total_size as f64,
                    }
                    .emit(app_handle)
                    .ok();
                }
                Err(e) => {
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

        file.sync_all()
            .map_err(|e| format!("Failed to sync file: {}", e))?;
        drop(file);

        fs::rename(&temp_path, &model_path).map_err(|e| format!("Failed to rename file: {}", e))?;

        ModelDownloadComplete {
            success: true,
            error: None,
        }
        .emit(app_handle)
        .ok();

        Ok(())
    }

    pub fn transcribe_chunked<F>(
        &self,
        audio_data: &[f32],
        sample_rate: u32,
        chunk_duration_secs: f32,
        mut on_chunk: F,
    ) -> Result<String, String>
    where
        F: FnMut(&str, bool),
    {
        let context = self
            .context
            .as_ref()
            .ok_or_else(|| "Model not loaded".to_string())?;

        let resampled_audio = if sample_rate != WHISPER_SAMPLE_RATE {
            println!(
                "🔄 Resampling from {} Hz to 16000 Hz for chunked processing",
                sample_rate
            );
            Self::resample_audio(audio_data, sample_rate, WHISPER_SAMPLE_RATE)?
        } else {
            audio_data.to_vec()
        };

        let chunk_samples = (WHISPER_SAMPLE_RATE as f32 * chunk_duration_secs) as usize;
        let total_chunks = (resampled_audio.len() + chunk_samples - 1) / chunk_samples;
        let mut full_text = String::new();

        println!(
            "🔀 Processing {} chunks of {:.1}s each",
            total_chunks, chunk_duration_secs
        );

        for (chunk_idx, chunk) in resampled_audio.chunks(chunk_samples).enumerate() {
            let chunk_start = std::time::Instant::now();

            let padded_chunk = if chunk.len() < chunk_samples {
                let mut padded = vec![0.0f32; chunk_samples];
                padded[..chunk.len()].copy_from_slice(chunk);
                padded
            } else {
                chunk.to_vec()
            };

            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_suppress_blank(true);
            params.set_suppress_non_speech_tokens(true);

            params.set_temperature_inc(0.0);
            params.set_temperature(0.0);

            params.set_single_segment(true);
            params.set_no_timestamps(true);

            params.set_max_initial_ts(0.0);
            params.set_max_len(0);
            params.set_split_on_word(false);

            let num_threads = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4) as i32;
            params.set_n_threads(num_threads);

            let mut state = context
                .create_state()
                .map_err(|e| format!("Failed to create state: {:?}", e))?;

            state
                .full(params, &padded_chunk)
                .map_err(|e| format!("Failed to transcribe chunk: {:?}", e))?;

            let num_segments = state
                .full_n_segments()
                .map_err(|e| format!("Failed to get segments: {:?}", e))?;

            let mut chunk_text = String::new();
            for i in 0..num_segments {
                let segment = state
                    .full_get_segment_text(i)
                    .map_err(|e| format!("Failed to get segment text: {:?}", e))?;
                chunk_text.push_str(&segment);
            }

            let chunk_text = chunk_text.trim();
            if !chunk_text.is_empty() {
                if !full_text.is_empty() {
                    full_text.push(' ');
                }
                full_text.push_str(chunk_text);

                let is_final = chunk_idx == total_chunks - 1;
                on_chunk(&full_text, is_final);
            }

            println!(
                "⏱️ Chunk {} took: {:?}",
                chunk_idx + 1,
                chunk_start.elapsed()
            );
        }

        Ok(full_text)
    }
}
