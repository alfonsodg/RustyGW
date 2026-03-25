#!/usr/bin/env python3
"""Mock backend services for RustyGW feature testing."""
import asyncio
import json
import sys
from aiohttp import web

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 9001
NAME = sys.argv[2] if len(sys.argv) > 2 else f"service-{PORT}"

async def health(request):
    return web.Response(text="OK")

async def api(request):
    path = request.path
    headers = dict(request.headers)
    return web.json_response({
        "service": NAME,
        "port": PORT,
        "path": path,
        "method": request.method,
        "traceparent": headers.get("traceparent", "none"),
        "x-request-id": headers.get("x-request-id", "none"),
        "x-custom": headers.get("x-custom", "none"),
    })

async def slow(request):
    await asyncio.sleep(10)
    return web.json_response({"service": NAME, "slow": True})

async def fail(request):
    return web.Response(status=503, text="Service Unavailable")

async def ws_handler(request):
    ws = web.WebSocketResponse()
    await ws.prepare(request)
    async for msg in ws:
        if msg.type == web.WSMsgType.TEXT:
            await ws.send_str(json.dumps({"echo": msg.data, "from": NAME}))
    return ws

app = web.Application()
app.router.add_get("/health", health)
app.router.add_get("/ws", ws_handler)
app.router.add_get("/slow", slow)
app.router.add_get("/fail", fail)
app.router.add_route("*", "/{path:.*}", api)

if __name__ == "__main__":
    print(f"Mock {NAME} starting on port {PORT}")
    web.run_app(app, port=PORT, print=None)
