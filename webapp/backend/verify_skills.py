import sys
import os
import json
import time
from pathlib import Path
from orchestrator import ChatOrchestrator

def main():
    # Use the actual absolute path to ensure robustness
    root = Path("/Users/lhy/Desktop/Skills探索")
    
    try:
        orch = ChatOrchestrator(root)
        
        print("\n" + "="*40)
        print("Skill Orchestrator Initialization Report")
        print("="*40)
        
        # 1. Check loaded tools
        tool_names = [t["function"]["name"] for t in orch.tools]
        print(f"Total tools loaded: {len(tool_names)}")
        print(f"Tool list: {', '.join(tool_names)}")
        
        # 2. Check for newly migrated Markdown skills
        migrated = ["github", "summarize", "video-frames"]
        all_ok = True
        print("\n[Migrated Skills Status]")
        for m in migrated:
            if m in tool_names:
                print(f"✅ '{m}': LOADED (Markdown Mode)")
            else:
                print(f"❌ '{m}': NOT FOUND")
                all_ok = False
        
        # 3. Check for existence of Native skills
        native = ["get_weather", "get_current_time", "run_terminal"]
        print("\n[Native Skills Status]")
        for n in native:
            if n in tool_names:
                print(f"✅ '{n}': LOADED (Native Mode)")
            else:
                print(f"❌ '{n}': NOT FOUND")
                all_ok = False
                
        print("\n" + "="*40)
        if all_ok:
            print("Status: ALL PASS 🎉")
        else:
            print("Status: SOME ISSUES FOUND")
        print("="*40 + "\n")

    except Exception as e:
        print(f"CRITICAL ERROR during orchestrator initialization: {e}")
        import traceback
        traceback.print_exc()

if __name__ == "__main__":
    main()
