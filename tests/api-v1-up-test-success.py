#!/usr/bin/env python
import asyncio
from time import time

import requests
from lib.testbase import TestBase


class ApiV1UpTestSuccess(TestBase):
    async def ping(self):
        headers = {"authorization": self.state["user"]["access_token"]}
        r = requests.get(f"{self.base_url}/api/v1/up", headers=headers)
        r.raise_for_status()

    async def setup(self):
        pass

    async def on_connected(self, ws):
        # Ping the homeserver, so it knows device was up for the first time,
        # and activates the timer for the first "downtime" notification.
        await self.ping()

        # @TODO: Make notification time configurable, so this test can run much faster.

        # Expect a normal downtime notification.
        message = await self.wait_for_message(ws)
        assert message["event"] == "message"
        assert all(p in message for p in ("title", "message", "tags", "priority"))
        assert message["title"] == "Відключення світла!"

        # Ping it again immediately to receive the "uptime" notification.
        start_t = time()
        await self.ping()

        # Expect a normal uptime notification.
        message = await self.wait_for_message(ws)
        assert message["event"] == "message"
        assert all(p in message for p in ("title", "message", "tags", "priority"))
        assert message["title"] == "Світло з'явилося!"
        # Notification was success, but took unreasonibly long.
        assert (t := round(time() - start_t, 4)) < 0.05, f"Notification took too long ({t})"


if __name__ == "__main__":
    test = ApiV1UpTestSuccess(timeout=60)
    asyncio.run(test.run())
