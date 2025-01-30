import asyncio

import aiohttp
import numpy as np

base_url = "http://0.0.0.0:8080"


async def create_user():
    data = {
        "user_type": "Normal",
        "invites_limit": 5,
        "up_delay": 5,  # Set minimal allowed delay to test faster.
        "ntfy_enabled": False,
        "tg_enabled": False,
        "tg_user_id": 12345,
        "tg_language_code": "en",
    }
    async with aiohttp.ClientSession() as session:
        r = await session.post(f"{base_url}/api/v1/users", json=data)
        r.raise_for_status()
        return await r.json()


async def get_user(id: str):
    async with aiohttp.ClientSession() as session:
        r = await session.get(f"{base_url}/api/v1/users/{id}")
        r.raise_for_status()
        return await r.json()


async def create_invite(id: str):
    async with aiohttp.ClientSession() as session:
        r = await session.post(f"{base_url}/api/v1/invites", json={"owner_id": id})
        r.raise_for_status()
        return await r.json()


async def up(token: str, sem: asyncio.Semaphore):
    async with sem, aiohttp.ClientSession() as session:
        await asyncio.sleep(0.5)
        r = await session.get(f"{base_url}/api/v1/up", headers={"authorization": token})
        r.raise_for_status()
        return float(r.headers["x-response-time"].split()[0].strip())


async def test(N, data, sem):
    token = data["state"]["user"]["access_token"]
    print(f"\nRunning test with {N:,} requests ...")
    results = np.array(await asyncio.gather(*(up(token, sem) for _ in range(N))))
    results.sort()
    median, p95th = np.percentile(results, 50), np.percentile(results, 95)
    print(f"min {min(results)}ms, max {max(results)}ms, median {median}ms, 95th precentile {p95th}ms")


async def main():
    # sem = asyncio.Semaphore(1)
    # data1, data2 = await create_user(), await create_user()
    # await asyncio.gather(test(99, data1, sem), test(99, data2, sem))
    data = await create_user()
    token = await create_invite(data["state"]["user"]["id"])
    print(token)
    user = await get_user(data["state"]["user"]["id"])
    print(user)


if __name__ == "__main__":
    asyncio.run(main())
