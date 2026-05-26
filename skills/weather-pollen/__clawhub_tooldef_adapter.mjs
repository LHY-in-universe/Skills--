import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const ROOT = path.dirname(fileURLToPath(import.meta.url));
const MANIFEST = JSON.parse(fs.readFileSync(path.join(ROOT, 'skill_manifest.json'), 'utf8'));

function loadPayload() {
  const raw = process.env.SKILL_ARGS_JSON;
  if (raw) {
    try { return JSON.parse(raw); } catch {}
  }
  for (const arg of process.argv.slice(2)) {
    if (arg.startsWith('--args=')) {
      try { return JSON.parse(arg.slice(7)); } catch {}
    }
  }
  return {};
}

const payload = loadPayload();
const entry = MANIFEST.adapter_entry || 'logic.ts';
const mod = await import(pathToFileURL(path.join(ROOT, entry)).href);
const tools = Array.isArray(mod.tools)
  ? mod.tools
  : Object.values(mod).filter((value) => value && typeof value === 'object' && typeof value.name === 'string' && typeof value.execute === 'function');

if (!tools.length) {
  console.error('No exported tools found in module:', entry);
  process.exit(1);
}

let toolName = payload.tool || MANIFEST.default_tool || '';
if (!toolName && tools.length === 1) {
  toolName = tools[0].name;
}

const tool = tools.find((item) => item.name === toolName);
if (!tool) {
  console.error(`Unknown tool: ${toolName}. Available: ${tools.map((item) => item.name).join(', ')}`);
  process.exit(1);
}

const args = payload.args && typeof payload.args === 'object' && !Array.isArray(payload.args)
  ? payload.args
  : Object.fromEntries(Object.entries(payload).filter(([key]) => key !== 'tool'));

const result = await tool.execute(args);
if (typeof result === 'string') {
  console.log(result);
} else {
  console.log(JSON.stringify(result ?? null, null, 2));
}
