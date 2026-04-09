from fastapi import FastAPI, HTTPException, WebSocket
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import StreamingResponse
from pydantic import BaseModel
from typing import List, Dict, Any, Optional
from pathlib import Path
import json as _json

from orchestrator import ChatOrchestrator
from lark_bridge import start_lark_bridge
from voice_engine import get_voice_engine

app = FastAPI(title="SiliconFlow AI Orchestrator API")
voice_engine = None

@app.on_event("startup")
async def startup_event():
    global voice_engine
    # Start the Lark WebSocket bridge
    start_lark_bridge(orchestrator)
    print("Lark Bridge started successfully.")
    
    # Initialize Voice Engine
    try:
        voice_engine = get_voice_engine(PROJECT_ROOT)
        print("Voice Engine initialized.")
    except Exception as e:
        print(f"Failed to initialize Voice Engine: {e}")

# Enable CORS for the frontend
app.add_middleware(
    CORSMiddleware,
    allow_origins=[
        "http://localhost:5173",
        "http://localhost:5174",
        "http://127.0.0.1:5173",
        "http://127.0.0.1:5174",
        "null",  # Electron file:// protocol
    ],
    allow_credentials=False,
    allow_methods=["GET", "POST", "DELETE", "PATCH"],
    allow_headers=["Content-Type"],
)

PROJECT_ROOT = Path(__file__).parent.parent.parent
orchestrator = ChatOrchestrator(PROJECT_ROOT)

class ChatRequest(BaseModel):
    user_input: str
    conv_id: Optional[str] = None

class ConfigUpdateRequest(BaseModel):
    api_url: Optional[str] = None
    api_key: Optional[str] = None
    model: Optional[str] = None

class SkillToggleRequest(BaseModel):
    name: str
    enabled: bool

class ModelAddRequest(BaseModel):
    name: str
    model_id: str

class RoutingConfigRequest(BaseModel):
    enabled: bool
    router_model: str
    summary_model: str = ""
    tiers: Dict[str, str]

@app.get("/api/routing")
async def get_routing():
    return orchestrator.router.get_config()

@app.post("/api/routing")
async def update_routing(req: RoutingConfigRequest):
    config = {"enabled": req.enabled, "router_model": req.router_model, "summary_model": req.summary_model, "tiers": req.tiers}
    orchestrator.router.save_config(config)
    return {"status": "ok", "config": config}

@app.get("/api/models")
async def get_models():
    return orchestrator.models

@app.post("/api/models")
async def add_model(req: ModelAddRequest):
    orchestrator.add_model(req.name, req.model_id)
    return {"status": "ok", "models": orchestrator.models}

@app.delete("/api/models/{model_name}")
async def delete_model(model_name: str):
    orchestrator.delete_model(model_name)
    return {"status": "ok", "models": orchestrator.models}

@app.get("/api/config")
async def get_config():
    return {
        "api_url": orchestrator.api_url,
        "current_model": orchestrator.current_model
    }

@app.post("/api/config")
async def update_config(req: ConfigUpdateRequest):
    orchestrator.update_config(req.api_url, req.api_key, req.model)
    return {"status": "ok", "config": await get_config()}

@app.get("/api/skills")
async def get_skills():
    # Return skills with their names, descriptions, and current status
    skills_info = []
    for tool in orchestrator.tools:
        name = tool["function"]["name"]
        skills_info.append({
            "name": name,
            "description": tool["function"]["description"],
            "enabled": orchestrator.active_skills.get(name, False)
        })
    return skills_info

@app.post("/api/skills/toggle")
async def toggle_skill(req: SkillToggleRequest):
    orchestrator.toggle_skill(req.name, req.enabled)
    return {"status": "ok", "name": req.name, "enabled": req.enabled}

def _sse(event: dict) -> str:
    return f"data: {_json.dumps(event, ensure_ascii=False)}\n\n"

@app.post("/api/chat")
async def chat(req: ChatRequest):
    if not orchestrator.api_key:
        raise HTTPException(status_code=400, detail="API Key is not configured.")

    async def generate():
        # Fallback to active_conversation_id if trying to decouple later, currently strip conv_id
        async for event in orchestrator.stream_chat(req.user_input):
            yield _sse(event)

    return StreamingResponse(generate(), media_type="text/event-stream")

