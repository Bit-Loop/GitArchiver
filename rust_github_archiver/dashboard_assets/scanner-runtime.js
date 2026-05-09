(function (global) {
    const namespace = global.GitHubArchiverDashboard = global.GitHubArchiverDashboard || {};

    namespace.createScannerRuntimeController = function createScannerRuntimeController(deps) {
        function setToggleBusy(isBusy) {
            const toggle = document.getElementById('scannerControlButton');
            const card = document.getElementById('scannerControlCard');
            if (toggle) {
                toggle.disabled = isBusy;
                toggle.classList.toggle('busy', isBusy);
            }
            if (card) {
                card.classList.toggle('scanner-control-busy', isBusy);
            }
        }

        function setControlState(statusPayload = {}) {
            const toggle = document.getElementById('scannerControlButton');
            const statusChip = document.getElementById('scannerToggleStatus');
            const hint = document.getElementById('scannerToggleHint');
            const meta = document.getElementById('scannerToggleMeta');
            const issuesEl = document.getElementById('scannerIssues');
            if (!statusChip) {
                return;
            }

            const status = statusPayload.status || 'stopped';
            deps.state.latestStatus = status;

            if (toggle && !deps.state.toggleLocked) {
                toggle.textContent = status === 'running'
                    ? '⏸️ Pause Scanner'
                    : status === 'paused'
                        ? '▶️ Resume Scanner'
                        : '▶️ Start Scanner';
                toggle.disabled = false;
            }

            const chipClasses = {
                running: 'scanner-chip scanner-chip-active',
                paused: 'scanner-chip scanner-chip-paused',
                stopped: 'scanner-chip scanner-chip-idle',
                attention: 'scanner-chip scanner-chip-error',
                error: 'scanner-chip scanner-chip-error',
                unknown: 'scanner-chip scanner-chip-idle'
            };
            statusChip.className = chipClasses[status] || chipClasses.unknown;
            statusChip.textContent = status === 'running'
                ? 'Scanner Running'
                : status === 'paused'
                    ? 'Scanner Paused'
                    : status === 'attention'
                        ? 'Scanner Attention'
                        : status === 'error'
                            ? 'Scanner Error'
                            : 'Scanner Stopped';

            if (hint) {
                if (status === 'running') {
                    const rate = Number(statusPayload.processing_rate);
                    hint.textContent = `Active scan • ${
                        Number.isFinite(rate) ? `${rate.toFixed(1)} files/min` : 'collecting metrics'
                    }`;
                } else if (status === 'paused') {
                    hint.textContent = 'Paused — press resume to continue';
                } else if (status === 'attention') {
                    hint.textContent = 'Issues detected — see details below';
                } else if (status === 'stopped') {
                    hint.textContent = 'Idle — press start to begin scanning';
                } else {
                    hint.textContent = 'Unable to reach scanner';
                }
            }

            if (meta) {
                const files = statusPayload.files_processed ?? 0;
                const events = statusPayload.events_processed ?? 0;
                const lastUpdated = statusPayload.last_updated
                    ? new Date(statusPayload.last_updated).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
                    : 'never';
                meta.textContent = `Files ${files} • Events ${events} • Updated ${lastUpdated}`;
            }

            if (issuesEl) {
                const issues = Array.isArray(statusPayload.issues) ? statusPayload.issues : [];
                deps.state.latestIssues = issues;
                issuesEl.innerHTML = issues.length
                    ? issues.slice(0, 3).map((msg) => `<div>${msg}</div>`).join('')
                    : '';
            }
        }

        async function handleToggle() {
            const controlButton = document.getElementById('scannerControlButton');
            if (!controlButton) {
                return;
            }

            if (!deps.isAuthenticated() || !deps.getAuthToken()) {
                deps.showAlert('Login required to control the scanner', 'warning');
                return;
            }

            if (deps.state.toggleLocked) {
                return;
            }

            deps.state.toggleLocked = true;
            setToggleBusy(true);

            const action = deps.state.latestStatus === 'running'
                ? 'pause'
                : deps.state.latestStatus === 'paused'
                    ? 'resume'
                    : 'start';

            const messageMap = {
                start: 'Starting scanner...',
                resume: 'Resuming scanner...',
                pause: 'Pausing scanner...'
            };

            try {
                deps.showAlert(messageMap[action], action === 'pause' ? 'warning' : 'info');
                const response = await deps.makeAuthenticatedRequest('/api/v1/scraper/control', {
                    method: 'POST',
                    body: JSON.stringify({ action })
                });
                const data = await deps.parseJsonSafely(response);

                if (response.ok && data.status === 'success') {
                    deps.showAlert(
                        data.message || (action === 'pause' ? 'Scanner paused' : 'Scanner running'),
                        'success'
                    );
                    await deps.updateSecretStats();
                    await deps.updateSecretTrends(deps.getSecretTrendPeriod(), { force: true });
                    await deps.updateScraperStatus();
                    await deps.updateScannerMetrics();
                } else {
                    throw new Error(data.message || 'Failed to update scanner state');
                }
            } catch (error) {
                console.error('Scanner toggle error:', error);
                deps.showAlert(error.message || 'Failed to update scanner state', 'danger');
            } finally {
                deps.state.toggleLocked = false;
                setToggleBusy(false);
            }
        }

        return {
            handleToggle,
            setToggleBusy,
            setControlState
        };
    };
})(window);
