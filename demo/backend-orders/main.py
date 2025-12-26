#!/usr/bin/env python3
"""
Orders Service - Demo Backend
Simple REST API for order management
"""

from fastapi import FastAPI, HTTPException, WebSocket
from pydantic import BaseModel
from typing import List, Optional
from datetime import datetime
import uvicorn
import json

app = FastAPI(title="Orders Service", version="1.0.0")

# Sample data
orders_db = [
    {"id": 1, "user_id": 1, "product_id": 1, "quantity": 1, "total": 1299.99, "status": "completed", "date": "2025-12-20"},
    {"id": 2, "user_id": 2, "product_id": 2, "quantity": 2, "total": 59.98, "status": "pending", "date": "2025-12-22"},
    {"id": 3, "user_id": 1, "product_id": 3, "quantity": 3, "total": 38.97, "status": "shipped", "date": "2025-12-23"},
    {"id": 4, "user_id": 3, "product_id": 4, "quantity": 5, "total": 29.95, "status": "completed", "date": "2025-12-24"},
]

class Order(BaseModel):
    id: int
    user_id: int
    product_id: int
    quantity: int
    total: float
    status: str
    date: str

@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    await websocket.accept()
    await websocket.send_text(json.dumps({"service": "orders", "data": orders_db}))

@app.get("/")
async def root():
    return {"service": "orders", "status": "running", "version": "1.0.0"}

@app.get("/orders", response_model=List[Order])
async def get_orders():
    return orders_db

@app.get("/orders/{order_id}", response_model=Order)
async def get_order(order_id: int):
    order = next((o for o in orders_db if o["id"] == order_id), None)
    if not order:
        raise HTTPException(status_code=404, detail="Order not found")
    return order

@app.get("/orders/user/{user_id}", response_model=List[Order])
async def get_orders_by_user(user_id: int):
    orders = [o for o in orders_db if o["user_id"] == user_id]
    return orders

@app.get("/orders/status/{status}", response_model=List[Order])
async def get_orders_by_status(status: str):
    orders = [o for o in orders_db if o["status"] == status]
    return orders

@app.get("/health")
async def health():
    return {"status": "healthy", "service": "orders"}

if __name__ == "__main__":
    uvicorn.run(app, host="0.0.0.0", port=8093)
