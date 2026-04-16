import asyncio
import base64
import json
import sys
from pathlib import Path

from voice_engine import get_voice_engine


PROJECT_ROOT = Path(__file__).resolve().parents[2]
voice_engine = get_voice_engine(PROJECT_ROOT)


async def emit(event: dict):
    sys.stdout.write(json.dumps(event, ensure_ascii=False) + "\n")
    sys.stdout.flush()


async def on_text_captured(text: str):
    await emit({"type": "asr_result", "content": text})


async def on_event(event: dict):
    await emit(event)


async def read_stdin_line() -> str:
    loop = asyncio.get_running_loop()
    return await loop.run_in_executor(None, sys.stdin.readline)


async def main():
    if not voice_engine:
        await emit({"type": "error", "content": "Voice Engine not ready."})
        return

    bypass_wakeword = False
    while True:
        line = await read_stdin_line()
        if not line:
            break
        line = line.strip()
        if not line:
            continue

        try:
            msg = json.loads(line)
        except Exception as e:
            await emit({"type": "error", "content": f"invalid_json: {e}"})
            continue

        msg_type = msg.get("type")
        try:
            if msg_type == "shutdown":
                break
            if msg_type == "debug_config":
                bypass_wakeword = bool(msg.get("bypass_wakeword", False))
                await emit({
                    "type": "debug_config_ack",
                    "bypass_wakeword": bypass_wakeword,
                })
            elif msg_type == "audio_chunk":
                data_b64 = msg.get("data_b64", "")
                if not data_b64:
                    continue
                chunk = base64.b64decode(data_b64)
                await voice_engine.push_audio_chunk(
                    chunk,
                    on_text_captured,
                    on_event,
                    bypass_wakeword=bypass_wakeword,
                )
            elif msg_type == "end_utterance":
                await voice_engine.flush_pending_asr(
                    on_text_captured,
                    keep_awake=bypass_wakeword,
                )
            else:
                await emit({"type": "error", "content": f"unsupported_command: {msg_type}"})
        except Exception as e:
            await emit({"type": "error", "content": str(e)})


if __name__ == "__main__":
    asyncio.run(main())
