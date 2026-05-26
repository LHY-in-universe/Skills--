//! Rust 内进程语音管线。
//!
//! 取代 `webapp/backend/voice_worker.py` 子进程：
//! - KWS 唤醒词（zipformer keyword spotter，流式）
//! - Silero VAD 端点检测（流式 segment 队列）
//! - Paraformer ASR（离线一段一段解码）
//!
//! 调用入口：`push_audio_chunk` 与 `flush_pending_asr`，返回事件列表
//! 由调用方（`handlers/voice.rs`）串行翻译成 WebSocket 消息。

use anyhow::{anyhow, Result};
use sherpa_rs::paraformer::{ParaformerConfig, ParaformerRecognizer};
use sherpa_rs::sherpa_rs_sys as sys;
use sherpa_rs::silero_vad::{SileroVad, SileroVadConfig};
use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};
use std::ptr::null;

#[derive(Debug, Clone)]
pub enum PipelineEvent {
    Wakeword(String),
    AsrResult(String),
}

pub struct VoicePipeline {
    kws_spotter: *const sys::SherpaOnnxKeywordSpotter,
    kws_stream: *const sys::SherpaOnnxOnlineStream,
    vad: SileroVad,
    recognizer: ParaformerRecognizer,
    bypass_wakeword: bool,
    is_awakened: bool,
    is_speaking: bool,
}

unsafe impl Send for VoicePipeline {}

impl Drop for VoicePipeline {
    fn drop(&mut self) {
        unsafe {
            if !self.kws_stream.is_null() {
                sys::SherpaOnnxDestroyOnlineStream(self.kws_stream);
            }
            if !self.kws_spotter.is_null() {
                sys::SherpaOnnxDestroyKeywordSpotter(self.kws_spotter);
            }
        }
    }
}

impl VoicePipeline {
    pub fn new(models_root: PathBuf) -> Result<Self> {
        let asr_dir = models_root.join("sherpa-onnx-paraformer-zh-2023-09-14");
        let kws_dir = models_root.join("sherpa-onnx-kws-zipformer-wenetspeech-3.3M-2024-01-01");
        let vad_path = models_root.join("silero_vad.onnx");
        let keywords_path = models_root.join("keywords.txt");

        for p in [
            asr_dir.join("model.int8.onnx"),
            asr_dir.join("tokens.txt"),
            kws_dir.join("encoder-epoch-12-avg-2-chunk-16-left-64.onnx"),
            kws_dir.join("decoder-epoch-12-avg-2-chunk-16-left-64.onnx"),
            kws_dir.join("joiner-epoch-12-avg-2-chunk-16-left-64.onnx"),
            kws_dir.join("tokens.txt"),
            vad_path.clone(),
            keywords_path.clone(),
        ] {
            if !p.exists() {
                return Err(anyhow!("voice model file missing: {}", p.display()));
            }
        }

        let recognizer = ParaformerRecognizer::new(ParaformerConfig {
            model: asr_dir
                .join("model.int8.onnx")
                .to_string_lossy()
                .into_owned(),
            tokens: asr_dir.join("tokens.txt").to_string_lossy().into_owned(),
            num_threads: Some(2),
            debug: false,
            provider: None,
        })
        .map_err(|e| anyhow!("init paraformer asr: {e}"))?;

        let vad = SileroVad::new(
            SileroVadConfig {
                model: vad_path.to_string_lossy().into_owned(),
                threshold: 0.5,
                min_silence_duration: 0.5,
                min_speech_duration: 0.25,
                max_speech_duration: 30.0,
                window_size: 512,
                sample_rate: 16000,
                num_threads: Some(1),
                debug: false,
                provider: None,
            },
            60.0,
        )
        .map_err(|e| anyhow!("init silero vad: {e}"))?;

        let (kws_spotter, kws_stream) = unsafe { create_kws(&kws_dir, &keywords_path)? };

        Ok(Self {
            kws_spotter,
            kws_stream,
            vad,
            recognizer,
            bypass_wakeword: false,
            is_awakened: false,
            is_speaking: false,
        })
    }

    pub fn set_bypass_wakeword(&mut self, value: bool) {
        self.bypass_wakeword = value;
        if value {
            self.is_awakened = true;
        }
    }

    /// 推入一段 16kHz 单声道 PCM16-LE 字节流。
    /// 返回管线在该批样本上产生的事件（唤醒、ASR 文本……）。
    pub fn push_audio_chunk(&mut self, pcm_bytes: &[u8]) -> Vec<PipelineEvent> {
        let mut events = Vec::new();
        if pcm_bytes.len() < 2 {
            return events;
        }

        let samples: Vec<f32> = pcm_bytes
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect();
        if samples.is_empty() {
            return events;
        }

        if !self.bypass_wakeword {
            if let Some(keyword) = self.run_kws(&samples) {
                self.is_awakened = true;
                events.push(PipelineEvent::Wakeword(keyword));
            }
        }

        if self.is_awakened {
            self.vad.accept_waveform(samples);
            if !self.is_speaking && self.vad.is_speech() {
                self.is_speaking = true;
            }
            self.drain_vad(&mut events);
        }

        events
    }

