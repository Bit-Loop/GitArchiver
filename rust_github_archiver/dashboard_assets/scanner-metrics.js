(function (global) {
    const namespace = global.GitHubArchiverDashboard = global.GitHubArchiverDashboard || {};
    const contract = namespace.scannerContract;

    namespace.createScannerMetricsController = function createScannerMetricsController(deps) {
        function setUnavailable(value = '—', statusMessage = null) {
            const metricFields = [
                'scannerActiveScans',
                'scannerProcessingRate',
                'scannerFilesProcessed',
                'scannerEventsProcessed',
                'scannerSecretsFound',
                'scannerQueuePending',
                'scannerQueueProcessing',
                'scannerQueueCompleted',
                'scannerQueueFailed',
                'scannerOldestPending'
            ];

            metricFields.forEach((id) => {
                const el = document.getElementById(id);
                if (el) {
                    el.textContent = value;
                }
            });

            if (statusMessage) {
                const statusEl = document.getElementById('scannerMetricsStatus');
                if (statusEl) {
                    statusEl.textContent = statusMessage;
                }
            }
        }

        async function updateMetrics() {
            const statusElement = document.getElementById('scannerMetricsStatus');
            if (!statusElement) {
                return;
            }

            if (!deps.getAuthToken()) {
                setUnavailable('—', 'Login to load data');
                return;
            }

            try {
                statusElement.textContent = 'Refreshing...';
                const response = await deps.makeAuthenticatedRequest('/api/v1/scanner/metrics');
                const data = await deps.parseJsonSafely(response);

                if (response.status === 401) {
                    deps.handleUnauthorized();
                    setUnavailable('—', 'Login to load data');
                    return;
                }

                if (!response.ok) {
                    throw new Error(data.message || 'Failed to fetch scanner metrics');
                }

                const metrics = contract.normalizeScannerMetricsResponse(data);
                const numericFields = {
                    scannerActiveScans: metrics.activeScans,
                    scannerFilesProcessed: metrics.filesProcessed,
                    scannerEventsProcessed: metrics.eventsProcessed,
                    scannerSecretsFound: metrics.secretsFound,
                    scannerQueuePending: metrics.queuePending,
                    scannerQueueProcessing: metrics.queueProcessing,
                    scannerQueueCompleted: metrics.queueCompletedLastHour,
                    scannerQueueFailed: metrics.queueFailed
                };

                Object.entries(numericFields).forEach(([id, value]) => {
                    const el = document.getElementById(id);
                    if (el) {
                        el.textContent = deps.formatNumber(value);
                    }
                });

                const rateEl = document.getElementById('scannerProcessingRate');
                if (rateEl) {
                    rateEl.textContent = `${metrics.processingRatePerMinute.toFixed(1)}/min`;
                }

                const oldestEl = document.getElementById('scannerOldestPending');
                if (oldestEl) {
                    oldestEl.textContent = metrics.oldestPendingAgeSeconds == null
                        ? '—'
                        : deps.formatDurationFromSeconds(metrics.oldestPendingAgeSeconds);
                }

                const timestamp = metrics.timestamp ? new Date(metrics.timestamp) : new Date();
                statusElement.textContent = metrics.issues.length
                    ? metrics.issues[0]
                    : `Updated ${timestamp.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`;

                deps.setControlState(contract.deriveScannerRuntimeState(metrics));
            } catch (error) {
                console.error('Failed to update scanner metrics:', error);
                setUnavailable('—');
                statusElement.textContent = 'Error loading metrics';
            }
        }

        return {
            setUnavailable,
            updateMetrics
        };
    };
})(window);
