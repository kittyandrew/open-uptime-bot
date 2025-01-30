import asyncio
import json
import os
import sys
from abc import ABC, abstractmethod

import requests
from websockets.client import WebSocketClientProtocol, connect


class TestBase(ABC):
    SUCCESS_CODE = 0
    ERROR_CODE = 666

    def __init__(self, timeout: float):
        self.timeout = timeout
        self.log = lambda s: print(f"[{self.__class__.__name__}] - {s}", file=sys.stderr)

        self.base_url = os.environ["OUBOT_BASE_URL"]
        self.ntfy_base_url = os.environ["NTFY_BASE_URL"]

        self.ntfy_url = None
        # self.state = None

    async def _setup(self):
        self.log("Running setup function...")
        data = {
            "user_type": "Normal",
            "invites_limit": 5,
            "up_delay": 5,  # Set minimal allowed delay to test faster.
            "ntfy_enabled": True,
            "tg_enabled": False,
            "tg_user_id": 12345,
            "tg_language_code": "en",
        }
        r = requests.post(f"{self.base_url}/api/v1/users", json=data)
        r.raise_for_status()
        result = r.json()
        # self.log(f"Result: {result} ...")
        assert result["status"] == 200
        self.state = result["state"]

        username = self.state["ntfy"]["username"]
        password = self.state["ntfy"]["password"]
        topic = self.state["ntfy"]["topic"]
        self.ntfy_url = f"{username}:{password}@{self.ntfy_base_url}/{topic}"

        await self.setup()

    @abstractmethod
    async def setup(self) -> None:
        pass

    @abstractmethod
    async def on_connected(self, ws: WebSocketClientProtocol):
        pass

    async def wait_for_message(self, ws: WebSocketClientProtocol):
        event = await ws.recv()
        self.log(f"RECEIVED MESSAGE: {event}")
        return json.loads(event)

    async def _run(self):
        if not self.ntfy_url:
            self.log("env NTFY_BASE_URL has to be provided")
            exit(self.ERROR_CODE)

        async with connect(f"ws://{self.ntfy_url}/ws") as websocket:
            message = json.loads(await websocket.recv())
            assert message["event"] == "open"
            self.log(f"[NTFY] - Received opened socket event: {message}")

            await self.on_connected(websocket)
            os._exit(self.SUCCESS_CODE)

    async def _timeout(self):
        await asyncio.sleep(self.timeout)
        self.log(f"REACHED TIMEOUT ({self.timeout} SECONDS)!")
        exit(self.ERROR_CODE)

    async def run(self):
        await self._setup()
        await asyncio.gather(self._run(), self._timeout())
        exit(self.ERROR_CODE)  # This should only be reached if we exited improperly.
