#!/usr/bin/env python3
"""
Database cleanup script - Fix corrupted data and data type issues
"""

import asyncio
import asyncpg
from config import Config

async def cleanup_database():
    """Clean up corrupted data and fix data type issues"""
    config = Config()
    
    # Connect to database
    dsn = f"postgresql://{config.DB_USER}:{config.DB_PASSWORD}@{config.DB_HOST}:{config.DB_PORT}/{config.DB_NAME}"
    pool = await asyncpg.create_pool(dsn, min_size=1, max_size=5)
    
    try:
        async with pool.acquire() as conn:
            print("🔍 Analyzing database for issues...")
            
            # 1. Check for events with numeric type fields (these are corrupted)
            corrupted_count = await conn.fetchval("""
                SELECT COUNT(*) FROM github_events 
                WHERE type ~ '^[0-9]+$'
            """)
            
            if corrupted_count > 0:
                print(f"⚠️  Found {corrupted_count:,} events with numeric type fields (corrupted)")
                
                # Show examples
                examples = await conn.fetch("""
                    SELECT id, type, created_at 
                    FROM github_events 
                    WHERE type ~ '^[0-9]+$' 
                    LIMIT 5
                """)
                
                print("   Examples of corrupted events:")
                for event in examples:
                    print(f"     ID {event['id']}: type='{event['type']}' created={event['created_at']}")
                
                # Delete corrupted events
                print("🗑️  Deleting corrupted events...")
                deleted = await conn.execute("""
                    DELETE FROM github_events 
                    WHERE type ~ '^[0-9]+$'
                """)
                
                print(f"✅ Deleted {deleted} corrupted events")
            else:
                print("✅ No corrupted events with numeric types found")
            
            # 2. Check for events with NULL or empty required fields
            print("\n🔍 Checking for events with missing required data...")
            
            null_type_count = await conn.fetchval("""
                SELECT COUNT(*) FROM github_events 
                WHERE type IS NULL OR type = ''
            """)
            
            if null_type_count > 0:
                print(f"⚠️  Found {null_type_count:,} events with NULL/empty type")
                await conn.execute("""
                    DELETE FROM github_events 
                    WHERE type IS NULL OR type = ''
                """)
                print(f"✅ Deleted events with NULL/empty type")
            
            # 3. Validate and fix ID fields
            print("\n🔧 Validating ID fields...")
            
            # Check for string values that should be integers
            id_issues = await conn.fetch("""
                SELECT 
                    COUNT(CASE WHEN actor_id::text = 'null' OR actor_id::text = '' THEN 1 END) as bad_actor_ids,
                    COUNT(CASE WHEN repo_id::text = 'null' OR repo_id::text = '' THEN 1 END) as bad_repo_ids,
                    COUNT(CASE WHEN org_id::text = 'null' OR org_id::text = '' THEN 1 END) as bad_org_ids
                FROM github_events
            """)
            
            if id_issues:
                issue = id_issues[0]
                if issue['bad_actor_ids'] > 0 or issue['bad_repo_ids'] > 0 or issue['bad_org_ids'] > 0:
                    print(f"   Bad actor_ids: {issue['bad_actor_ids']:,}")
                    print(f"   Bad repo_ids: {issue['bad_repo_ids']:,}")  
                    print(f"   Bad org_ids: {issue['bad_org_ids']:,}")
                    
                    # Fix bad ID values
                    await conn.execute("""
                        UPDATE github_events 
                        SET actor_id = NULL 
                        WHERE actor_id::text = 'null' OR actor_id::text = ''
                    """)
                    
                    await conn.execute("""
                        UPDATE github_events 
                        SET repo_id = NULL 
                        WHERE repo_id::text = 'null' OR repo_id::text = ''
                    """)
                    
                    await conn.execute("""
                        UPDATE github_events 
                        SET org_id = NULL 
                        WHERE org_id::text = 'null' OR org_id::text = ''
                    """)
                    
                    print("✅ Fixed bad ID values")
                else:
                    print("✅ All ID fields look good")
            
            # 4. Get final database stats
            print("\n📊 Final database statistics:")
            
            total_events = await conn.fetchval("SELECT COUNT(*) FROM github_events")
            total_repos = await conn.fetchval("SELECT COUNT(*) FROM repositories")
            
            # Get event type distribution
            event_types = await conn.fetch("""
                SELECT type, COUNT(*) as count 
                FROM github_events 
                GROUP BY type 
                ORDER BY count DESC 
                LIMIT 10
            """)
            
            print(f"   Total events: {total_events:,}")
            print(f"   Total repositories: {total_repos:,}")
            print("   Top event types:")
            for event_type in event_types:
                print(f"     {event_type['type']}: {event_type['count']:,}")
            
            # 5. Check for files that may have failed processing
            failed_files = await conn.fetch("""
                SELECT filename, event_count, file_size, processed_at
                FROM processed_files 
                WHERE event_count = 0 AND file_size > 100000
                ORDER BY processed_at DESC
                LIMIT 5
            """)
            
            if failed_files:
                print(f"\n⚠️  Files that may need reprocessing:")
                for file_info in failed_files:
                    print(f"   {file_info['filename']}: size={file_info['file_size']:,} bytes, processed={file_info['processed_at']}")
            else:
                print("\n✅ No failed file processing detected")
            
            print("\n🎉 Database cleanup completed successfully!")
            
    except Exception as e:
        print(f"❌ Error during cleanup: {e}")
        import traceback
        traceback.print_exc()
    finally:
        await pool.close()

if __name__ == "__main__":
    asyncio.run(cleanup_database())