    /// websocket 断开或 manual end 时调用，把 VAD 队列里残余 segment 全部解码。
    pub fn flush_pending_asr(&mut self, keep_awake: bool) -> Vec<PipelineEvent> {
        let mut events = Vec::new();
        self.vad.flush();
        self.drain_vad(&mut events);
        self.is_speaking = false;
        self.is_awakened = keep_awake || self.bypass_wakeword;
        events
    }

    fn drain_vad(&mut self, events: &mut Vec<PipelineEvent>) {
        while !self.vad.is_empty() {
            let seg = self.vad.front();
            self.vad.pop();
            if seg.samples.is_empty() {
                continue;
            }
            self.is_speaking = false;
            self.is_awakened = self.bypass_wakeword;
            let result = self.recognizer.transcribe(16000, &seg.samples);
            let text = result.text.trim().to_string();
            if !text.is_empty() {
                events.push(PipelineEvent::AsrResult(text));
            }
        }
    }

    fn run_kws(&mut self, samples: &[f32]) -> Option<String> {
        unsafe {
            sys::SherpaOnnxOnlineStreamAcceptWaveform(
                self.kws_stream,
                16000,
                samples.as_ptr(),
                samples.len() as i32,
            );
            while sys::SherpaOnnxIsKeywordStreamReady(self.kws_spotter, self.kws_stream) == 1 {
                sys::SherpaOnnxDecodeKeywordStream(self.kws_spotter, self.kws_stream);
            }
            let result_ptr = sys::SherpaOnnxGetKeywordResult(self.kws_spotter, self.kws_stream);
            if result_ptr.is_null() {
                return None;
            }
            let keyword_ptr = (*result_ptr).keyword;
            let keyword = if keyword_ptr.is_null() {
                String::new()
            } else {
                CStr::from_ptr(keyword_ptr as _)
                    .to_string_lossy()
                    .trim()
                    .to_string()
            };
            sys::SherpaOnnxDestroyKeywordResult(result_ptr);
            if keyword.is_empty() {
                None
            } else {
                sys::SherpaOnnxResetKeywordStream(self.kws_spotter, self.kws_stream);
                Some(keyword)
            }
        }
    }
}

unsafe fn create_kws(
    kws_dir: &Path,
    keywords_path: &Path,
) -> Result<(
    *const sys::SherpaOnnxKeywordSpotter,
    *const sys::SherpaOnnxOnlineStream,
)> {
    let provider = CString::new("cpu")?;
    let encoder = CString::new(
        kws_dir
            .join("encoder-epoch-12-avg-2-chunk-16-left-64.onnx")
            .to_string_lossy()
            .as_ref(),
    )?;
    let decoder = CString::new(
        kws_dir
            .join("decoder-epoch-12-avg-2-chunk-16-left-64.onnx")
            .to_string_lossy()
            .as_ref(),
    )?;
    let joiner = CString::new(
        kws_dir
            .join("joiner-epoch-12-avg-2-chunk-16-left-64.onnx")
            .to_string_lossy()
            .as_ref(),
    )?;
    let tokens = CString::new(kws_dir.join("tokens.txt").to_string_lossy().as_ref())?;
    let keywords = CString::new(keywords_path.to_string_lossy().as_ref())?;

    let cfg = sys::SherpaOnnxKeywordSpotterConfig {
        feat_config: sys::SherpaOnnxFeatureConfig {
            sample_rate: 16000,
            feature_dim: 80,
        },
        keywords_buf: null(),
        keywords_buf_size: 0,
        keywords_file: keywords.as_ptr(),
        max_active_paths: 4,
        keywords_score: 3.0,
        keywords_threshold: 0.1,
        num_trailing_blanks: 1,
        model_config: sys::SherpaOnnxOnlineModelConfig {
            transducer: sys::SherpaOnnxOnlineTransducerModelConfig {
                encoder: encoder.as_ptr(),
                decoder: decoder.as_ptr(),
                joiner: joiner.as_ptr(),
            },
            num_threads: 2,
            provider: provider.as_ptr(),
            debug: 0,
            tokens: tokens.as_ptr(),
            paraformer: std::mem::zeroed(),
            zipformer2_ctc: std::mem::zeroed(),
            model_type: std::mem::zeroed(),
            modeling_unit: std::mem::zeroed(),
            bpe_vocab: std::mem::zeroed(),
            tokens_buf: std::mem::zeroed(),
            tokens_buf_size: std::mem::zeroed(),
            nemo_ctc: std::mem::zeroed(),
        },
    };

    let spotter = sys::SherpaOnnxCreateKeywordSpotter(&cfg);
    if spotter.is_null() {
        return Err(anyhow!("SherpaOnnxCreateKeywordSpotter returned null"));
    }
    let stream = sys::SherpaOnnxCreateKeywordStream(spotter);
    if stream.is_null() {
        sys::SherpaOnnxDestroyKeywordSpotter(spotter);
        return Err(anyhow!("SherpaOnnxCreateKeywordStream returned null"));
    }
    Ok((spotter, stream))
}
