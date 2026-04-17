import asyncio
from webapp.backend.router import ModelRouter
from webapp.backend.orchestrator import ChatOrchestrator
from pathlib import Path

async def main():
    router = ModelRouter("dummy", "dummy", Path("dummy.json"))
    
    # Fake tools
    dummy_tools = [
        {"type": "function", "function": {"name": "get_weather", "description": "Get weather"}},
        {"type": "function", "function": {"name": "run_terminal", "description": "Run shell commands"}}
    ]
    print("Testing Skill Router...")
    res = await router.select_skills_via_gemma("Help me check the weather and run ls -la", dummy_tools)
    print("Skill Tools Selected:", [r["function"]["name"] for r in res])

    print("Testing Model Router...")
    tier = await router.classify_async("Help me check the weather and run ls -la", "qwen")
    print("Difficulty Tier:", tier)

asyncio.run(main())
