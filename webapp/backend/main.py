from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
from typing import List, Dict, Any, Optional
from pathlib import Path
import os

from orchestrator import ChatOrchestrator

app = FastAPI(title="SiliconFlow AI Orchestrator API")

# Enable CORS for the frontend
app.add_middleware(
    CORSMiddleware,
    allow_origins=[
        "http://localhost:5173",
        "http://localhost:5174",
        "http://127.0.0.1:5173",
        "http://127.0.0.1:5174",
    ],
    allow_credentials=False,
    allow_methods=["GET", "POST", "DELETE", "PATCH"],
    allow_headers=["Content-Type"],
)

PROJECT_ROOT = Path(__file__).parent.parent.parent
orchestrator = ChatOrchestrator(PROJECT_ROOT)

class ChatRequest(BaseModel):
    user_input: str

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

@app.post("/api/chat")
async def chat(req: ChatRequest):
    if not orchestrator.api_key:
        raise HTTPException(status_code=400, detail="API Key is not configured.")
    
    response = await orchestrator.chat(req.user_input)
    return response

@app.get("/api/history")
async def get_history():
    # Return messages without system prompt for display
    return [m for m in orchestrator.messages if m["role"] != "system"]

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
    from pathlib import Path as P
    cwd = req.cwd.strip()
    if cwd:
        p = P(cwd).expanduser().resolve()
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
    response = await orchestrator.resume_after_permission(req.granted, req.always_allow)
    return response

@app.post("/api/chat/abort")
async def abort_chat():
    """Abort the current generation."""
    orchestrator.abort()
    return {"status": "ok"}

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
