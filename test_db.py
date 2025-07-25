#!/usr/bin/env python3
"""
Simple test script to debug database connection
"""

import asyncio
import asyncpg
from config import Config

async def test_connection():
    config = Config()
    
    print(f"Testing connection with:")
    print(f"  Host: {config.DB_HOST}")
    print(f"  Port: {config.DB_PORT}")
    print(f"  Database: {config.DB_NAME}")
    print(f"  User: {config.DB_USER}")
    print(f"  Password: {'*' * len(config.DB_PASSWORD)}")
    
    try:
        conn = await asyncpg.connect(
            host=config.DB_HOST,
            port=config.DB_PORT,
            database=config.DB_NAME,
            user=config.DB_USER,
            password=config.DB_PASSWORD
        )
        
        result = await conn.fetchval("SELECT current_database()")
        print(f"✅ Connected successfully! Current database: {result}")
        
        await conn.close()
        
    except Exception as e:
        print(f"❌ Connection failed: {e}")

if __name__ == "__main__":
    asyncio.run(test_connection())
