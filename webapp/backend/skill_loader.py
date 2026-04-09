import os
import re
import yaml
import json
import logging
from pathlib import Path
from typing import List, Dict, Any, Optional

logger = logging.getLogger("SkillLoader")

import shutil
import shlex

class MarkdownSkillLoader:
    """Loads and parses OpenClaw-style SKILL.md files into executable tools."""
    
    def __init__(self, skills_root: Path):
        self.skills_root = skills_root
        self.skills: Dict[str, Dict[str, Any]] = {}

    def check_dependencies(self) -> Dict[str, Dict[str, Any]]:
        """Checks if the required binaries for each skill are installed."""
        report = {}
        for name, skill in self.skills.items():
            requires = skill.get("metadata", {}).get("openclaw", {}).get("requires", {})
            bins = requires.get("bins", [])
            
            skill_report = {"ready": True, "missing": []}
            for b in bins:
                if not shutil.which(b):
                    skill_report["ready"] = False
                    skill_report["missing"].append(b)
            
            skill["ready"] = skill_report["ready"]
            skill["missing_bins"] = skill_report["missing"]
            report[name] = skill_report
            
        return report

    def scan_skills(self) -> List[Dict[str, Any]]:
        """Scans the skills directory and loads all valid Markdown skills."""
        tools = []
        if not self.skills_root.exists():
            return tools

        for skill_dir in self.skills_root.iterdir():
            if not skill_dir.is_dir():
                continue
            
            skill_file = skill_dir / "SKILL.md"
            if skill_file.exists():
                try:
                    skill_data = self._parse_skill_file(skill_file)
                    if skill_data:
                        self.skills[skill_data["name"]] = skill_data
                        tools.append(skill_data["tool_schema"])
                except Exception as e:
                    logger.error(f"Failed to load skill from {skill_dir}: {e}")
        
        return tools

    def _parse_skill_file(self, path: Path) -> Optional[Dict[str, Any]]:
        """Parses SKILL.md for metadata and command templates."""
        content = path.read_text(encoding="utf-8")
        
        # 1. Extract Frontmatter
        frontmatter_match = re.match(r'^---\s*\n(.*?)\n---\s*\n', content, re.DOTALL)
        if not frontmatter_match:
            return None
        
        try:
            metadata = yaml.safe_load(frontmatter_match.group(1))
        except Exception:
            return None

        name = metadata.get("name")
        description = metadata.get("description", "")
        
        # 2. Extract first bash block as the primary command template
        # We look for the first bash block in the markdown
        bash_match = re.search(r'```bash\n(.*?)\n```', content, re.DOTALL)
        command_template = bash_match.group(1).strip() if bash_match else ""

        # 3. Construct OpenAI Tool Schema
        # Note: Generic MD skills won't have strict parameter definitions in YAML usually
        # but OpenClaw skills often have examples. We'll default to a generic 'input' parameter
        # or try to infer from the command template.
        
        # Refinement: If it's a known skill we migrated, we can be more specific, 
        # but for true dynamic usage, we use a flexible parameter schema.
        tool_schema = {
            "type": "function",
            "function": {
                "name": name,
                "description": description,
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command_args": {
                            "type": "string",
                            "description": "Arguments to pass to the skill's command. Example: '--city London' or 'https://google.com'"
                        }
                    },
                    "required": ["command_args"]
                }
            }
        }

        return {
            "name": name,
            "path": path.parent,
            "template": command_template,
            "tool_schema": tool_schema,
            "metadata": metadata
        }

    def get_command(self, skill_name: str, args: Dict[str, Any]) -> Optional[str]:
        """Interpolates arguments into the skill's command template."""
        skill = self.skills.get(skill_name)
        if not skill:
            return None
        
        template = skill["template"]
        # Basic substitution: replace {baseDir} with the actual skill path
        command = template.replace("{baseDir}", str(skill["path"]))
        
        # Add arguments
        cmd_args = args.get("command_args") or args.get("arguments", "")
        if cmd_args:
            # Safely quote the arguments to prevent injection
            safe_args = shlex.quote(str(cmd_args))
            return f"{command} {safe_args}".strip()
            
        return command.strip()
