import os
import asyncio
import json
import logging
import uuid
from pathlib import Path
from typing import Optional, List, Dict, Any, Callable

import sherpa_onnx
import edge_tts
import numpy as np
from pydub import AudioSegment

logger = logging.getLogger("VoiceEngine")

class VoiceEngine:
    """
    Core speech engine handling KWS (Wake-word), VAD (Silero), ASR (Paraformer), and TTS (Edge).
    Optimized for low-latency and future BPU acceleration.
    """
    def __init__(self, models_root: Path):
        self.models_root = models_root
        self.asr_dir = models_root / "sherpa-onnx-paraformer-zh-2023-09-14"
        self.kws_dir = models_root / "sherpa-onnx-kws-zipformer-wenetspeech-3.3M-2024-01-01"
        self.vad_path = models_root / "silero_vad.onnx"
        self.keywords_path = models_root / "keywords.txt"
        
        # 1. ASR Config (Paraformer-mini)
        self.recognizer = self._init_recognizer()
        
        # 2. KWS Config (Zipformer)
        self.kws = self._init_kws()
        self.kws_stream = self.kws.create_stream() if self.kws else None
        
        # 3. VAD Config (Silero)
        self.vad = self._init_vad()
        
        # State Management
        self.audio_buffer: List[np.ndarray] = []
        self.is_awakened = False
        self.is_speaking = False
        self.sample_rate = 16000

    def _init_recognizer(self):
        """Initialize the Sherpa-ONNX Paraformer recognizer."""
        try:
            recognizer = sherpa_onnx.OfflineRecognizer.from_paraformer(
                paraformer=str(self.asr_dir / "model.int8.onnx"),
                tokens=str(self.asr_dir / "tokens.txt"),
                num_threads=2,
                sample_rate=16000,
                feature_config=sherpa_onnx.FeatureConfig(
                    sample_rate=16000,
                    feature_dim=80,
                ),
                debug=False,
            )
            logger.info("ASR Recognizer (Paraformer) initialized.")
            return recognizer
        except Exception as e:
            logger.error(f"Failed to init ASR: {e}")
            return None

    def _init_kws(self):
        """Initialize Sherpa-ONNX Keyword Spotter."""
        try:
            config = sherpa_onnx.KeywordSpotterConfig(
                feat_config=sherpa_onnx.FeatureConfig(
                    sample_rate=16000,
                    feature_dim=80,
                ),
                model_config=sherpa_onnx.OnlineModelConfig(
                    zipformer=sherpa_onnx.OnlineZipformerModelConfig(
                        encoder=str(self.kws_dir / "encoder-epoch-12-avg-2-chunk-16-left-64.onnx"),
                        decoder=str(self.kws_dir / "decoder-epoch-12-avg-2-chunk-16-left-64.onnx"),
                        joiner=str(self.kws_dir / "joiner-epoch-12-avg-2-chunk-16-left-64.onnx"),
                    ),
                    tokens=str(self.kws_dir / "tokens.txt"),
                    num_threads=2,
                    debug=False,
                ),
                keywords_file=str(self.keywords_path),
                max_active_paths=4,
            )
            kws = sherpa_onnx.KeywordSpotter(config)
            logger.info("Keyword Spotter (KWS) initialized.")
            return kws
        except Exception as e:
            logger.error(f"Failed to init KWS: {e}")
            return None

    def _init_vad(self):
        """Initialize Silero VAD."""
        try:
            config = sherpa_onnx.SileroVadModelConfig(
                model=str(self.vad_path),
                threshold=0.5,
                min_silence_duration=0.5,
                min_speech_duration=0.25,
                window_size=512,
            )
            vad = sherpa_onnx.VoiceActivityDetector(config, buffer_size_in_seconds=60)
            logger.info("Silero VAD initialized.")
            return vad
        except Exception as e:
            logger.error(f"Failed to init VAD: {e}")
            return None

    async def push_audio_chunk(self, chunk_pcm: bytes, on_text_captured: Callable[[str], Any], on_event: Optional[Callable[[Dict], Any]] = None):
        """
        Main logic: KWS -> WAKE -> VAD -> ASR.
        """
        if not self.kws or not self.recognizer or not self.vad:
            return

        samples = np.frombuffer(chunk_pcm, dtype=np.int16).astype(np.float32) / 32768.0
        
        # 1. Continuous KWS Check
        self.kws_stream.accept_waveform(16000, samples)
        while self.kws.is_ready(self.kws_stream):
            self.kws.decode_stream(self.kws_stream)
        
        keyword = self.kws.get_result(self.kws_stream).keyword
        if keyword:
            logger.info(f"WAKE WORD DETECTED: {keyword}")
            self.is_awakened = True
            if on_event:
                await on_event({"type": "wakeword", "keyword": keyword})
            # Clear previous buffer
            self.audio_buffer = []

        # 2. If awakened, run VAD and ASR
        if self.is_awakened:
            self.vad.accept_waveform(samples)
            while not self.vad.empty():
                if not self.is_speaking and self.vad.is_speech_detected():
                    self.is_speaking = True
                    logger.info("VAD: Instruction speech started.")
                    self.audio_buffer = []
                
                if self.is_speaking:
                    self.audio_buffer.append(samples)
                
                if self.is_speaking and self.vad.is_silence_detected():
                    self.is_speaking = False
                    self.is_awakened = False # Reset for next cycle
                    logger.info("VAD: Instruction ended. Running ASR...")
                    
                    if self.audio_buffer:
                        full_audio = np.concatenate(self.audio_buffer)
                        stream = self.recognizer.create_stream()
                        stream.accept_waveform(16000, full_audio)
                        self.recognizer.decode_stream(stream)
                        text = stream.result.text.strip()
                        if text:
                            await on_text_captured(text)
                    
                    self.audio_buffer = []

                self.vad.pop()

    async def generate_speech(self, text: str, output_path: Optional[Path] = None, voice: str = "zh-CN-XiaoxiaoNeural") -> Path:
        """Synthesize text to speech using Edge-TTS."""
        if not output_path:
            temp_dir = Path("temp/voice")
            temp_dir.mkdir(parents=True, exist_ok=True)
            output_path = temp_dir / f"{uuid.uuid4()}.mp3"

        communicate = edge_tts.Communicate(text, voice)
        await communicate.save(str(output_path))
        return output_path

    async def get_streaming_tts(self, text: str, voice: str = "zh-CN-XiaoxiaoNeural"):
        """Generator for streaming MP3 chunks from Edge-TTS."""
        communicate = edge_tts.Communicate(text, voice)
        async for chunk in communicate.stream():
            if chunk["type"] == "audio":
                yield chunk["data"]

def get_voice_engine(project_root: Path):
    models_dir = project_root / "webapp/backend/models/voice"
    return VoiceEngine(models_dir)
