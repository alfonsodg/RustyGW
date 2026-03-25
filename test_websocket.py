#!/usr/bin/env python3
import asyncio
import websockets
import json

async def test_websocket():
    uri = "ws://gdev02:8094/ws"
    try:
        async with websockets.connect(uri) as websocket:
            print("✅ Conectado al WebSocket del gateway")
            
            # Enviar mensaje de prueba
            test_message = {"type": "ping", "data": "test"}
            await websocket.send(json.dumps(test_message))
            print(f"📤 Enviado: {test_message}")
            
            # Recibir respuesta
            response = await asyncio.wait_for(websocket.recv(), timeout=5.0)
            print(f"📥 Recibido: {response}")
            
    except Exception as e:
        print(f"❌ Error: {e}")

if __name__ == "__main__":
    asyncio.run(test_websocket())
