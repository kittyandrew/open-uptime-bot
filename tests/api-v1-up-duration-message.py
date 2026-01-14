#!/usr/bin/env python
import asyncio
import re

import requests
from lib.testbase import TestBase


class ApiV1UpDurationMessage(TestBase):
    """
    Test that validates duration messages in notifications.

    When power goes out: "Світло було X" (power was on for X)
    When power comes back: "Світла не було X" (power was out for X)
    """

    # Regex patterns for Ukrainian duration format
    # Matches: "0 хв", "35 хв", "1 день 12 год 22 хв", "238 днів 15 год 13 хв"
    DURATION_PATTERN = re.compile(r"(\d+\s+(день|дні|днів)\s+)?(\d+\s+год\s+)?(\d+\s+хв)")

    async def ping(self):
        headers = {"authorization": self.state["user"]["access_token"]}
        r = requests.get(f"{self.base_url}/api/v1/up", headers=headers)
        r.raise_for_status()

    async def setup(self):
        pass

    async def on_connected(self, ws):
        # Ping the homeserver, so it knows device was up for the first time,
        # and activates the timer for the first "downtime" notification.
        # This first ping should NOT generate a notification (Uninitialized -> Up has no message).
        self.log("Sending first ping (Uninitialized -> Up)...")
        await self.ping()

        # Wait for the Down notification (after up_delay timeout of 5 seconds).
        self.log("Waiting for Down notification...")
        message = await self.wait_for_message(ws)
        assert message["event"] == "message"
        assert message["title"] == "Відключення світла!"

        # Verify the message body contains "Світло було X" with valid duration format
        # This should have a duration since we tracked state_changed_at from the first ping
        msg_body = message.get("message", "")
        self.log(f"Down notification message body: '{msg_body}'")
        assert msg_body.startswith("Світло було "), f"Expected message to start with 'Світло було ', got: '{msg_body}'"

        # Extract duration part and validate format
        duration_part = msg_body.replace("Світло було ", "")
        assert self.DURATION_PATTERN.match(duration_part), f"Duration format invalid: '{duration_part}'"
        self.log(f"Down notification duration validated: '{duration_part}'")

        # Ping again to receive the "uptime" notification after a minute.
        await asyncio.sleep(61)  # @TODO: Is this possible to do faster? Pre-loading data?
        self.log("Sending second ping (Down -> Up)...")
        await self.ping()

        # Wait for the Up notification.
        self.log("Waiting for Up notification...")
        message = await self.wait_for_message(ws)
        assert message["event"] == "message"
        assert message["title"] == "Світло з'явилося!"

        # Verify the message body contains "Світла не було X" with valid duration format
        # This should have a duration since we tracked when it went down
        msg_body = message.get("message", "")
        self.log(f"Up notification message body: '{msg_body}'")
        assert msg_body.startswith("Світла не було "), f"Expected message to start with 'Світла не було ', got: '{msg_body}'"

        # Extract duration part and validate format
        duration_part = msg_body.replace("Світла не було ", "")
        assert self.DURATION_PATTERN.match(duration_part), f"Duration format invalid: '{duration_part}'"
        self.log(f"Up notification duration validated: '{duration_part}'")

        self.log("All duration message tests passed!")


if __name__ == "__main__":
    test = ApiV1UpDurationMessage(timeout=120)
    asyncio.run(test.run())
