use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::sync::Arc;

use super::database::{
    Database, DatabaseHealth, DatabaseStatistics, EventPreview, EventScanTarget,
    PushEventQueueInsert, ScanQueueStats, SecretDashboardData, SecretDetectionFilter,
    SecretDetectionRow, SecretOverviewMetrics, SecretTrendSample,
};
use crate::scanning::persistence::ScanPersistenceAdapter;
use crate::scanning::CompletedScan;

#[derive(Clone)]
pub struct PersistenceService {
    database: Arc<Database>,
}

impl PersistenceService {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    pub fn database(&self) -> &Arc<Database> {
        &self.database
    }

    pub async fn health_status(&self) -> DatabaseHealth {
        self.database.health_status().await
    }

    pub async fn database_statistics(&self) -> Result<DatabaseStatistics> {
        self.database.get_database_statistics().await
    }

    pub async fn secret_overview_metrics(&self) -> Result<SecretOverviewMetrics> {
        self.database.get_secret_overview_metrics().await
    }

    pub async fn secret_dashboard_data(
        &self,
        repo_limit: i64,
        recent_limit: i64,
    ) -> Result<SecretDashboardData> {
        self.database
            .get_secret_dashboard_data(repo_limit, recent_limit)
            .await
    }

    pub async fn secret_trend_samples(
        &self,
        start_time: DateTime<Utc>,
    ) -> Result<Vec<SecretTrendSample>> {
        self.database.get_secret_trend_samples(start_time).await
    }

    pub async fn secret_detections(
        &self,
        filter: SecretDetectionFilter,
    ) -> Result<Vec<SecretDetectionRow>> {
        self.database.get_secret_detections(filter).await
    }

    pub async fn total_event_count(&self) -> Result<i64> {
        self.database.get_total_event_count().await
    }

    pub async fn recent_events(&self, limit: i64) -> Result<Vec<EventPreview>> {
        self.database.get_recent_events(limit).await
    }

    pub async fn search_events(&self, query: &str, limit: i64) -> Result<Vec<EventPreview>> {
        self.database.search_events(query, limit).await
    }

    pub async fn insert_events_batch(&self, events: Vec<Value>, filename: &str) -> Result<i64> {
        self.database.insert_events_batch(events, filename).await
    }

    pub async fn enqueue_push_event_from_monitor(
        &self,
        record: PushEventQueueInsert,
    ) -> Result<()> {
        self.database.enqueue_push_event_from_monitor(record).await
    }

    pub async fn repository_push_events(
        &self,
        repository: &str,
        limit: usize,
    ) -> Result<Vec<EventScanTarget>> {
        self.database
            .get_push_events_for_repository(repository, limit as i64)
            .await
    }

    pub async fn claim_pending_push_events(
        &self,
        limit: i64,
        worker_id: &str,
    ) -> Result<Vec<EventScanTarget>> {
        self.database
            .claim_pending_push_events(limit, worker_id)
            .await
    }

    pub async fn release_push_events(&self, event_ids: &[i64]) -> Result<()> {
        self.database.release_push_events(event_ids).await
    }

    pub async fn mark_push_events_completed(&self, event_ids: &[i64]) -> Result<()> {
        self.database.mark_push_events_completed(event_ids).await
    }

    pub async fn mark_push_events_failed(
        &self,
        event_ids: &[i64],
        error: Option<&str>,
    ) -> Result<()> {
        self.database
            .mark_push_events_failed(event_ids, error)
            .await
    }

    pub async fn scan_queue_stats(&self) -> Result<ScanQueueStats> {
        self.database.get_scan_queue_stats().await
    }

    pub async fn persist_scan(
        &self,
        scan: &CompletedScan,
        failure_reason: Option<&str>,
    ) -> Result<()> {
        let adapter = ScanPersistenceAdapter::new(self.database.as_ref());
        adapter.persist_scan(scan, failure_reason).await
    }
}
