use reqwest::multipart::{Form, Part};
use rubato::{FftFixedInOut, Resampler};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

const STORE_KEY: &str = "groq_api_key";

#[derive(Deserialize, Serialize)]
struct GroqTranscriptionResponse {
    text: String,
}

pub async fn transcribe_with_groq(
    audio_data: &[f32],
    sample_rate: u32,
    language: Option<String>,
    api_key: &str,
) -> Result<String, String> {
    let mut samples: Vec<f32> = if sample_rate != 16_000 {
        let channels = 1;
        let chunk_size = 1024;
        let mut resampler =
            FftFixedInOut::<f32>::new(sample_rate as usize, 16_000usize, chunk_size, channels)
                .map_err(|e| e.to_string())?;
        let mut output = Vec::new();
        let mut input_pos = 0;
        while input_pos < audio_data.len() {
            let remaining = audio_data.len() - input_pos;
            let take = remaining.min(chunk_size);
            let mut in_buf = vec![vec![0.0f32; chunk_size]];
            in_buf[0][..take].copy_from_slice(&audio_data[input_pos..input_pos + take]);
            input_pos += take;
            let mut out_buf = resampler.output_buffer_allocate(true);
            resampler
                .process_into_buffer(&in_buf, &mut out_buf, None)
                .map_err(|e| e.to_string())?;
            output.extend_from_slice(&out_buf[0]);
        }
        output
    } else {
        audio_data.to_vec()
    };

    let mean = if !samples.is_empty() {
        samples.iter().copied().sum::<f32>() / samples.len() as f32
    } else {
        0.0
    };
    if mean.abs() > 1e-6 {
        for s in &mut samples {
            *s -= mean;
        }
    }
    let mut peak = 0.0f32;
    for s in &samples {
        let a = s.abs();
        if a > peak {
            peak = a;
        }
    }
    if peak > 0.0 && peak < 0.2 {
        let mut gain = 0.8 / peak;
        if gain > 4.0 {
            gain = 4.0;
        }
        for s in &mut samples {
            *s = (*s * gain).clamp(-1.0, 1.0);
        }
    }

    let mut cursor = Cursor::new(Vec::<u8>::new());
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec).map_err(|e| e.to_string())?;
        for s in &samples {
            let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer.write_sample(v).map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
    }
    let wav_bytes = cursor.into_inner();

    let file_part = Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;

    let mut form = Form::new()
        .text("model", "whisper-large-v3")
        .part("file", file_part)
        .text("response_format", "verbose_json");

    if let Some(lang) = language
        .as_ref()
        .filter(|v| !v.trim().is_empty() && !v.eq_ignore_ascii_case("auto"))
    {
        form = form.text("language", lang.clone());
    }

    let client = reqwest::Client::new();
    let res = client
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(format!("Groq API error: {}", res.status()));
    }

    let body = res
        .json::<GroqTranscriptionResponse>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(body.text)
}
