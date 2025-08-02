#!/usr/bin/env python3
"""
Intelligent Wordlist Generator for Bug Bounty and Security Research
Analyzes GitHub data to generate relevant wordlists without duplicates or noise
"""

import asyncio
import re
import json
import logging
from typing import Set, Dict, List, Any, Optional
from collections import Counter, defaultdict
from pathlib import Path
from datetime import datetime, timedelta
import sys

# Add project directory to path
sys.path.insert(0, str(Path(__file__).parent))

from config import Config
from db_manager import GitHubArchiveDB


class WordlistGenerator:
    """Intelligent wordlist generator from GitHub data"""
    
    def __init__(self, config: Config):
        self.config = config
        self.db = GitHubArchiveDB(config)
        self.logger = logging.getLogger(__name__)
        
        # Filtering configuration
        self.min_word_length = 3
        self.max_word_length = 50
        self.min_frequency = 2  # Minimum occurrences to include
        self.max_common_word_freq = 1000  # Exclude overly common words
        
        # Common words to exclude (too generic for bug bounty)
        self.common_words = {
            'the', 'and', 'for', 'are', 'but', 'not', 'you', 'all', 'can', 'had', 'her', 'was', 'one',
            'our', 'out', 'day', 'get', 'has', 'him', 'his', 'how', 'its', 'may', 'new', 'now', 'old',
            'see', 'two', 'who', 'boy', 'did', 'does', 'let', 'man', 'men', 'put', 'say', 'she', 'too',
            'use', 'www', 'com', 'org', 'net', 'edu', 'gov', 'int', 'mil', 'http', 'https', 'ftp',
            'main', 'test', 'demo', 'temp', 'tmp', 'file', 'data', 'info', 'page', 'site', 'web',
            'home', 'user', 'admin', 'root', 'public', 'private', 'static', 'assets', 'images', 'img',
            'css', 'js', 'html', 'php', 'asp', 'jsp', 'xml', 'json', 'txt', 'pdf', 'doc', 'jpg',
            'png', 'gif', 'svg', 'ico', 'readme', 'license', 'changelog', 'todo', 'makefile'
        }
        
        # Technical patterns for different categories
        self.patterns = {
            'subdomains': re.compile(r'\b[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?\b'),
            'paths': re.compile(r'/[a-zA-Z0-9._\-]+(?:/[a-zA-Z0-9._\-]*)*'),
            'parameters': re.compile(r'[?&]([a-zA-Z_][a-zA-Z0-9_]*)[=&]'),
            'filenames': re.compile(r'\b[a-zA-Z0-9._\-]+\.[a-zA-Z0-9]{2,10}\b'),
            'functions': re.compile(r'\b[a-zA-Z_][a-zA-Z0-9_]*\s*\('),
            'variables': re.compile(r'\b[a-zA-Z_][a-zA-Z0-9_]*\b'),
            'technologies': re.compile(r'\b(?:react|angular|vue|node|express|django|flask|spring|laravel|rails)\b', re.I),
            'security_terms': re.compile(r'\b(?:auth|login|admin|password|token|key|secret|api|endpoint|vulnerability)\b', re.I)
        }
        
    async def __aenter__(self):
        """Async context manager entry"""
        await self.db.connect()
        return self
        
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit"""
        await self.db.disconnect()
        
    async def generate_comprehensive_wordlist(self, 
                                            target_domains: Optional[List[str]] = None,
                                            technology_filter: Optional[List[str]] = None,
                                            days_back: int = 30,
                                            max_words: int = 10000) -> Dict[str, List[str]]:
        """Generate comprehensive wordlist from GitHub data"""
        self.logger.info("Starting comprehensive wordlist generation")
        
        try:
            # Get data from different sources
            wordlists = {}
            
            # 1. Repository names and descriptions
            wordlists['repositories'] = await self._extract_from_repositories(
                target_domains, technology_filter, days_back
            )
            
            # 2. Commit messages and branch names
            wordlists['commits'] = await self._extract_from_commits(
                target_domains, technology_filter, days_back
            )
            
            # 3. File and directory names
            wordlists['files'] = await self._extract_from_file_paths(
                target_domains, technology_filter, days_back
            )
            
            # 4. Issue and PR titles
            wordlists['issues'] = await self._extract_from_issues(
                target_domains, technology_filter, days_back
            )
            
            # 5. Technology-specific terms
            wordlists['technologies'] = await self._extract_technology_terms(
                target_domains, technology_filter, days_back
            )
            
            # 6. Security-relevant terms
            wordlists['security'] = await self._extract_security_terms(
                target_domains, technology_filter, days_back
            )
            
            # Filter and rank each category
            for category in wordlists:
                wordlists[category] = self._filter_and_rank_words(
                    wordlists[category], max_words // len(wordlists)
                )
                
            self.logger.info(f"Generated wordlists: {[(k, len(v)) for k, v in wordlists.items()]}")
            return wordlists
            
        except Exception as e:
            self.logger.error(f"Error generating wordlists: {e}")
            return {}
            
    async def generate_targeted_wordlist(self, target_domain: str, 
                                       max_words: int = 5000) -> List[str]:
        """Generate targeted wordlist for specific domain"""
        self.logger.info(f"Generating targeted wordlist for {target_domain}")
        
        try:
            all_words = Counter()
            
            # Search for repositories related to target domain
            async with self.db.pool.acquire() as conn:
                # Find repositories that might be related to target
                repos = await conn.fetch("""
                    SELECT r.name, r.full_name, r.description, r.language
                    FROM repositories r
                    JOIN github_events e ON r.id = e.repo_id
                    WHERE 
                        r.name ILIKE $1 OR 
                        r.full_name ILIKE $1 OR 
                        r.description ILIKE $1 OR
                        r.html_url ILIKE $1
                    GROUP BY r.id, r.name, r.full_name, r.description, r.language
                    ORDER BY COUNT(e.id) DESC
                    LIMIT 100
                """, f'%{target_domain.replace(".", "%")}%')
                
                # Extract words from repository data
                for repo in repos:
                    words = self._extract_words_from_text([
                        repo['name'], repo['full_name'], repo['description'] or ''
                    ])
                    all_words.update(words)
                    
                # Get events for these repositories
                if repos:
                    repo_names = [repo['full_name'] for repo in repos]
                    events = await conn.fetch("""
                        SELECT payload
                        FROM github_events
                        WHERE repo_name = ANY($1)
                        AND created_at > NOW() - INTERVAL '30 days'
                        LIMIT 1000
                    """, repo_names)
                    
                    # Extract words from event payloads
                    for event in events:
                        try:
                            payload = json.loads(event['payload']) if isinstance(event['payload'], str) else event['payload']
                            text_data = self._extract_text_from_payload(payload)
                            words = self._extract_words_from_text(text_data)
                            all_words.update(words)
                        except:
                            continue
                            
            # Filter and rank words
            filtered_words = self._filter_and_rank_words(list(all_words.keys()), max_words)
            
            self.logger.info(f"Generated {len(filtered_words)} words for {target_domain}")
            return filtered_words
            
        except Exception as e:
            self.logger.error(f"Error generating targeted wordlist: {e}")
            return []
            
    async def _extract_from_repositories(self, target_domains: Optional[List[str]], 
                                       technology_filter: Optional[List[str]], 
                                       days_back: int) -> List[str]:
        """Extract words from repository names and descriptions"""
        words = Counter()
        
        try:
            async with self.db.pool.acquire() as conn:
                query = """
                    SELECT r.name, r.full_name, r.description, r.language
                    FROM repositories r
                    JOIN github_events e ON r.id = e.repo_id
                    WHERE e.created_at > NOW() - INTERVAL '%s days'
                """ % days_back
                
                params = []
                if target_domains:
                    query += " AND (r.html_url ILIKE ANY($1) OR r.full_name ILIKE ANY($1))"
                    params.append([f'%{domain}%' for domain in target_domains])
                    
                if technology_filter:
                    query += f" AND r.language = ANY(${len(params) + 1})"
                    params.append(technology_filter)
                    
                query += " GROUP BY r.id, r.name, r.full_name, r.description, r.language LIMIT 5000"
                
                repos = await conn.fetch(query, *params)
                
                for repo in repos:
                    text_parts = [repo['name'], repo['full_name'], repo['description'] or '']
                    extracted_words = self._extract_words_from_text(text_parts)
                    words.update(extracted_words)
                    
        except Exception as e:
            self.logger.error(f"Error extracting from repositories: {e}")
            
        return list(words.keys())
        
    async def _extract_from_commits(self, target_domains: Optional[List[str]], 
                                  technology_filter: Optional[List[str]], 
                                  days_back: int) -> List[str]:
        """Extract words from commit messages"""
        words = Counter()
        
        try:
            async with self.db.pool.acquire() as conn:
                # Look for push events with commit messages
                events = await conn.fetch("""
                    SELECT payload
                    FROM github_events
                    WHERE type = 'PushEvent'
                    AND created_at > NOW() - INTERVAL '%s days'
                    LIMIT 2000
                """ % days_back)
                
                for event in events:
                    try:
                        payload = json.loads(event['payload']) if isinstance(event['payload'], str) else event['payload']
                        commits = payload.get('commits', [])
                        
                        for commit in commits:
                            message = commit.get('message', '')
                            if message:
                                extracted_words = self._extract_words_from_text([message])
                                words.update(extracted_words)
                                
                    except:
                        continue
                        
        except Exception as e:
            self.logger.error(f"Error extracting from commits: {e}")
            
        return list(words.keys())
        
    async def _extract_from_file_paths(self, target_domains: Optional[List[str]], 
                                     technology_filter: Optional[List[str]], 
                                     days_back: int) -> List[str]:
        """Extract words from file and directory paths"""
        words = Counter()
        
        try:
            async with self.db.pool.acquire() as conn:
                # Look for events that contain file paths
                events = await conn.fetch("""
                    SELECT payload
                    FROM github_events
                    WHERE type IN ('PushEvent', 'PullRequestEvent', 'CreateEvent')
                    AND created_at > NOW() - INTERVAL '%s days'
                    LIMIT 2000
                """ % days_back)
                
                for event in events:
                    try:
                        payload = json.loads(event['payload']) if isinstance(event['payload'], str) else event['payload']
                        
                        # Extract file paths from various event types
                        file_paths = []
                        
                        if 'commits' in payload:
                            for commit in payload['commits']:
                                if 'added' in commit:
                                    file_paths.extend(commit['added'])
                                if 'modified' in commit:
                                    file_paths.extend(commit['modified'])
                                if 'removed' in commit:
                                    file_paths.extend(commit['removed'])
                                    
                        # Extract words from file paths
                        for path in file_paths:
                            path_words = self._extract_path_components(path)
                            words.update(path_words)
                            
                    except:
                        continue
                        
        except Exception as e:
            self.logger.error(f"Error extracting from file paths: {e}")
            
        return list(words.keys())
        
    async def _extract_from_issues(self, target_domains: Optional[List[str]], 
                                 technology_filter: Optional[List[str]], 
                                 days_back: int) -> List[str]:
        """Extract words from issue and PR titles"""
        words = Counter()
        
        try:
            async with self.db.pool.acquire() as conn:
                events = await conn.fetch("""
                    SELECT payload
                    FROM github_events
                    WHERE type IN ('IssuesEvent', 'PullRequestEvent')
                    AND created_at > NOW() - INTERVAL '%s days'
                    LIMIT 1000
                """ % days_back)
                
                for event in events:
                    try:
                        payload = json.loads(event['payload']) if isinstance(event['payload'], str) else event['payload']
                        
                        # Extract titles from issues/PRs
                        title = None
                        if 'issue' in payload:
                            title = payload['issue'].get('title')
                        elif 'pull_request' in payload:
                            title = payload['pull_request'].get('title')
                            
                        if title:
                            extracted_words = self._extract_words_from_text([title])
                            words.update(extracted_words)
                            
                    except:
                        continue
                        
        except Exception as e:
            self.logger.error(f"Error extracting from issues: {e}")
            
        return list(words.keys())
        
    async def _extract_technology_terms(self, target_domains: Optional[List[str]], 
                                      technology_filter: Optional[List[str]], 
                                      days_back: int) -> List[str]:
        """Extract technology-specific terms"""
        words = Counter()
        
        # Technology keywords to look for
        tech_patterns = {
            'frameworks': ['react', 'angular', 'vue', 'express', 'django', 'flask', 'spring', 'laravel'],
            'languages': ['javascript', 'python', 'java', 'golang', 'rust', 'typescript', 'php'],
            'tools': ['docker', 'kubernetes', 'jenkins', 'gitlab', 'github', 'aws', 'azure', 'gcp'],
            'databases': ['mysql', 'postgresql', 'mongodb', 'redis', 'elasticsearch'],
            'security': ['oauth', 'jwt', 'ssl', 'tls', 'encryption', 'authentication', 'authorization']
        }
        
        try:
            async with self.db.pool.acquire() as conn:
                # Search for technology-related repositories
                for category, terms in tech_patterns.items():
                    for term in terms:
                        repos = await conn.fetch("""
                            SELECT r.name, r.description
                            FROM repositories r
                            WHERE r.language ILIKE $1 
                            OR r.description ILIKE $1
                            OR r.name ILIKE $1
                            LIMIT 100
                        """, f'%{term}%')
                        
                        for repo in repos:
                            text_parts = [repo['name'], repo['description'] or '']
                            extracted_words = self._extract_words_from_text(text_parts)
                            # Filter for tech-related words
                            tech_words = [w for w in extracted_words if any(t in w.lower() for t in terms)]
                            words.update(tech_words)
                            
        except Exception as e:
            self.logger.error(f"Error extracting technology terms: {e}")
            
        return list(words.keys())
        
    async def _extract_security_terms(self, target_domains: Optional[List[str]], 
                                    technology_filter: Optional[List[str]], 
                                    days_back: int) -> List[str]:
        """Extract security-relevant terms"""
        words = Counter()
        
        security_keywords = [
            'vulnerability', 'exploit', 'security', 'authentication', 'authorization',
            'token', 'api', 'endpoint', 'cors', 'csrf', 'xss', 'sqli', 'injection',
            'admin', 'login', 'password', 'secret', 'key', 'crypto', 'hash',
            'session', 'cookie', 'header', 'parameter', 'payload', 'request'
        ]
        
        try:
            async with self.db.pool.acquire() as conn:
                for keyword in security_keywords:
                    # Search in repository descriptions
                    repos = await conn.fetch("""
                        SELECT r.name, r.description
                        FROM repositories r
                        WHERE r.description ILIKE $1
                        LIMIT 50
                    """, f'%{keyword}%')
                    
                    for repo in repos:
                        text_parts = [repo['name'], repo['description'] or '']
                        extracted_words = self._extract_words_from_text(text_parts)
                        words.update(extracted_words)
                        
        except Exception as e:
            self.logger.error(f"Error extracting security terms: {e}")
            
        return list(words.keys())
        
    def _extract_words_from_text(self, text_parts: List[str]) -> List[str]:
        """Extract meaningful words from text"""
        words = []
        
        for text in text_parts:
            if not text:
                continue
                
            # Split by common delimiters
            text = re.sub(r'[^a-zA-Z0-9\s\-_.]', ' ', text)
            parts = re.split(r'[\s\-_.]+', text.lower())
            
            for part in parts:
                if (self.min_word_length <= len(part) <= self.max_word_length and
                    part not in self.common_words and
                    not part.isdigit() and
                    re.match(r'^[a-zA-Z][a-zA-Z0-9]*$', part)):
                    words.append(part)
                    
        return words
        
    def _extract_path_components(self, path: str) -> List[str]:
        """Extract meaningful components from file paths"""
        words = []
        
        # Split path and extract components
        parts = path.split('/')
        for part in parts:
            if not part:
                continue
                
            # Remove file extensions
            name = re.sub(r'\.[a-zA-Z0-9]+$', '', part)
            
            # Split by common delimiters in filenames
            components = re.split(r'[\-_.]+', name.lower())
            
            for component in components:
                if (self.min_word_length <= len(component) <= self.max_word_length and
                    component not in self.common_words and
                    not component.isdigit()):
                    words.append(component)
                    
        return words
        
    def _extract_text_from_payload(self, payload: Dict[str, Any]) -> List[str]:
        """Extract text from event payload"""
        text_parts = []
        
        def extract_recursive(obj, depth=0):
            if depth > 5:  # Prevent infinite recursion
                return
                
            if isinstance(obj, dict):
                for key, value in obj.items():
                    if isinstance(value, str) and len(value) < 500:  # Avoid huge strings
                        text_parts.append(value)
                    elif isinstance(value, (dict, list)):
                        extract_recursive(value, depth + 1)
            elif isinstance(obj, list):
                for item in obj[:10]:  # Limit list processing
                    extract_recursive(item, depth + 1)
                    
        extract_recursive(payload)
        return text_parts
        
    def _filter_and_rank_words(self, words: List[str], max_words: int) -> List[str]:
        """Filter and rank words by relevance"""
        if not words:
            return []
            
        word_counts = Counter(words)
        
        # Filter out overly common and rare words
        filtered_words = {
            word: count for word, count in word_counts.items()
            if self.min_frequency <= count <= self.max_common_word_freq
            and word not in self.common_words
            and len(word) >= self.min_word_length
            and len(word) <= self.max_word_length
        }
        
        # Rank by frequency but prefer medium-frequency words for bug bounty
        ranked_words = sorted(filtered_words.items(), 
                            key=lambda x: min(x[1], 100),  # Cap frequency impact
                            reverse=True)
        
        return [word for word, _ in ranked_words[:max_words]]
        
    async def save_wordlists(self, wordlists: Dict[str, List[str]], 
                           output_dir: Path, prefix: str = "github_wordlist") -> Dict[str, str]:
        """Save wordlists to files"""
        output_dir = Path(output_dir)
        output_dir.mkdir(exist_ok=True)
        
        saved_files = {}
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        
        for category, words in wordlists.items():
            if words:
                filename = f"{prefix}_{category}_{timestamp}.txt"
                filepath = output_dir / filename
                
                with open(filepath, 'w', encoding='utf-8') as f:
                    for word in words:
                        f.write(f"{word}\n")
                        
                saved_files[category] = str(filepath)
                self.logger.info(f"Saved {len(words)} {category} words to {filepath}")
                
        return saved_files


async def main():
    """CLI interface for wordlist generator"""
    import argparse
    
    parser = argparse.ArgumentParser(description='Intelligent Wordlist Generator')
    parser.add_argument('command', choices=['comprehensive', 'targeted'], 
                       help='Generation mode')
    parser.add_argument('--target-domain', help='Target domain for focused generation')
    parser.add_argument('--domains', nargs='+', help='Target domains for filtering')
    parser.add_argument('--technologies', nargs='+', help='Technology filter')
    parser.add_argument('--days', type=int, default=30, help='Days back to analyze')
    parser.add_argument('--max-words', type=int, default=10000, help='Maximum words per category')
    parser.add_argument('--output-dir', default='./wordlists', help='Output directory')
    
    args = parser.parse_args()
    
    logging.basicConfig(level=logging.INFO)
    config = Config()
    
    async with WordlistGenerator(config) as generator:
        if args.command == 'comprehensive':
            wordlists = await generator.generate_comprehensive_wordlist(
                target_domains=args.domains,
                technology_filter=args.technologies,
                days_back=args.days,
                max_words=args.max_words
            )
            
            if wordlists:
                saved_files = await generator.save_wordlists(
                    wordlists, Path(args.output_dir), "comprehensive"
                )
                print(f"Comprehensive wordlists saved: {saved_files}")
                
        elif args.command == 'targeted':
            if not args.target_domain:
                print("--target-domain is required for targeted generation")
                return
                
            words = await generator.generate_targeted_wordlist(
                args.target_domain, args.max_words
            )
            
            if words:
                wordlists = {'targeted': words}
                saved_files = await generator.save_wordlists(
                    wordlists, Path(args.output_dir), f"targeted_{args.target_domain}"
                )
                print(f"Targeted wordlist saved: {saved_files}")


if __name__ == '__main__':
    asyncio.run(main())
