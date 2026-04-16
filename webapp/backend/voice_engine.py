import asyncio
import logging
from pathlib import Path
from typing import Optional, List, Dict, Any, Callable

import sherpa_onnx
import numpy as np

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
                feature_dim=80,
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
            kws = sherpa_onnx.KeywordSpotter(
                tokens=str(self.kws_dir / "tokens.txt"),
                encoder=str(self.kws_dir / "encoder-epoch-12-avg-2-chunk-16-left-64.onnx"),
                decoder=str(self.kws_dir / "decoder-epoch-12-avg-2-chunk-16-left-64.onnx"),
                joiner=str(self.kws_dir / "joiner-epoch-12-avg-2-chunk-16-left-64.onnx"),
                keywords_file=str(self.keywords_path),
                num_threads=2,
                sample_rate=16000,
                feature_dim=80,
                max_active_paths=4,
            )
            logger.info("Keyword Spotter (KWS) initialized.")
            return kws
        except Exception as e:
            logger.error(f"Failed to init KWS: {e}")
            return None

    def _init_vad(self):
        """Initialize Silero VAD."""
        try:
            silero_cfg = sherpa_onnx.SileroVadModelConfig(
                model=str(self.vad_path),
                threshold=0.5,
                min_silence_duration=0.5,
                min_speech_duration=0.25,
                window_size=512,
            )
            config = sherpa_onnx.VadModelConfig(silero_vad=silero_cfg)
            vad = sherpa_onnx.VoiceActivityDetector(config, buffer_size_in_seconds=60)
            logger.info("Silero VAD initialized.")
            return vad
        except Exception as e:
            logger.error(f"Failed to init VAD: {e}")
            return None

    async def push_audio_chunk(
        self,
        chunk_pcm: bytes,
        on_text_captured: Callable[[str], Any],
        on_event: Optional[Callable[[Dict], Any]] = None,
        bypass_wakeword: bool = False,
    ):
        """
        Main logic: KWS -> WAKE -> VAD -> ASR.
        """
        if not self.recognizer or not self.vad or (not bypass_wakeword and not self.kws):
            return

        samples = np.frombuffer(chunk_pcm, dtype=np.int16).astype(np.float32) / 32768.0
        
        if bypass_wakeword:
            self.is_awakened = True
        else:
            # 1. Continuous KWS Check
            self.kws_stream.accept_waveform(16000, samples)
            while self.kws.is_ready(self.kws_stream):
                self.kws.decode_stream(self.kws_stream)

            kws_result = self.kws.get_result(self.kws_stream)
            if isinstance(kws_result, str):
                keyword = kws_result.strip()
            else:
                keyword = str(getattr(kws_result, "keyword", "")).strip()
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
            if not self.is_speaking and self.vad.is_speech_detected():
                self.is_speaking = True
                logger.info("VAD: Instruction speech started.")
                self.audio_buffer = []

            if self.is_speaking:
                self.audio_buffer.append(samples)

            # For sherpa-onnx>=1.12, finalized speech segments are exposed via vad.front
            while not self.vad.empty():
                seg = self.vad.front
                self.vad.pop()
                seg_samples = np.asarray(seg.samples, dtype=np.float32)
                if seg_samples.size == 0:
                    continue

                self.is_speaking = False
                self.is_awakened = bool(bypass_wakeword)
                logger.info("VAD: Instruction ended. Running ASR...")

                stream = self.recognizer.create_stream()
                stream.accept_waveform(16000, seg_samples)
                self.recognizer.decode_stream(stream)
                text = stream.result.text.strip()
                if text:
                    logger.info(f"ASR_TEXT(auto): {text}")
                    await on_text_captured(text)
                self.audio_buffer = []

    async def flush_pending_asr(self, on_text_captured: Callable[[str], Any], keep_awake: bool = False):
        """Flush VAD buffers on websocket disconnect to avoid losing last utterance."""
        if not self.recognizer or not self.vad:
            return
        try:
            self.vad.flush()
        except Exception:
            pass

        emitted = False
        while not self.vad.empty():
            seg = self.vad.front
            self.vad.pop()
            seg_samples = np.asarray(seg.samples, dtype=np.float32)
            if seg_samples.size == 0:
                continue
            stream = self.recognizer.create_stream()
            stream.accept_waveform(16000, seg_samples)
            self.recognizer.decode_stream(stream)
            text = stream.result.text.strip()
            if text:
                emitted = True
                logger.info(f"ASR_TEXT(manual): {text}")
                await on_text_captured(text)

        if not emitted and self.is_speaking:
            seg = self.vad.current_segment
            seg_samples = np.asarray(seg.samples, dtype=np.float32)
            if seg_samples.size > 0:
                stream = self.recognizer.create_stream()
                stream.accept_waveform(16000, seg_samples)
                self.recognizer.decode_stream(stream)
                text = stream.result.text.strip()
                if text:
                    logger.info(f"ASR_TEXT(current): {text}")
                    await on_text_captured(text)

        if not emitted and self.audio_buffer:
            full_audio = np.concatenate(self.audio_buffer)
            if full_audio.size > 0:
                stream = self.recognizer.create_stream()
                stream.accept_waveform(16000, full_audio)
                self.recognizer.decode_stream(stream)
                text = stream.result.text.strip()
                if text:
                    logger.info(f"ASR_TEXT(buffer): {text}")
                    await on_text_captured(text)

        self.is_speaking = False
        self.is_awakened = bool(keep_awake)
        self.audio_buffer = []

def get_voice_engine(project_root: Path):
    models_dir = project_root / "webapp/backend/models/voice"
    return VoiceEngine(models_dir)
