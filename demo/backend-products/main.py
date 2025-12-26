#!/usr/bin/env python3
"""
Products Service - Demo Backend
Simple REST API for product catalog
"""

from fastapi import FastAPI, HTTPException, WebSocket
from pydantic import BaseModel
from typing import List, Optional
import uvicorn
import json

app = FastAPI(title="Products Service", version="1.0.0")

# Sample data
products_db = [
    {"id": 1, "name": "Laptop Pro", "price": 1299.99, "category": "electronics", "stock": 15},
    {"id": 2, "name": "Wireless Mouse", "price": 29.99, "category": "electronics", "stock": 50},
    {"id": 3, "name": "Coffee Mug", "price": 12.99, "category": "home", "stock": 100},
    {"id": 4, "name": "Notebook", "price": 5.99, "category": "office", "stock": 200},
]

class Product(BaseModel):
    id: int
    name: str
    price: float
    category: str
    stock: int

@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    await websocket.accept()
    await websocket.send_text(json.dumps({"service": "products", "data": products_db}))

@app.get("/")
async def root():
    return {"service": "products", "status": "running", "version": "1.0.0"}

@app.get("/products", response_model=List[Product])
async def get_products():
    return products_db

@app.get("/products/{product_id}", response_model=Product)
async def get_product(product_id: int):
    product = next((p for p in products_db if p["id"] == product_id), None)
    if not product:
        raise HTTPException(status_code=404, detail="Product not found")
    return product

@app.get("/products/category/{category}", response_model=List[Product])
async def get_products_by_category(category: str):
    products = [p for p in products_db if p["category"] == category]
    return products

@app.get("/health")
async def health():
    return {"status": "healthy", "service": "products"}

if __name__ == "__main__":
    uvicorn.run(app, host="0.0.0.0", port=8092)