@app.get("/api/history")
async def get_history(conv_id: Optional[str] = None):
    # Return messages without system prompt for display
    messages = orchestrator._conversations.get(conv_id, {}).get("messages") if conv_id else orchestrator.messages
    return [m for m in (messages or []) if m["role"] != "system"]

@app.post("/api/history/clear")
async def clear_history():
    orchestrator.clear_history()
    return {"status": "ok"}

class ConversationRenameRequest(BaseModel):
    name: str

@app.get("/api/conversations")
async def list_conversations():
    return orchestrator.list_conversations()

@app.post("/api/conversations")
async def create_conversation():
    return orchestrator.create_conversation()

@app.post("/api/conversations/{conv_id}/activate")
async def activate_conversation(conv_id: str):
    try:
        orchestrator.switch_conversation(conv_id)
        return {"status": "ok", "active_id": conv_id}
    except ValueError as e:
        raise HTTPException(status_code=404, detail=str(e))

@app.delete("/api/conversations/{conv_id}")
async def delete_conversation(conv_id: str):
    try:
        orchestrator.delete_conversation(conv_id)
        return {"status": "ok", "conversations": orchestrator.list_conversations()}
    except ValueError as e:
        raise HTTPException(status_code=404, detail=str(e))

@app.patch("/api/conversations/{conv_id}")
async def rename_conversation(conv_id: str, req: ConversationRenameRequest):
    try:
        orchestrator.rename_conversation(conv_id, req.name)
        return {"status": "ok"}
    except ValueError as e:
        raise HTTPException(status_code=404, detail=str(e))

class TerminalCwdRequest(BaseModel):
    cwd: str

@app.get("/api/terminal/cwd")
async def get_terminal_cwd():
    return {"cwd": orchestrator.user_cwd or ""}

@app.post("/api/terminal/cwd")
async def set_terminal_cwd(req: TerminalCwdRequest):
    cwd = req.cwd.strip()
    if cwd:
        p = Path(cwd).expanduser().resolve()
        if not p.exists() or not p.is_dir():
            raise HTTPException(status_code=400, detail=f"目录不存在: {cwd}")
        orchestrator.user_cwd = str(p)
    else:
        orchestrator.user_cwd = None
    return {"cwd": orchestrator.user_cwd or ""}

class PermissionResumeRequest(BaseModel):
    granted: bool
    always_allow: bool = False

@app.post("/api/chat/resume")
async def resume_chat(req: PermissionResumeRequest):
    """Called after user responds to a permission dialog."""
    async def generate():
        async for event in orchestrator.stream_resume_after_permission(req.granted, req.always_allow):
            yield _sse(event)

    return StreamingResponse(generate(), media_type="text/event-stream")

@app.post("/api/chat/abort")
async def abort_chat():
    """Abort the current generation."""
    orchestrator.abort()
    return {"status": "ok"}

@app.get("/api/token-usage")
async def get_token_usage():
    return orchestrator.token_tracker.get_stats()

@app.websocket("/api/voice/bridge")
async def voice_bridge(websocket: WebSocket):
    await websocket.accept()
    print("Voice Bridge connected.")
    
    if not voice_engine:
        await websocket.send_json({"type": "error", "content": "Voice Engine not ready."})
        await websocket.close()
        return

    # Callback when a full sentence is captured via ASR
    async def on_text_captured(text: str):
        await websocket.send_json({"type": "asr_result", "content": text})
        
        # Drive the orchestrator with the recognized text
        async for event in orchestrator.stream_chat(text):
            await websocket.send_json(event)
            if event["type"] == "text":
                content = event["content"]
                async for audio_chunk in voice_engine.get_streaming_tts(content):
                    import base64
                    await websocket.send_json({
                        "type": "audio_stream",
                        "data": base64.b64encode(audio_chunk).decode("utf-8")
                    })

    # Callback for non-text events like wake-word detection
    async def on_event(event: Dict):
        await websocket.send_json(event)

    try:
        while True:
            data = await websocket.receive()
            if "bytes" in data:
                await voice_engine.push_audio_chunk(data["bytes"], on_text_captured, on_event)
            elif "text" in data:
                # Handle control messages like "abort"
                msg = _json.loads(data["text"])
                if msg.get("type") == "abort":
                    orchestrator.abort()
    except Exception as e:
        print(f"Voice Bridge disconnected: {e}")
    finally:
        print("Voice Bridge closed.")

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
