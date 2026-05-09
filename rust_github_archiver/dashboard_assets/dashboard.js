        const state = {
            token: sessionStorage.getItem('authToken') || '',
            user: null,
            scannerStatus: 'unknown',
            researchCandidates: [],
            researchFindings: [],
            selectedResearchFinding: null,
            refreshTimer: null
        };

        const $ = (id) => document.getElementById(id);

        function setText(id, value) {
            const el = $(id);
            if (el) {
                el.textContent = value;
            }
        }

        function clearElement(element) {
            element.replaceChildren();
            return element;
        }

        function emptyState(text) {
            const node = document.createElement('div');
            node.className = 'empty';
            node.textContent = text;
            return node;
        }

        function tableCell(text, className = '') {
            const cell = document.createElement('td');
            if (className) {
                cell.className = className;
            }
            cell.textContent = text;
            return cell;
        }

        function actionButton(text, onClick, primary = false) {
            const button = document.createElement('button');
            button.className = primary ? 'btn primary' : 'btn';
            button.type = 'button';
            button.textContent = text;
            button.addEventListener('click', onClick);
            return button;
        }

        function formatNumber(value) {
            const number = Number(value);
            return Number.isFinite(number) ? number.toLocaleString() : '0';
        }

        function formatPercent(value) {
            const number = Number(value);
            return Number.isFinite(number) ? `${number.toFixed(1)}%` : '0.0%';
        }

        function formatTime(value) {
            if (!value) {
                return 'n/a';
            }
            const date = new Date(value);
            return Number.isNaN(date.getTime())
                ? 'n/a'
                : date.toLocaleString([], { dateStyle: 'short', timeStyle: 'short' });
        }

        function formatDuration(seconds) {
            const value = Number(seconds);
            if (!Number.isFinite(value) || value < 0) {
                return 'n/a';
            }
            if (value < 60) {
                return `${Math.round(value)}s`;
            }
            if (value < 3600) {
                return `${Math.round(value / 60)}m`;
            }
            return `${Math.round(value / 3600)}h`;
        }

        function authHeaders() {
            return state.token ? { Authorization: `Bearer ${state.token}` } : {};
        }

        async function readJson(response) {
            const text = await response.text();
            if (!text) {
                return {};
            }
            try {
                return JSON.parse(text);
            } catch (error) {
                return { error: text };
            }
        }

        async function api(path, options = {}) {
            const headers = {
                Accept: 'application/json',
                ...(options.body ? { 'Content-Type': 'application/json' } : {}),
                ...(options.auth ? authHeaders() : {}),
                ...(options.headers || {})
            };
            const response = await fetch(path, { ...options, headers });
            const data = await readJson(response);
            if (!response.ok) {
                const message = data.message || data.error || `${response.status} ${response.statusText}`;
                const error = new Error(message);
                error.status = response.status;
                error.payload = data;
                throw error;
            }
            return data;
        }

        function setChip(id, text, tone) {
            const chip = $(id);
            if (!chip) {
                return;
            }
            chip.classList.remove('good', 'warn', 'bad');
            if (tone) {
                chip.classList.add(tone);
            }
            const label = chip.querySelector('span:last-child');
            if (label) {
                label.textContent = text;
            }
        }

        function toast(message, type = 'info') {
            const host = $('toast');
            const item = document.createElement('div');
            item.className = `toast-item ${type}`;
            item.textContent = message;
            host.appendChild(item);
            setTimeout(() => item.remove(), 4200);
        }

        function classifyStatus(value) {
            const status = String(value || '').toLowerCase();
            if (['running', 'healthy', 'ok', 'ready', 'connected'].includes(status)) {
                return 'good';
            }
            if (['idle', 'paused', 'queued', 'warning', 'degraded'].includes(status)) {
                return 'warn';
            }
            if (['error', 'failed', 'offline', 'stopped', 'unavailable'].includes(status)) {
                return 'bad';
            }
            return '';
        }

        function showSection(sectionId) {
            document.querySelectorAll('.section').forEach((section) => {
                section.classList.toggle('active', section.id === sectionId);
            });
            document.querySelectorAll('.rail button').forEach((button) => {
                button.classList.toggle('active', button.dataset.section === sectionId);
            });
        }

        function updateAuthUi() {
            const signedIn = Boolean(state.token);
            $('loginButton').hidden = signedIn;
            $('logoutButton').hidden = !signedIn;
            setChip('authChip', signedIn ? (state.user?.username || 'Signed in') : 'Signed out', signedIn ? 'good' : 'warn');
        }

        async function verifyAuth() {
            if (!state.token) {
                updateAuthUi();
                return;
            }
            try {
                const data = await api('/api/auth/verify', { auth: true });
                state.user = { username: data.user, role: data.role };
            } catch (error) {
                state.token = '';
                state.user = null;
                localStorage.removeItem('authToken');
                sessionStorage.removeItem('authToken');
            }
            updateAuthUi();
        }

        async function loadSystemStatus() {
            const data = await api('/api/v1/system/status');
            const status = data.status || 'unknown';
            setText('systemStatus', status);
            setText('systemNote', `${data.hostname || 'host unknown'} | load ${Number(data.load_average || 0).toFixed(2)}`);
            setText('databaseStatus', data.database?.is_connected ? 'Connected' : 'Offline');
            setText('databaseNote', `Connections ${formatNumber(data.database?.connection_count)} | cache ${formatPercent(data.database?.cache_hit_ratio)}`);
            setChip('apiChip', 'API online', data.ready ? 'good' : classifyStatus(status));

            if (data.scraper) {
                state.scannerStatus = data.scraper.status || 'unknown';
                renderScannerFallback(data.scraper);
            }
        }

        function renderScannerFallback(scraper) {
            setText('scannerStatus', scraper.status || 'Unknown');
            setText('navScanner', scraper.status || 'idle');
            setText('filesProcessed', formatNumber(scraper.files_processed));
            setText('eventsProcessed', formatNumber(scraper.events_processed));
            setText('scannerRate', `${Number(scraper.processing_rate || 0).toFixed(1)}/min`);
            setChip('scannerStatusChip', `Scanner ${scraper.status || 'unknown'}`, classifyStatus(scraper.status));
            updateScannerButtons(scraper.status || 'unknown');
        }

        async function loadScannerMetrics() {
            if (!state.token) {
                renderScannerIssues(['Sign in to view queue metrics and control the scanner.']);
                return;
            }

            const data = await api('/api/v1/scanner/metrics', { auth: true });
            state.scannerStatus = data.status || 'unknown';
            setText('scannerStatus', data.status || 'Unknown');
            setText('navScanner', data.status || 'idle');
            setText('activeScans', formatNumber(data.active_scans));
            setText('filesProcessed', formatNumber(data.files_processed));
            setText('eventsProcessed', formatNumber(data.events_processed));
            setText('scannerRate', `${Number(data.processing_rate_per_minute || 0).toFixed(1)}/min`);
            setText('queuePending', formatNumber(data.queue_pending));
            setText('queueNote', `${formatNumber(data.queue_processing)} processing, ${formatNumber(data.queue_failed)} failed`);
            setText('oldestPending', formatDuration(data.oldest_pending_age_seconds));
            setChip('scannerStatusChip', `Scanner ${data.status || 'unknown'}`, classifyStatus(data.status));
            setChip('trufflehogChip', data.trufflehog_available ? 'TruffleHog ready' : 'TruffleHog missing', data.trufflehog_available ? 'good' : 'bad');
            renderScannerIssues(data.issues || []);
            updateScannerButtons(data.status || 'unknown');
        }

        function updateScannerButtons(status) {
            const primary = $('scannerPrimaryButton');
            if (!primary) {
                return;
            }
            if (status === 'running') {
                primary.textContent = 'Pause';
                primary.dataset.action = 'pause';
            } else if (status === 'paused') {
                primary.textContent = 'Resume';
                primary.dataset.action = 'resume';
            } else {
                primary.textContent = 'Start';
                primary.dataset.action = 'start';
            }
        }

        function renderScannerIssues(issues) {
            const host = $('scannerIssues');
            if (!host) {
                return;
            }
            clearElement(host);
            if (!issues.length) {
                host.appendChild(emptyState('No scanner issues reported.'));
                return;
            }
            issues.slice(0, 5).forEach((issue) => {
                const item = document.createElement('div');
                item.className = 'error-box';
                item.textContent = issue;
                host.appendChild(item);
            });
        }

        async function loadOverview() {
            if (!state.token) {
                renderOverview({
                    total_secrets: 0,
                    critical_secrets: 0,
                    high_secrets: 0,
                    verified_secrets: 0,
                    repositories_scanned: 0,
                    severity_distribution: {},
                    top_repositories: []
                });
                return;
            }
            const data = await api('/api/v1/monitoring/overview', { auth: true });
            renderOverview(data);
        }

        function renderOverview(data) {
            setText('secretTotal', formatNumber(data.total_secrets));
            setText('navOverview', formatNumber(data.total_secrets));
            setText('navDetections', formatNumber(data.total_secrets));
            setText('secretNote', `${formatNumber(data.critical_secrets)} critical, ${formatNumber(data.high_secrets)} high`);
            setText('verifiedChip', `${formatNumber(data.verified_secrets)} verified`);
            setText('repoScanChip', `${formatNumber(data.repositories_scanned)} repos`);
            renderSeverityBars(data.severity_distribution || {});
            renderTopRepositories(data.top_repositories || []);
        }

        function renderSeverityBars(distribution) {
            const host = $('severityBars');
            const entries = ['Critical', 'High', 'Medium', 'Low'].map((label) => {
                const value = distribution[label] ?? distribution[label.toLowerCase()] ?? 0;
                return [label, Number(value) || 0];
            });
            const max = Math.max(...entries.map((entry) => entry[1]), 1);
            clearElement(host);
            entries.forEach(([label, value]) => {
                const width = Math.round((value / max) * 100);
                const row = document.createElement('div');
                row.className = 'bar-row';

                const labelNode = document.createElement('span');
                labelNode.textContent = label;

                const track = document.createElement('span');
                track.className = 'bar-track';
                const fill = document.createElement('span');
                fill.className = 'bar-fill';
                fill.style.width = `${width}%`;
                track.appendChild(fill);

                const count = document.createElement('span');
                count.className = 'mono';
                count.textContent = formatNumber(value);

                row.append(labelNode, track, count);
                host.appendChild(row);
            });
        }

        function renderTopRepositories(repositories) {
            const host = $('topRepos');
            clearElement(host);
            if (!repositories.length) {
                host.appendChild(emptyState('No repository risk data available.'));
                return;
            }
            repositories.slice(0, 8).forEach((repo) => {
                const item = document.createElement('div');
                item.className = 'list-item';

                const detail = document.createElement('div');
                const name = document.createElement('strong');
                name.textContent = repo.repository || 'unknown';
                const note = document.createElement('div');
                note.className = 'metric-note';
                note.textContent = `${formatNumber(repo.critical_count)} critical | ${formatNumber(repo.high_count)} high`;
                detail.append(name, note);

                const score = document.createElement('div');
                score.className = 'mono';
                score.textContent = Number(repo.risk_score || 0).toFixed(1);

                item.append(detail, score);
                host.appendChild(item);
            });
        }

        async function loadDetections() {
            if (!state.token) {
                renderDetections([], 'Sign in to view scanner detections.');
                return;
            }
            const params = new URLSearchParams({ limit: '50' });
            const repo = $('repositoryFilter').value.trim();
            const severity = $('severityFilter').value;
            if (repo) {
                params.set('repository', repo);
            }
            if (severity) {
                params.set('severity', severity);
            }
            const data = await api(`/api/v1/scanner/results?${params.toString()}`, { auth: true });
            renderDetections(data.results || []);
        }

        function renderDetections(rows, emptyText = 'No detections match the current filter.') {
            const body = $('detectionsBody');
            clearElement(body);
            if (!rows.length) {
                const row = document.createElement('tr');
                const cell = document.createElement('td');
                cell.colSpan = 6;
                cell.textContent = emptyText;
                row.appendChild(cell);
                body.appendChild(row);
                return;
            }
            rows.forEach((row) => {
                const severity = String(row.severity || '').toLowerCase();
                const tr = document.createElement('tr');
                const cells = [
                    formatTime(row.detected_at),
                    row.repository || 'unknown',
                    row.detector_name || row.detector || 'unknown'
                ];
                cells.forEach((value, index) => {
                    const cell = document.createElement('td');
                    if (index === 1) {
                        cell.className = 'mono';
                    }
                    cell.textContent = value;
                    tr.appendChild(cell);
                });

                const severityCell = document.createElement('td');
                const severityBadge = document.createElement('span');
                severityBadge.className = `severity ${severity}`;
                severityBadge.textContent = row.severity || 'unknown';
                severityCell.appendChild(severityBadge);

                const fileCell = document.createElement('td');
                fileCell.className = 'truncate';
                fileCell.title = row.file_path || '';
                fileCell.textContent = row.file_path || 'n/a';

                const verifiedCell = document.createElement('td');
                verifiedCell.textContent = row.verified ? 'Yes' : 'No';

                tr.append(severityCell, fileCell, verifiedCell);
                body.appendChild(tr);
            });
        }

        async function loadLogs() {
            if (!state.token) {
                renderLogs([], 'Sign in to view system logs.');
                return;
            }
            const params = new URLSearchParams({ page: '1', page_size: '80' });
            const level = $('logLevelFilter').value;
            if (level) {
                params.set('level', level);
            }
            const data = await api(`/api/v1/monitoring/logs?${params.toString()}`, { auth: true });
            renderLogs(data.logs || []);
            setText('navLogs', formatNumber(data.total_count || (data.logs || []).length));
        }

        function renderLogs(logs, emptyText = 'No log entries available.') {
            const host = $('logList');
            clearElement(host);
            if (!logs.length) {
                host.appendChild(emptyState(emptyText));
                return;
            }
            logs.forEach((log) => {
                const line = document.createElement('div');
                line.className = 'log-line';

                const timestamp = document.createElement('span');
                timestamp.className = 'mono';
                timestamp.textContent = formatTime(log.timestamp);

                const level = document.createElement('strong');
                level.textContent = log.level || 'info';

                const message = document.createElement('span');
                message.textContent = log.message || '';

                line.append(timestamp, level, message);
                host.appendChild(line);
            });
        }

        async function loadResearch() {
            if (!state.token) {
                state.researchCandidates = [];
                state.researchFindings = [];
                state.selectedResearchFinding = null;
                renderResearchCandidates([], 'Sign in to view research candidates.');
                renderResearchFindings([], 'Sign in to view research findings.');
                renderResearchDetail(null);
                setText('navResearch', '0');
                setText('researchCandidateCount', '0');
                setText('researchFindingCount', '0');
                return;
            }

            const [candidateData, findingData] = await Promise.all([
                api('/api/research/candidates?limit=30', { auth: true }),
                api('/api/research/findings?limit=50', { auth: true })
            ]);
            state.researchCandidates = candidateData.candidates || [];
            state.researchFindings = findingData.findings || [];
            if (state.selectedResearchFinding) {
                state.selectedResearchFinding = state.researchFindings.find((finding) => finding.id === state.selectedResearchFinding.id) || null;
            }
            renderResearchCandidates(state.researchCandidates);
            renderResearchFindings(state.researchFindings);
            renderResearchDetail(state.selectedResearchFinding);
            setText('navResearch', formatNumber(state.researchFindings.length));
            setText('researchCandidateCount', formatNumber(state.researchCandidates.length));
            setText('researchFindingCount', formatNumber(state.researchFindings.length));
        }

        function renderResearchCandidates(candidates, emptyText = 'No research candidates available.') {
            const body = $('researchCandidatesBody');
            clearElement(body);
            if (!candidates.length) {
                const row = document.createElement('tr');
                const cell = tableCell(emptyText);
                cell.colSpan = 5;
                row.appendChild(cell);
                body.appendChild(row);
                return;
            }
            candidates.forEach((candidate, index) => {
                const row = document.createElement('tr');
                row.append(
                    tableCell(candidate.source_type || 'unknown'),
                    tableCell(candidate.repository || 'unknown', 'mono'),
                    tableCell(candidate.title || 'candidate'),
                    tableCell(candidate.severity || 'informational')
                );
                const action = document.createElement('td');
                action.appendChild(actionButton('Create', () => createResearchFinding(index), true));
                row.appendChild(action);
                body.appendChild(row);
            });
        }

        function renderResearchFindings(findings, emptyText = 'No research findings yet.') {
            const body = $('researchFindingsBody');
            clearElement(body);
            if (!findings.length) {
                const row = document.createElement('tr');
                const cell = tableCell(emptyText);
                cell.colSpan = 4;
                row.appendChild(cell);
                body.appendChild(row);
                return;
            }
            findings.forEach((finding) => {
                const row = document.createElement('tr');
                row.style.cursor = 'pointer';
                row.addEventListener('click', () => selectResearchFinding(finding.id));
                row.append(
                    tableCell(finding.title || 'untitled'),
                    tableCell(finding.status || 'draft'),
                    tableCell(`${formatNumber(finding.readiness_score)}/100`, 'mono'),
                    tableCell(finding.scope_asset || finding.repository || 'not set', 'mono')
                );
                body.appendChild(row);
            });
        }

        function renderResearchDetail(finding) {
            const host = $('researchDetail');
            clearElement(host);
            if (!finding) {
                host.appendChild(emptyState('Select or create a research finding.'));
                setText('researchSelectedScore', '0');
                setText('researchSelectedStatus', 'No finding selected');
                return;
            }
            setText('researchSelectedScore', formatNumber(finding.readiness_score));
            setText('researchSelectedStatus', finding.status || 'draft');
            const title = document.createElement('strong');
            title.textContent = finding.title || 'Research finding';
            const meta = document.createElement('div');
            meta.className = 'metric-note';
            meta.textContent = `${finding.playbook || 'playbook not set'} | ${finding.scope_status || 'scope unknown'} | ${finding.repository || 'asset unknown'}`;
            const blockers = document.createElement('div');
            const blockerItems = Array.isArray(finding.readiness_blockers) ? finding.readiness_blockers : [];
            blockers.textContent = blockerItems.length
                ? `Blockers: ${blockerItems.join('; ')}`
                : 'Blockers: none recorded';
            const evidence = document.createElement('pre');
            evidence.className = 'mono';
            evidence.textContent = JSON.stringify(finding.raw_evidence || {}, null, 2);
            host.append(title, meta, blockers, evidence);
        }

        async function createResearchFinding(index) {
            const candidate = state.researchCandidates[index];
            if (!candidate) {
                return;
            }
            try {
                const finding = await api('/api/research/findings', {
                    method: 'POST',
                    auth: true,
                    body: JSON.stringify({
                        source_type: candidate.source_type,
                        source_detection_id: candidate.source_detection_id,
                        source_event_id: candidate.source_event_id,
                        title: candidate.title,
                        repository: candidate.repository,
                        severity: candidate.severity,
                        playbook: candidate.playbook,
                        raw_evidence: candidate.raw_evidence,
                        scope_asset: candidate.repository,
                        scope_status: 'unknown'
                    })
                });
                state.selectedResearchFinding = finding;
                toast('Research finding created.', 'success');
                await scoreSelectedResearch();
                await loadResearch();
            } catch (error) {
                toast(error.message || 'Failed to create research finding.', 'error');
            }
        }

        function selectResearchFinding(id) {
            state.selectedResearchFinding = state.researchFindings.find((finding) => finding.id === id) || null;
            renderResearchDetail(state.selectedResearchFinding);
        }

        async function scoreSelectedResearch() {
            const finding = state.selectedResearchFinding;
            if (!finding) {
                toast('Select a research finding first.', 'error');
                return;
            }
            try {
                const data = await api(`/api/research/findings/${finding.id}/score`, {
                    method: 'POST',
                    auth: true
                });
                state.selectedResearchFinding = data.finding;
                renderResearchDetail(state.selectedResearchFinding);
                toast(`Readiness score: ${data.score?.score ?? 0}/100`, 'success');
            } catch (error) {
                toast(error.message || 'Failed to score research finding.', 'error');
            }
        }

        async function exportSelectedResearch() {
            const finding = state.selectedResearchFinding;
            if (!finding) {
                toast('Select a research finding first.', 'error');
                return;
            }
            try {
                const data = await api(`/api/research/findings/${finding.id}/export`, {
                    method: 'POST',
                    auth: true,
                    body: JSON.stringify({ format: 'markdown', redacted: true })
                });
                $('researchOutput').textContent = data.content || JSON.stringify(data.content, null, 2);
                toast('Redacted Markdown export generated.', 'success');
            } catch (error) {
                toast(error.message || 'Failed to export research finding.', 'error');
            }
        }

        async function runResearchAi() {
            const finding = state.selectedResearchFinding;
            if (!finding) {
                toast('Select a research finding first.', 'error');
                return;
            }
            const payload = {
                provider: $('researchProvider').value,
                model: $('researchModel').value.trim() || undefined,
                prompt: $('researchAiPrompt').value,
                include_full_evidence: $('researchFullEvidence').value === 'true',
                confirmed_full_evidence: false
            };
            setText('researchAiMode', payload.provider === 'local-openai' ? 'Local' : 'External');
            await submitResearchAiPayload(finding.id, payload);
        }

        async function submitResearchAiPayload(findingId, payload) {
            try {
                const data = await api(`/api/research/findings/${findingId}/ai-assist`, {
                    method: 'POST',
                    auth: true,
                    body: JSON.stringify(payload)
                });
                $('researchOutput').textContent = data.ai_output?.result || 'AI assist completed.';
                state.selectedResearchFinding = data.finding;
                await loadResearch();
                toast('AI assist completed.', 'success');
            } catch (error) {
                if (error.status === 409 && error.payload?.confirmation_required) {
                    $('researchOutput').textContent = JSON.stringify(error.payload.evidence_preview || {}, null, 2);
                    const confirmed = window.confirm('Send full evidence to the selected external AI provider?');
                    if (confirmed) {
                        await submitResearchAiPayload(findingId, {
                            ...payload,
                            confirmed_full_evidence: true
                        });
                    }
                    return;
                }
                toast(error.message || 'Research AI assist failed.', 'error');
            }
        }

        async function refreshAll() {
            $('refreshButton').disabled = true;
            const tasks = [
                loadSystemStatus(),
                loadScannerMetrics(),
                loadOverview(),
                loadDetections(),
                loadResearch(),
                loadLogs()
            ];
            const results = await Promise.allSettled(tasks);
            const failed = results.filter((result) => result.status === 'rejected');
            if (failed.length) {
                const authFailure = failed.some((result) => result.reason?.status === 401);
                if (authFailure) {
                    toast('Some data requires sign in.', 'error');
                } else {
                    toast(failed[0].reason?.message || 'Dashboard refresh failed.', 'error');
                }
            }
            $('refreshButton').disabled = false;
        }

        async function controlScanner(action) {
            if (!state.token) {
                openLogin();
                return;
            }
            const buttons = ['scannerPrimaryButton', 'scannerStopButton', 'scannerRestartButton'].map($);
            buttons.forEach((button) => {
                if (button) {
                    button.disabled = true;
                }
            });
            try {
                const data = await api('/api/v1/scraper/control', {
                    method: 'POST',
                    auth: true,
                    body: JSON.stringify({ action })
                });
                toast(data.message || `Scanner ${action} accepted.`, 'success');
                await refreshAll();
            } catch (error) {
                toast(error.message || 'Scanner control failed.', 'error');
            } finally {
                buttons.forEach((button) => {
                    if (button) {
                        button.disabled = false;
                    }
                });
            }
        }

        async function submitScan(event) {
            event.preventDefault();
            if (!state.token) {
                openLogin();
                return;
            }
            const repository = $('repositoryInput').value.trim();
            if (!repository) {
                toast('Repository is required.', 'error');
                return;
            }
            $('scanSubmitButton').disabled = true;
            setText('scanResultChip', 'Submitting');
            try {
                const branch = $('branchInput').value.trim();
                const payload = {
                    repository,
                    scan_type: $('scanTypeInput').value,
                    include_private: false,
                    ...(branch ? { branch } : {})
                };
                const data = await api('/api/v1/scanner/scan', {
                    method: 'POST',
                    auth: true,
                    body: JSON.stringify(payload)
                });
                setText('scanResultChip', data.status || 'Submitted');
                toast(`Scan queued for ${repository}.`, 'success');
                await refreshAll();
            } catch (error) {
                setText('scanResultChip', 'Failed');
                toast(error.message || 'Failed to launch scan.', 'error');
            } finally {
                $('scanSubmitButton').disabled = false;
            }
        }

        function openLogin() {
            $('loginModal').classList.add('open');
            setTimeout(() => $('usernameInput').focus(), 0);
        }

        function closeLogin() {
            $('loginModal').classList.remove('open');
            $('passwordInput').value = '';
        }

        async function submitLogin(event) {
            event.preventDefault();
            $('loginSubmitButton').disabled = true;
            try {
                const data = await api('/api/auth/login', {
                    method: 'POST',
                    body: JSON.stringify({
                        username: $('usernameInput').value,
                        password: $('passwordInput').value
                    })
                });
                state.token = data.token;
                state.user = data.user;
                sessionStorage.setItem('authToken', state.token);
                localStorage.removeItem('authToken');
                updateAuthUi();
                closeLogin();
                toast('Signed in.', 'success');
                await refreshAll();
            } catch (error) {
                toast(error.message || 'Sign in failed.', 'error');
            } finally {
                $('loginSubmitButton').disabled = false;
            }
        }

        function logout() {
            state.token = '';
            state.user = null;
            localStorage.removeItem('authToken');
            sessionStorage.removeItem('authToken');
            updateAuthUi();
            refreshAll();
        }

        function escapeHtml(value) {
            return String(value ?? '')
                .replaceAll('&', '&amp;')
                .replaceAll('<', '&lt;')
                .replaceAll('>', '&gt;')
                .replaceAll('"', '&quot;')
                .replaceAll("'", '&#39;');
        }

        function bindEvents() {
            document.querySelectorAll('.rail button').forEach((button) => {
                button.addEventListener('click', () => showSection(button.dataset.section));
            });
            $('refreshButton').addEventListener('click', refreshAll);
            $('loginButton').addEventListener('click', openLogin);
            $('logoutButton').addEventListener('click', logout);
            $('closeLoginButton').addEventListener('click', closeLogin);
            $('cancelLoginButton').addEventListener('click', closeLogin);
            $('loginForm').addEventListener('submit', submitLogin);
            $('scanForm').addEventListener('submit', submitScan);
            $('applyDetectionFilters').addEventListener('click', loadDetections);
            $('applyLogFilters').addEventListener('click', loadLogs);
            $('refreshResearchButton').addEventListener('click', loadResearch);
            $('scoreResearchButton').addEventListener('click', scoreSelectedResearch);
            $('exportResearchButton').addEventListener('click', exportSelectedResearch);
            $('researchRunAiButton').addEventListener('click', runResearchAi);
            $('scannerPrimaryButton').addEventListener('click', () => controlScanner($('scannerPrimaryButton').dataset.action || 'start'));
            $('scannerStopButton').addEventListener('click', () => controlScanner('stop'));
            $('scannerRestartButton').addEventListener('click', () => controlScanner('restart'));
            $('loginModal').addEventListener('click', (event) => {
                if (event.target === $('loginModal')) {
                    closeLogin();
                }
            });
        }

        async function boot() {
            bindEvents();
            updateAuthUi();
            await verifyAuth();
            await refreshAll();
            state.refreshTimer = setInterval(refreshAll, 30000);
        }

        boot().catch((error) => {
            setChip('apiChip', 'API unavailable', 'bad');
            toast(error.message || 'Dashboard failed to start.', 'error');
        });
