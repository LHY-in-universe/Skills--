import asyncio
from webapp.backend.orchestrator import ChatOrchestrator
from pathlib import Path
import os

async def main():
    project_root = Path(".").absolute()
    # Ensure directories exist
    (project_root / "siliconflow" / "config").mkdir(parents=True, exist_ok=True)
    (project_root / "siliconflow" / "data").mkdir(parents=True, exist_ok=True)
    (project_root / "siliconflow" / "index" / "memory").mkdir(parents=True, exist_ok=True)
    
    # Pre-create a dummy models.json to avoid errors
    with open(project_root / "webapp" / "backend" / "models.json", "w") as f:
        f.write('{"Qwen-27B": "Qwen/Qwen3.5-27B"}')

    orch = ChatOrchestrator(project_root)
    print("Initial API URL:", orch.api_url)
    
    # Trigger update
    orch.update_config(api_url="https://api.deepseek.com", api_key="sk-test-key")
    print("Updated API URL:", orch.api_url)
    
    # Check .env file
    env_path = project_root / "siliconflow" / "config" / ".env"
    if env_path.exists():
        content = env_path.read_text()
        print(".env content:\n", content)
    else:
        print(".env not found!")

if __name__ == "__main__":
    asyncio.run(main())
