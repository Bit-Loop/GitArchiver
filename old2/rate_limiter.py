#!/usr/bin/env python3
"""
GitHub API Rate Limiter
Manages GitHub API rate limits intelligently to prevent 403/429 errors
"""

import asyncio
import aiohttp
import time
import logging
from datetime import datetime, timedelta
from typing import Optional, Dict, Any
from dataclasses import dataclass


@dataclass
class RateLimitStatus:
    """Rate limit status information"""
    limit: int
    remaining: int
    reset_time: int
    used: int
    resource: str
    authenticated: bool
    
    @property
    def reset_datetime(self) -> datetime:
        """Convert reset time to datetime"""
        return datetime.fromtimestamp(self.reset_time)
    
    @property
    def seconds_until_reset(self) -> int:
        """Seconds until rate limit resets"""
        return max(0, self.reset_time - int(time.time()))
    
    @property
    def is_exhausted(self) -> bool:
        """Check if rate limit is exhausted"""
        return self.remaining <= 5  # Keep 5 requests as buffer


class GitHubRateLimiter:
    """Intelligent GitHub API rate limiter"""
    
    def __init__(self, github_token: Optional[str] = None):
        self.github_token = github_token
        self.authenticated = bool(github_token)
        self.session: Optional[aiohttp.ClientSession] = None
        
        # Rate limit tracking
        self._core_limit: Optional[RateLimitStatus] = None
        self._search_limit: Optional[RateLimitStatus] = None
        self._graphql_limit: Optional[RateLimitStatus] = None
        
        # Request timing
        self._last_request_time = 0
        self._min_request_interval = 0.1  # Minimum 100ms between requests
        
        self.logger = logging.getLogger(__name__)
        
    async def __aenter__(self):
        """Async context manager entry"""
        await self.initialize()
        return self
        
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit"""
        await self.close()
        
    async def initialize(self):
        """Initialize the rate limiter"""
        headers = {
            'User-Agent': 'GitHubArchiver/1.0',
            'Accept': 'application/vnd.github.v3+json'
        }
        
        if self.github_token:
            headers['Authorization'] = f'token {self.github_token}'
            
        self.session = aiohttp.ClientSession(
            headers=headers,
            timeout=aiohttp.ClientTimeout(total=30)
        )
        
        # Get initial rate limit status
        await self.check_rate_limits()
        
    async def close(self):
        """Close the session"""
        if self.session:
            await self.session.close()
            
    async def check_rate_limits(self) -> Dict[str, RateLimitStatus]:
        """Check all rate limit statuses"""
        try:
            async with self.session.get('https://api.github.com/rate_limit') as response:
                if response.status == 200:
                    data = await response.json()
                    
                    self._core_limit = RateLimitStatus(
                        limit=data['rate']['limit'],
                        remaining=data['rate']['remaining'],
                        reset_time=data['rate']['reset'],
                        used=data['rate']['used'],
                        resource='core',
                        authenticated=self.authenticated
                    )
                    
                    self._search_limit = RateLimitStatus(
                        limit=data['search']['limit'],
                        remaining=data['search']['remaining'],
                        reset_time=data['search']['reset'],
                        used=data['search']['used'],
                        resource='search',
                        authenticated=self.authenticated
                    )
                    
                    if 'graphql' in data:
                        self._graphql_limit = RateLimitStatus(
                            limit=data['graphql']['limit'],
                            remaining=data['graphql']['remaining'],
                            reset_time=data['graphql']['reset'],
                            used=data['graphql']['used'],
                            resource='graphql',
                            authenticated=self.authenticated
                        )
                        
                    self.logger.info(f"Rate limits - Core: {self._core_limit.remaining}/{self._core_limit.limit}, "
                                   f"Search: {self._search_limit.remaining}/{self._search_limit.limit}")
                    
                    return {
                        'core': self._core_limit,
                        'search': self._search_limit,
                        'graphql': self._graphql_limit
                    }
                else:
                    self.logger.error(f"Failed to check rate limits: {response.status}")
                    
        except Exception as e:
            self.logger.error(f"Error checking rate limits: {e}")
            
        return {}
    
    async def wait_if_needed(self, resource: str = 'core') -> bool:
        """Wait if rate limit is exhausted"""
        rate_limit = self._core_limit if resource == 'core' else self._search_limit
        
        if not rate_limit:
            await self.check_rate_limits()
            rate_limit = self._core_limit if resource == 'core' else self._search_limit
            
        if rate_limit and rate_limit.is_exhausted:
            wait_time = rate_limit.seconds_until_reset + 10  # Add 10 second buffer
            self.logger.warning(f"Rate limit exhausted for {resource}. Waiting {wait_time} seconds until reset.")
            await asyncio.sleep(wait_time)
            await self.check_rate_limits()
            return True
            
        # Enforce minimum request interval
        current_time = time.time()
        time_since_last = current_time - self._last_request_time
        if time_since_last < self._min_request_interval:
            await asyncio.sleep(self._min_request_interval - time_since_last)
            
        self._last_request_time = time.time()
        return False
    
    async def make_request(self, method: str, url: str, resource: str = 'core', **kwargs) -> aiohttp.ClientResponse:
        """Make a rate-limited GitHub API request"""
        await self.wait_if_needed(resource)
        
        try:
            async with self.session.request(method, url, **kwargs) as response:
                # Update rate limit info from headers
                self._update_rate_limit_from_headers(response.headers, resource)
                
                if response.status in [403, 429]:
                    # Rate limited, update status and potentially wait
                    await self.check_rate_limits()
                    if 'retry-after' in response.headers:
                        retry_after = int(response.headers['retry-after'])
                        self.logger.warning(f"Rate limited. Waiting {retry_after} seconds.")
                        await asyncio.sleep(retry_after)
                    else:
                        await self.wait_if_needed(resource)
                        
                return response
                
        except Exception as e:
            self.logger.error(f"Request failed: {e}")
            raise
            
    def _update_rate_limit_from_headers(self, headers: Dict[str, str], resource: str):
        """Update rate limit status from response headers"""
        try:
            if 'x-ratelimit-remaining' in headers:
                limit = int(headers.get('x-ratelimit-limit', 0))
                remaining = int(headers.get('x-ratelimit-remaining', 0))
                reset_time = int(headers.get('x-ratelimit-reset', 0))
                used = int(headers.get('x-ratelimit-used', 0))
                
                rate_limit = RateLimitStatus(
                    limit=limit,
                    remaining=remaining,
                    reset_time=reset_time,
                    used=used,
                    resource=resource,
                    authenticated=self.authenticated
                )
                
                if resource == 'core':
                    self._core_limit = rate_limit
                elif resource == 'search':
                    self._search_limit = rate_limit
                    
        except (ValueError, KeyError) as e:
            self.logger.debug(f"Could not parse rate limit headers: {e}")
            
    def get_status(self) -> Dict[str, Any]:
        """Get current rate limiter status"""
        return {
            'authenticated': self.authenticated,
            'core_limit': self._core_limit.__dict__ if self._core_limit else None,
            'search_limit': self._search_limit.__dict__ if self._search_limit else None,
            'graphql_limit': self._graphql_limit.__dict__ if self._graphql_limit else None,
            'last_request_time': self._last_request_time,
            'github_token_configured': bool(self.github_token)
        }
