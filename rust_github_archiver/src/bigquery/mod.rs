use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc, NaiveDate};
use gcp_bigquery_client::{Client, model::query_request::QueryRequest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn, error, debug};

/// BigQuery client for scanning GitHub Archive data
pub struct BigQueryScanner {
    client: Client,
    project_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ZeroCommitEvent {
    pub id: String,
    pub event_type: String,
    pub created_at: DateTime<Utc>,
    pub repo_name: String,
    pub repo_id: i64,
    pub actor_login: String,
    pub actor_id: i64,
    pub before_commit: String,
    pub after_commit: String,
    pub ref_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepositoryFilter {
    pub organizations: Vec<String>,
    pub users: Vec<String>,
    pub repositories: Vec<String>,
}

impl Default for RepositoryFilter {
    fn default() -> Self {
        Self {
            organizations: Vec::new(),
            users: Vec::new(),
            repositories: Vec::new(),
        }
    }
}

impl BigQueryScanner {
    /// Create a new BigQuery scanner with service account authentication
    pub async fn new(service_account_key_path: &str, project_id: String) -> Result<Self> {
        info!("Initializing BigQuery client with project: {}", project_id);
        
        let client = Client::from_service_account_key_file(service_account_key_path).await
            .map_err(|e| anyhow!("Failed to create BigQuery client: {}", e))?;
        
        Ok(Self {
            client,
            project_id,
        })
    }

    /// Create a new BigQuery scanner with application default credentials
    pub async fn new_with_default_credentials(project_id: String) -> Result<Self> {
        info!("Initializing BigQuery client with default credentials for project: {}", project_id);
        
        let client = Client::from_application_default_credentials().await
            .map_err(|e| anyhow!("Failed to create BigQuery client with default credentials: {}", e))?;
        
        Ok(Self {
            client,
            project_id,
        })
    }

    /// Query GitHub Archive for zero-commit PushEvents
    pub async fn scan_zero_commit_events(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
        filter: &RepositoryFilter,
        limit: Option<i64>,
    ) -> Result<Vec<ZeroCommitEvent>> {
        info!("Scanning zero-commit events from {} to {}", start_date, end_date);
        
        let query = self.build_zero_commit_query(start_date, end_date, filter, limit);
        debug!("BigQuery SQL: {}", query);
        
        let mut query_request = QueryRequest::new(query);
        query_request.max_results = limit.map(|l| l as u32);
        query_request.use_legacy_sql = Some(false);
        
        let mut response = self.client
            .job()
            .query(&self.project_id, query_request)
            .await
            .map_err(|e| anyhow!("BigQuery query failed: {}", e))?;
        
        let mut events = Vec::new();
        let mut result_set = gcp_bigquery_client::model::query_response::ResultSet::new_from_query_response(response);
        
        while result_set.next_row() {
            let event = ZeroCommitEvent {
                id: result_set.get_string_by_name("id")?.unwrap_or_default(),
                event_type: result_set.get_string_by_name("type")?.unwrap_or_default(),
                created_at: result_set.get_datetime_by_name("created_at")?
                    .ok_or_else(|| anyhow!("Missing created_at field"))?
                    .and_utc(),
                repo_name: result_set.get_string_by_name("repo_name")?.unwrap_or_default(),
                repo_id: result_set.get_i64_by_name("repo_id")?.unwrap_or(0),
                actor_login: result_set.get_string_by_name("actor_login")?.unwrap_or_default(),
                actor_id: result_set.get_i64_by_name("actor_id")?.unwrap_or(0),
                before_commit: result_set.get_string_by_name("before_commit")?.unwrap_or_default(),
                after_commit: result_set.get_string_by_name("after_commit")?.unwrap_or_default(),
                ref_name: result_set.get_string_by_name("ref")?.unwrap_or_default(),
            };
            
            if !event.before_commit.is_empty() && event.before_commit != "0000000000000000000000000000000000000000" {
                events.push(event);
            }
        }
        
        info!("Found {} zero-commit events", events.len());
        Ok(events)
    }

    /// Build the BigQuery SQL for finding zero-commit events
    fn build_zero_commit_query(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
        filter: &RepositoryFilter,
        limit: Option<i64>,
    ) -> String {
        let mut where_clauses = vec![
            "type = 'PushEvent'".to_string(),
            "JSON_EXTRACT_ARRAY(payload, '$.commits') = []".to_string(), // Zero commits
            "JSON_EXTRACT_SCALAR(payload, '$.before') IS NOT NULL".to_string(),
            "JSON_EXTRACT_SCALAR(payload, '$.before') != ''".to_string(),
            "JSON_EXTRACT_SCALAR(payload, '$.before') != '0000000000000000000000000000000000000000'".to_string(),
            format!("DATE(created_at) >= '{}'", start_date),
            format!("DATE(created_at) <= '{}'", end_date),
        ];

        // Add repository filters
        if !filter.organizations.is_empty() || !filter.users.is_empty() || !filter.repositories.is_empty() {
            let mut repo_filters = Vec::new();
            
            if !filter.organizations.is_empty() {
                let orgs = filter.organizations.iter()
                    .map(|org| format!("'{}'", org.replace("'", "''")))
                    .collect::<Vec<_>>()
                    .join(", ");
                repo_filters.push(format!("SPLIT(repo.name, '/')[OFFSET(0)] IN ({})", orgs));
            }
            
            if !filter.users.is_empty() {
                let users = filter.users.iter()
                    .map(|user| format!("'{}'", user.replace("'", "''")))
                    .collect::<Vec<_>>()
                    .join(", ");
                repo_filters.push(format!("SPLIT(repo.name, '/')[OFFSET(0)] IN ({})", users));
            }
            
            if !filter.repositories.is_empty() {
                let repos = filter.repositories.iter()
                    .map(|repo| format!("'{}'", repo.replace("'", "''")))
                    .collect::<Vec<_>>()
                    .join(", ");
                repo_filters.push(format!("repo.name IN ({})", repos));
            }
            
            if !repo_filters.is_empty() {
                where_clauses.push(format!("({})", repo_filters.join(" OR ")));
            }
        }

        let limit_clause = if let Some(l) = limit {
            format!("LIMIT {}", l)
        } else {
            String::new()
        };

        format!(
            r#"
SELECT 
    id,
    type,
    created_at,
    repo.name as repo_name,
    repo.id as repo_id,
    actor.login as actor_login,
    actor.id as actor_id,
    JSON_EXTRACT_SCALAR(payload, '$.before') as before_commit,
    JSON_EXTRACT_SCALAR(payload, '$.after') as after_commit,
    JSON_EXTRACT_SCALAR(payload, '$.ref') as ref
FROM `githubarchive.month.*`
WHERE {}
ORDER BY created_at DESC
{}
            "#,
            where_clauses.join(" AND "),
            limit_clause
        )
    }

    /// Get available GitHub Archive table dates
    pub async fn get_available_dates(&self) -> Result<Vec<NaiveDate>> {
        info!("Fetching available GitHub Archive dates");
        
        let query = r#"
SELECT 
    DISTINCT DATE(_TABLE_SUFFIX) as table_date
FROM `githubarchive.month.*`
WHERE _TABLE_SUFFIX IS NOT NULL
ORDER BY table_date DESC
        "#;
        
        let query_request = QueryRequest::new(query.to_string());
        let mut response = self.client
            .job()
            .query(&self.project_id, query_request)
            .await
            .map_err(|e| anyhow!("Failed to query available dates: {}", e))?;
        
        let mut dates = Vec::new();
        let mut result_set = gcp_bigquery_client::model::query_response::ResultSet::new_from_query_response(response);
        
        while result_set.next_row() {
            if let Some(date_str) = result_set.get_string_by_name("table_date")? {
                if let Ok(date) = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
                    dates.push(date);
                }
            }
        }
        
        info!("Found {} available dates", dates.len());
        Ok(dates)
    }

    /// Get statistics about PushEvents in a date range
    pub async fn get_push_event_stats(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<HashMap<String, i64>> {
        info!("Getting PushEvent statistics from {} to {}", start_date, end_date);
        
        let query = format!(
            r#"
SELECT 
    COUNT(*) as total_push_events,
    COUNT(CASE WHEN JSON_EXTRACT_ARRAY(payload, '$.commits') = [] THEN 1 END) as zero_commit_events,
    COUNT(CASE WHEN JSON_EXTRACT_ARRAY(payload, '$.commits') != [] THEN 1 END) as normal_push_events,
    COUNT(DISTINCT repo.name) as unique_repositories,
    COUNT(DISTINCT actor.login) as unique_actors
FROM `githubarchive.month.*`
WHERE type = 'PushEvent'
    AND DATE(created_at) >= '{}'
    AND DATE(created_at) <= '{}'
            "#,
            start_date, end_date
        );
        
        let query_request = QueryRequest::new(query);
        let mut response = self.client
            .job()
            .query(&self.project_id, query_request)
            .await
            .map_err(|e| anyhow!("Failed to query PushEvent stats: {}", e))?;
        
        let mut stats = HashMap::new();
        let mut result_set = gcp_bigquery_client::model::query_response::ResultSet::new_from_query_response(response);
        
        if result_set.next_row() {
            stats.insert("total_push_events".to_string(), result_set.get_i64_by_name("total_push_events")?.unwrap_or(0));
            stats.insert("zero_commit_events".to_string(), result_set.get_i64_by_name("zero_commit_events")?.unwrap_or(0));
            stats.insert("normal_push_events".to_string(), result_set.get_i64_by_name("normal_push_events")?.unwrap_or(0));
            stats.insert("unique_repositories".to_string(), result_set.get_i64_by_name("unique_repositories")?.unwrap_or(0));
            stats.insert("unique_actors".to_string(), result_set.get_i64_by_name("unique_actors")?.unwrap_or(0));
        }
        
        info!("PushEvent stats: {:?}", stats);
        Ok(stats)
    }

    /// Scan for zero-commit events by organization
    pub async fn scan_organization_zero_commits(
        &self,
        organization: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
        limit: Option<i64>,
    ) -> Result<Vec<ZeroCommitEvent>> {
        let filter = RepositoryFilter {
            organizations: vec![organization.to_string()],
            ..Default::default()
        };
        
        self.scan_zero_commit_events(start_date, end_date, &filter, limit).await
    }

    /// Scan for zero-commit events by user
    pub async fn scan_user_zero_commits(
        &self,
        user: &str,
        start_date: NaiveDate,
        end_date: NaiveDate,
        limit: Option<i64>,
    ) -> Result<Vec<ZeroCommitEvent>> {
        let filter = RepositoryFilter {
            users: vec![user.to_string()],
            ..Default::default()
        };
        
        self.scan_zero_commit_events(start_date, end_date, &filter, limit).await
    }

    /// Extract unique repository names from zero-commit events
    pub fn extract_repositories(events: &[ZeroCommitEvent]) -> Vec<String> {
        let mut repos: Vec<String> = events
            .iter()
            .map(|e| e.repo_name.clone())
            .collect();
        repos.sort();
        repos.dedup();
        repos
    }

    /// Extract unique before commit hashes from zero-commit events
    pub fn extract_before_commits(events: &[ZeroCommitEvent]) -> Vec<String> {
        let mut commits: Vec<String> = events
            .iter()
            .filter(|e| !e.before_commit.is_empty() && e.before_commit != "0000000000000000000000000000000000000000")
            .map(|e| e.before_commit.clone())
            .collect();
        commits.sort();
        commits.dedup();
        commits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_repository_filter_default() {
        let filter = RepositoryFilter::default();
        assert!(filter.organizations.is_empty());
        assert!(filter.users.is_empty());
        assert!(filter.repositories.is_empty());
    }

    #[test]
    fn test_extract_repositories() {
        let events = vec![
            ZeroCommitEvent {
                id: "1".to_string(),
                event_type: "PushEvent".to_string(),
                created_at: chrono::Utc::now(),
                repo_name: "org/repo1".to_string(),
                repo_id: 1,
                actor_login: "user1".to_string(),
                actor_id: 1,
                before_commit: "abc123".to_string(),
                after_commit: "def456".to_string(),
                ref_name: "refs/heads/main".to_string(),
            },
            ZeroCommitEvent {
                id: "2".to_string(),
                event_type: "PushEvent".to_string(),
                created_at: chrono::Utc::now(),
                repo_name: "org/repo2".to_string(),
                repo_id: 2,
                actor_login: "user2".to_string(),
                actor_id: 2,
                before_commit: "xyz789".to_string(),
                after_commit: "ghi012".to_string(),
                ref_name: "refs/heads/main".to_string(),
            },
            ZeroCommitEvent {
                id: "3".to_string(),
                event_type: "PushEvent".to_string(),
                created_at: chrono::Utc::now(),
                repo_name: "org/repo1".to_string(), // Duplicate
                repo_id: 1,
                actor_login: "user3".to_string(),
                actor_id: 3,
                before_commit: "mno345".to_string(),
                after_commit: "pqr678".to_string(),
                ref_name: "refs/heads/develop".to_string(),
            },
        ];

        let repos = BigQueryScanner::extract_repositories(&events);
        assert_eq!(repos.len(), 2);
        assert!(repos.contains(&"org/repo1".to_string()));
        assert!(repos.contains(&"org/repo2".to_string()));
    }

    #[test]
    fn test_extract_before_commits() {
        let events = vec![
            ZeroCommitEvent {
                id: "1".to_string(),
                event_type: "PushEvent".to_string(),
                created_at: chrono::Utc::now(),
                repo_name: "org/repo1".to_string(),
                repo_id: 1,
                actor_login: "user1".to_string(),
                actor_id: 1,
                before_commit: "abc123".to_string(),
                after_commit: "def456".to_string(),
                ref_name: "refs/heads/main".to_string(),
            },
            ZeroCommitEvent {
                id: "2".to_string(),
                event_type: "PushEvent".to_string(),
                created_at: chrono::Utc::now(),
                repo_name: "org/repo2".to_string(),
                repo_id: 2,
                actor_login: "user2".to_string(),
                actor_id: 2,
                before_commit: "0000000000000000000000000000000000000000".to_string(), // Should be filtered
                after_commit: "ghi012".to_string(),
                ref_name: "refs/heads/main".to_string(),
            },
            ZeroCommitEvent {
                id: "3".to_string(),
                event_type: "PushEvent".to_string(),
                created_at: chrono::Utc::now(),
                repo_name: "org/repo3".to_string(),
                repo_id: 3,
                actor_login: "user3".to_string(),
                actor_id: 3,
                before_commit: "xyz789".to_string(),
                after_commit: "mno345".to_string(),
                ref_name: "refs/heads/develop".to_string(),
            },
        ];

        let commits = BigQueryScanner::extract_before_commits(&events);
        assert_eq!(commits.len(), 2);
        assert!(commits.contains(&"abc123".to_string()));
        assert!(commits.contains(&"xyz789".to_string()));
        assert!(!commits.contains(&"0000000000000000000000000000000000000000".to_string()));
    }
}
