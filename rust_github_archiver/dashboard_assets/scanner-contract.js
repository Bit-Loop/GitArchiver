(function (global) {
    const namespace = global.GitHubArchiverDashboard = global.GitHubArchiverDashboard || {};

    /**
     * @typedef {Object} ScannerMetricsContract
     * @property {string} status
     * @property {number} activeScans
     * @property {number} filesProcessed
     * @property {number} eventsProcessed
     * @property {number} processingRatePerMinute
     * @property {number} secretsFound
     * @property {number} queuePending
     * @property {number} queueProcessing
     * @property {number} queueFailed
     * @property {number} queueCompletedLastHour
     * @property {?number} oldestPendingAgeSeconds
     * @property {string|null} timestamp
     * @property {boolean} ready
     * @property {string[]} issues
     */

    function toFiniteNumber(value, fallback = 0) {
        const parsed = Number(value);
        return Number.isFinite(parsed) ? parsed : fallback;
    }

    /**
     * Normalize `/api/v1/scanner/metrics` into the dashboard contract.
     * @param {Object} payload
     * @returns {ScannerMetricsContract}
     */
    function normalizeScannerMetricsResponse(payload = {}) {
        return {
            status: typeof payload.status === 'string' ? payload.status : 'unknown',
            activeScans: toFiniteNumber(payload.active_scans),
            filesProcessed: toFiniteNumber(payload.files_processed),
            eventsProcessed: toFiniteNumber(payload.events_processed),
            processingRatePerMinute: toFiniteNumber(payload.processing_rate_per_minute),
            secretsFound: toFiniteNumber(payload.secrets_found),
            queuePending: toFiniteNumber(payload.queue_pending),
            queueProcessing: toFiniteNumber(payload.queue_processing),
            queueFailed: toFiniteNumber(payload.queue_failed),
            queueCompletedLastHour: toFiniteNumber(payload.queue_completed_last_hour),
            oldestPendingAgeSeconds: payload.oldest_pending_age_seconds == null
                ? null
                : toFiniteNumber(payload.oldest_pending_age_seconds, null),
            timestamp: typeof payload.timestamp === 'string' ? payload.timestamp : null,
            ready: Boolean(payload.ready),
            issues: Array.isArray(payload.issues) ? payload.issues.filter(Boolean) : []
        };
    }

    /**
     * Normalize scanner runtime state used by the dashboard controls.
     * @param {ScannerMetricsContract} metrics
     * @returns {Object}
     */
    function deriveScannerRuntimeState(metrics) {
        let status = metrics.status;
        if (status === 'queued') {
            status = 'running';
        } else if (status === 'attention') {
            status = 'attention';
        } else if (status !== 'running') {
            status = 'stopped';
        }

        return {
            status,
            processing_rate: metrics.processingRatePerMinute,
            files_processed: metrics.filesProcessed,
            events_processed: metrics.eventsProcessed,
            last_updated: metrics.timestamp,
            issues: metrics.issues,
            ready: metrics.ready
        };
    }

    namespace.scannerContract = {
        normalizeScannerMetricsResponse,
        deriveScannerRuntimeState
    };
})(window);
