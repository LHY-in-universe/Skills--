import os
import json
import asyncio
import httpx
import time
import logging
from pathlib import Path
from typing import Dict, Any, Optional, List

import lark_oapi as lark
from lark_oapi.api.im.v1 import P2ImMessageReceiveV1
from lark_oapi.ws import Client

# Configure logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(name)s - %(levelname)s - %(message)s')
logger = logging.getLogger("LarkBridge")

class LarkBridge:
    def __init__(self, orchestrator):
        self.orchestrator = orchestrator
        # Pull credentials from the orchestrator's environment dict
        self.app_id = orchestrator.env.get("LARK_APP_ID")
        self.app_secret = orchestrator.env.get("LARK_APP_SECRET")
        self.api_base = "https://open.feishu.cn/open-apis"
        self._tenant_access_token = None
        self._token_expires_at = 0
        self.loop = None
        
        # Session mapping: l_chat_id -> internal conv_id
        self.session_map: Dict[str, str] = {}

    async def get_tenant_access_token(self) -> str:
        """Fetch or refresh the tenant access token."""
        if self._tenant_access_token and self._token_expires_at > time.time() + 60:
            return self._tenant_access_token

        url = f"{self.api_base}/auth/v3/tenant_access_token/internal"
        data = {"app_id": self.app_id, "app_secret": self.app_secret}
        
        async with httpx.AsyncClient() as client:
            resp = await client.post(url, json=data)
            resp.raise_for_status()
            res = resp.json()
            if res.get("code") == 0:
                self._tenant_access_token = res["tenant_access_token"]
                self._token_expires_at = time.time() + res.get("expire", 7200)
                return self._tenant_access_token
            else:
                raise Exception(f"Failed to get Lark token: {res.get('msg')}")

    def _parse_message_content(self, message) -> str:
        """Extract plain text from Lark message content JSON."""
        try:
            content_json = json.loads(message.content)
            if message.message_type == "text":
                # Handle text and potential mentions
                text = content_json.get("text", "")
                # Clean up @bot mentions if any (common in group)
                import re
                text = re.sub(r'at user_id="[^"]+"', '', text)
                return text.strip()
            return f"[不支持的消息类型: {message.message_type}]"
        except Exception as e:
            logger.error(f"Error parsing message content: {e}")
            return ""

    async def _create_streaming_card(self, chat_id: str) -> str:
        """Create a CardKit 2.0 streaming card instance."""
        token = await self.get_tenant_access_token()
        url = f"{self.api_base}/cardkit/v1/cards"
        
        card_json = {
            "schema": "2.0",
            "config": {
                "streaming_mode": True,
                "summary": {"content": "AI 正在思考中..."},
                "streaming_config": {"print_frequency_ms": {"default": 80}, "print_step": {"default": 1}}
            },
            "body": {
                "elements": [
                    {"tag": "markdown", "content": "⏳ AI 正在思考中...", "element_id": "content"}
                ]
            },
            "header": {
                "title": {"tag": "plain_text", "content": "SiliconFlow AI 助手"},
                "template": "blue"
            }
        }
        
        async with httpx.AsyncClient() as client:
            resp = await client.post(
                url, 
                headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
                json={"type": "card_json", "data": json.dumps(card_json)}
            )
            resp.raise_for_status()
            res = resp.json()
            if res.get("code") == 0:
                return res["data"]["card_id"]
            else:
                raise Exception(f"Failed to create CardKit card: {res.get('msg')}")

    async def _reply_with_card(self, message_id: str, card_id: str):
        """Reply to a message with a card template."""
        token = await self.get_tenant_access_token()
        url = f"{self.api_base}/im/v1/messages/{message_id}/reply"
        
        content = json.dumps({"type": "card", "data": {"card_id": card_id}})
        
        async with httpx.AsyncClient() as client:
            resp = await client.post(
                url,
                headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
                json={"msg_type": "interactive", "content": content}
            )
            resp.raise_for_status()
            return resp.json()

    async def _update_card_content(self, card_id: str, text: str, sequence: int):
        """Update the content element of a CardKit card."""
        token = await self.get_tenant_access_token()
        url = f"{self.api_base}/cardkit/v1/cards/{card_id}/elements/content/content"
        
        data = {
            "content": text or "...",
            "sequence": sequence,
            "uuid": f"upd_{card_id}_{sequence}"
        }
        
        async with httpx.AsyncClient() as client:
            await client.put(
                url,
                headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
                json=data
            )

    async def _finalize_card(self, card_id: str, final_text: str, sequence: int):
        """Close streaming mode and set final summary."""
        token = await self.get_tenant_access_token()
        url = f"{self.api_base}/cardkit/v1/cards/{card_id}/settings"
        
        settings = json.dumps({
            "config": {
                "streaming_mode": False,
                "summary": {"content": final_text[:100] + "..." if len(final_text) > 100 else final_text}
            }
        })
        
        async with httpx.AsyncClient() as client:
            await client.patch(
                url,
                headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
                json={"settings": settings, "sequence": sequence, "uuid": f"fin_{card_id}_{sequence}"}
            )

    async def handle_message(self, data: P2ImMessageReceiveV1) -> None:
        """Main handler for received Lark messages."""
        message = data.event.message
        chat_id = message.chat_id
        user_input = self._parse_message_content(message)
        
        if not user_input:
            return

        logger.info(f"Processing Lark message from chat {chat_id}: {user_input[:50]}")

        # 1. Setup session mapping
        if chat_id not in self.session_map:
            conv = self.orchestrator.create_conversation()
            self.session_map[chat_id] = conv["id"]
        
        conv_id = self.session_map[chat_id]

        # 2. Start streaming UI in Lark
        card_id = None
        try:
            card_id = await self._create_streaming_card(chat_id)
            await self._reply_with_card(message.message_id, card_id)
        except Exception as e:
            logger.error(f"UI setup failed: {e}")
            # Fallback to simple text reply if card fails
            return

        # 3. Process chat and stream back to Lark
        accumulated_text = ""
        sequence = 2
        last_update_time = 0
        update_interval = 0.5 # 500ms throttle

        try:
            async for event in self.orchestrator.stream_chat(user_input, conv_id=conv_id):
                if event["type"] == "text":
                    accumulated_text += event["content"]
                    
                    now = time.time()
                    if now - last_update_time > update_interval:
                        await self._update_card_content(card_id, accumulated_text, sequence)
                        sequence += 1
                        last_update_time = now
                
                elif event["type"] == "tool_start":
                    # Show tool execution status on the card
                    tool_name = event.get("name", "工具")
                    status_text = f"{accumulated_text}\n\n---\n🛠️ **正在执行技能**: `{tool_name}`..."
                    await self._update_card_content(card_id, status_text, sequence)
                    sequence += 1
                
                elif event["type"] == "tool_done":
                    # Optionally briefly show completion before next text chunk
                    await self._update_card_content(card_id, accumulated_text, sequence)
                    sequence += 1
            
            # Final update
            await self._update_card_content(card_id, accumulated_text, sequence)
            sequence += 1
            await self._finalize_card(card_id, accumulated_text, sequence)
            
        except Exception as e:
            logger.error(f"Chat processing failed: {e}")
            if card_id:
                try:
                    await self._update_card_content(card_id, f"❌ 错误: {str(e)}", sequence + 1)
                    await self._finalize_card(card_id, "Error", sequence + 2)
                except: pass

    def start(self):
        """Initialize and start the Lark WebSocket client."""
        if not self.app_id or not self.app_secret or "your_app_id" in self.app_id:
            logger.error("LARK_APP_ID or LARK_APP_SECRET not correctly set in .env")
            return

        # Setup an event loop for this thread's async operations
        self.loop = asyncio.new_event_loop()
        asyncio.set_event_loop(self.loop)

        # Sync wrapper to call our async handler from the SDK's sync dispatcher
        def sync_handler_wrapper(data: P2ImMessageReceiveV1) -> None:
            future = asyncio.run_coroutine_threadsafe(self.handle_message(data), self.loop)
            try:
                # We don't block too long here to keep the SDK's dispatcher moving
                future.result(timeout=60) 
            except Exception as e:
                logger.error(f"Error in sync handler wrapper: {e}")

        # Define Event Handler
        event_handler = lark.EventDispatcherHandler.builder("", "") \
            .register_p2_im_message_receive_v1(sync_handler_wrapper) \
            .build()

        client = Client(self.app_id, self.app_secret, 
                        event_handler=event_handler, 
                        log_level=lark.LogLevel.INFO)
        
        logger.info("Starting Lark WebSocket Bridge...")
        client.start()

def start_lark_bridge(orchestrator):
    bridge = LarkBridge(orchestrator)
    import threading
    t = threading.Thread(target=bridge.start, daemon=True)
    t.start()
    return bridge
