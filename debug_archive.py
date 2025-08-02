#!/usr/bin/env python3
"""
Debug script to test GitHub Archive file fetching
"""

import asyncio
import aiohttp
import xml.etree.ElementTree as ET
from datetime import datetime


async def debug_file_fetching():
    """Debug the archive file fetching logic"""
    
    s3_list_url = 'https://data.gharchive.org/?list-type=2'
    
    print(f"Fetching from: {s3_list_url}")
    
    async with aiohttp.ClientSession() as session:
        async with session.get(s3_list_url) as response:
            if response.status != 200:
                print(f"Failed to fetch file list: HTTP {response.status}")
                return
            
            content = await response.text()
            print(f"Response length: {len(content)} characters")
            print(f"First 500 characters:\n{content[:500]}...")
            
            # Parse XML response
            root = ET.fromstring(content)
            files = []
            
            # Debug namespaces
            print(f"Root tag: {root.tag}")
            print(f"Root attributes: {root.attrib}")
            
            # Try different namespace patterns
            namespaces = {
                'aws': 'http://s3.amazonaws.com/doc/2006-03-01/',
                'doc': 'http://doc.s3.amazonaws.com/2006-03-01'
            }
            
            # Try both namespace patterns
            for ns_name, ns_url in namespaces.items():
                print(f"\nTrying namespace {ns_name}: {ns_url}")
                contents_elements = root.findall(f'.//{{{ns_url}}}Contents')
                print(f"Found {len(contents_elements)} Contents elements")
                
                if contents_elements:
                    for i, contents in enumerate(contents_elements[:5]):  # Show first 5
                        key = contents.find(f'{{{ns_url}}}Key')
                        last_modified = contents.find(f'{{{ns_url}}}LastModified')
                        size = contents.find(f'{{{ns_url}}}Size')
                        
                        if key is not None:
                            print(f"  File {i+1}: {key.text}")
                            if key.text.endswith('.json.gz'):
                                files.append({
                                    'filename': key.text,
                                    'url': f"https://data.gharchive.org/{key.text}",
                                    'last_modified': last_modified.text if last_modified is not None else None,
                                    'size': int(size.text) if size is not None else 0
                                })
                    break
            
            print(f"\nTotal files found: {len(files)}")
            if files:
                print("Sample files:")
                for file_info in files[:10]:
                    print(f"  {file_info['filename']} ({file_info['size']} bytes)")
            
            # Look for today's or recent files
            today = datetime.now()
            recent_files = []
            
            for file_info in files:
                filename = file_info['filename']
                # Try to extract date from filename (format: YYYY-MM-DD-H.json.gz)
                try:
                    date_part = filename.split('.')[0]  # Remove .json.gz
                    date_components = date_part.split('-')
                    if len(date_components) >= 3:
                        year, month, day = int(date_components[0]), int(date_components[1]), int(date_components[2])
                        file_date = datetime(year, month, day)
                        
                        # Check if it's from the last 30 days
                        days_ago = (today - file_date).days
                        if days_ago <= 30:
                            recent_files.append((file_info, days_ago))
                except:
                    pass
            
            print(f"\nRecent files (last 30 days): {len(recent_files)}")
            if recent_files:
                # Sort by days ago
                recent_files.sort(key=lambda x: x[1])
                print("Most recent files:")
                for file_info, days_ago in recent_files[:10]:
                    print(f"  {file_info['filename']} ({days_ago} days ago)")


if __name__ == "__main__":
    asyncio.run(debug_file_fetching())
