import argparse
import base64
import json
import mimetypes
from pathlib import Path
from typing import Any, Dict

import requests


def _guess_mime(p: Path) -> str:
    mime, _ = mimetypes.guess_type(str(p))
    return mime or 'image/jpeg'


def _path_to_data_url(image_path: Path, project_root: Path) -> str:
    rp = image_path.expanduser().resolve()
    root = project_root.resolve()
    if not str(rp).startswith(str(root)):
        raise ValueError('image_path 超出允许目录范围')
    if not rp.exists() or not rp.is_file():
        raise ValueError('image_path 不存在或不是文件')
    mime = _guess_mime(rp)
    b64 = base64.b64encode(rp.read_bytes()).decode('utf-8')
    return f'data:{mime};base64,{b64}'


def run(args: Dict[str, Any]) -> Dict[str, Any]:
    api_url = (args.get('api_url') or '').strip()
    api_key = (args.get('api_key') or '').strip()
    model = (args.get('model') or '').strip()
    image_url = (args.get('image_url') or '').strip()
    image_path = (args.get('image_path') or '').strip()
    question = (args.get('question') or '请描述并分析这张图片').strip()
    detail = (args.get('detail') or 'normal').strip()

    if not api_url:
        return {'ok': False, 'error': '缺少 api_url'}
    if not api_key:
        return {'ok': False, 'error': '缺少 api_key'}
    if not model:
        return {'ok': False, 'error': '缺少 model'}

    project_root = Path(args.get('project_root') or Path.cwd())
    if not image_url and image_path:
        try:
            image_url = _path_to_data_url(Path(image_path), project_root)
        except Exception as e:
            return {'ok': False, 'error': f'路径图片处理失败: {e}'}

    if not image_url:
        return {'ok': False, 'error': '请提供 image_url 或 image_path'}

    endpoint = api_url if api_url.endswith('/chat/completions') else api_url.rstrip('/') + '/chat/completions'
    payload = {
        'model': model,
        'messages': [
            {
                'role': 'user',
                'content': [
                    {'type': 'text', 'text': question},
                    {'type': 'image_url', 'image_url': {'url': image_url, 'detail': detail}},
                ],
            }
        ],
        'stream': False,
    }
    headers = {
        'Authorization': f'Bearer {api_key}',
        'Content-Type': 'application/json',
    }

    try:
        r = requests.post(endpoint, headers=headers, json=payload, timeout=90)
        if r.status_code >= 400:
            txt = r.text[:1000]
            return {'ok': False, 'error': f'视觉模型请求失败({r.status_code}): {txt}'}
        data = r.json()
        content = (
            data.get('choices', [{}])[0]
            .get('message', {})
            .get('content', '')
        )
        return {
            'ok': True,
            'summary': content,
            'model': model,
            'endpoint': endpoint,
            'raw': data,
        }
    except Exception as e:
        return {'ok': False, 'error': f'请求异常: {e}'}


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('--args', required=True)
    ns = p.parse_args()
    try:
        payload = json.loads(ns.args)
    except Exception as e:
        print(json.dumps({'ok': False, 'error': f'参数解析失败: {e}'}, ensure_ascii=False))
        raise SystemExit(0)

    print(json.dumps(run(payload), ensure_ascii=False))
