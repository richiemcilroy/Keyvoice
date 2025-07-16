use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct Transcript {
    pub id: String,
    pub text: String,
    pub timestamp: f64, // Unix timestamp in milliseconds
    pub duration_ms: f64,
    pub word_count: u32,
    pub wpm: f32,
    pub model_used: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type)]
pub struct TranscriptStore {
    pub transcripts: Vec<Transcript>,
}

impl TranscriptStore {
    pub fn get_store_path(app: &AppHandle) -> Result<PathBuf, String> {
        let app_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to get app data dir: {}", e))?;
        
        if !app_dir.exists() {
            std::fs::create_dir_all(&app_dir)
                .map_err(|e| format!("Failed to create app data dir: {}", e))?;
        }
        
        Ok(app_dir.join("transcripts.json"))
    }

    pub fn load(app: &AppHandle) -> Result<Self, String> {
        let path = Self::get_store_path(app)?;
        
        if !path.exists() {
            return Ok(Self::default());
        }
        
        let data = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read transcripts file: {}", e))?;
        
        serde_json::from_str(&data)
            .map_err(|e| format!("Failed to parse transcripts: {}", e))
    }

    pub fn save(&self, app: &AppHandle) -> Result<(), String> {
        let path = Self::get_store_path(app)?;
        
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize transcripts: {}", e))?;
        
        std::fs::write(&path, data)
            .map_err(|e| format!("Failed to write transcripts file: {}", e))?;
        
        Ok(())
    }

    pub fn add_transcript(&mut self, transcript: Transcript) {
        self.transcripts.insert(0, transcript);
        
        // Keep only the last 1000 transcripts to prevent unbounded growth
        if self.transcripts.len() > 1000 {
            self.transcripts.truncate(1000);
        }
    }

    pub fn get_transcripts(&self, limit: Option<u32>) -> Vec<Transcript> {
        match limit {
            Some(n) => self.transcripts.iter().take(n as usize).cloned().collect(),
            None => self.transcripts.clone(),
        }
    }

    pub fn get_transcript_by_id(&self, id: &str) -> Option<&Transcript> {
        self.transcripts.iter().find(|t| t.id == id)
    }

    pub fn delete_transcript(&mut self, id: &str) -> Result<(), String> {
        let initial_len = self.transcripts.len();
        self.transcripts.retain(|t| t.id != id);
        
        if self.transcripts.len() == initial_len {
            Err("Transcript not found".to_string())
        } else {
            Ok(())
        }
    }

    pub fn clear_all(&mut self) {
        self.transcripts.clear();
    }

    pub fn calculate_stats(&self) -> TranscriptStats {
        let total_words: u32 = self.transcripts.iter().map(|t| t.word_count).sum();
        let total_time_ms: f64 = self.transcripts.iter().map(|t| t.duration_ms).sum();
        let total_characters: u32 = self.transcripts.iter().map(|t| t.text.len() as u32).sum();
        
        // Only include transcripts with more than 10 words for WPM calculation
        let wpm_eligible_transcripts: Vec<&Transcript> = self.transcripts
            .iter()
            .filter(|t| t.word_count > 10)
            .collect();
        
        let wpm_total_words: u32 = wpm_eligible_transcripts.iter().map(|t| t.word_count).sum();
        let wpm_total_time_ms: f64 = wpm_eligible_transcripts.iter().map(|t| t.duration_ms).sum();
        
        let overall_wpm = if wpm_total_time_ms > 0.0 {
            (wpm_total_words as f64 / (wpm_total_time_ms / 60000.0)) as f32
        } else {
            0.0
        };
        
        TranscriptStats {
            total_words,
            total_time_ms,
            total_characters,
            overall_wpm,
            transcript_count: self.transcripts.len() as u32,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct TranscriptStats {
    pub total_words: u32,
    pub total_time_ms: f64,
    pub total_characters: u32,
    pub overall_wpm: f32,
    pub transcript_count: u32,
}