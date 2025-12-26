#!/usr/bin/env python3
"""
Users Service - Demo Backend
Simple REST API for user management
"""

from fastapi import FastAPI, HTTPException, WebSocket
from pydantic import BaseModel
from typing import List, Optional
import uvicorn
import json

app = FastAPI(title="Users Service", version="1.0.0")

# Sample data
users_db = [
    {"id": 1, "name": "Alice Johnson", "email": "alice@example.com", "role": "admin", "active": True},
    {"id": 2, "name": "Bob Smith", "email": "bob@example.com", "role": "user", "active": True},
    {"id": 3, "name": "Carol Davis", "email": "carol@example.com", "role": "user", "active": False},
    {"id": 4, "name": "David Wilson", "email": "david@example.com", "role": "moderator", "active": True},
]

class User(BaseModel):
    id: int
    name: str
    email: str
    role: str
    active: bool

@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    await websocket.accept()
    await websocket.send_text(json.dumps({"service": "users", "data": users_db}))

@app.get("/")
async def root():
    return {"service": "users", "status": "running", "version": "1.0.0"}

@app.get("/users", response_model=List[User])
async def get_users():
    return users_db

@app.get("/users/{user_id}", response_model=User)
async def get_user(user_id: int):
    user = next((u for u in users_db if u["id"] == user_id), None)
    if not user:
        raise HTTPException(status_code=404, detail="User not found")
    return user

@app.get("/users/role/{role}", response_model=List[User])
async def get_users_by_role(role: str):
    users = [u for u in users_db if u["role"] == role]
    return users

@app.get("/health")
async def health():
    return {"status": "healthy", "service": "users"}

if __name__ == "__main__":
    uvicorn.run(app, host="0.0.0.0", port=8091)
