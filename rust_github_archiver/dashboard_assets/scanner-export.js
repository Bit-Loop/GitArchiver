(function (global) {
    const namespace = global.GitHubArchiverDashboard = global.GitHubArchiverDashboard || {};

    namespace.createScannerExportController = function createScannerExportController(deps) {
        function exportSecrets(format = 'json') {
            if (!deps.getAuthToken()) {
                deps.showAlert('Please login to export secrets', 'warning');
                return;
            }

            const supportedFormats = ['json', 'csv'];
            const normalizedFormat = (format || 'json').toLowerCase();

            if (!supportedFormats.includes(normalizedFormat)) {
                deps.showAlert('Unsupported export format requested', 'warning');
                return;
            }

            deps.showAlert(`Exporting secrets as ${normalizedFormat.toUpperCase()}...`, 'info');

            deps.makeAuthenticatedRequest(`/api/v1/scanner/export?format=${normalizedFormat}`)
                .then(async (response) => {
                    if (!response.ok) {
                        throw new Error(`Export failed with status ${response.status}`);
                    }
                    return deps.parseJsonSafely(response);
                })
                .then((data) => {
                    if (!data || !data.detections) {
                        deps.showAlert('Export completed but no detections were returned', 'warning');
                        return;
                    }

                    let fileContents;
                    let mimeType = 'application/json';
                    let extension = normalizedFormat;

                    if (normalizedFormat === 'csv') {
                        fileContents = deps.buildSecretCsv(data.detections);
                        mimeType = 'text/csv';
                    } else {
                        fileContents = JSON.stringify(data, null, 2);
                    }

                    const blob = new Blob([fileContents], { type: mimeType });
                    const url = window.URL.createObjectURL(blob);
                    const a = document.createElement('a');
                    a.href = url;
                    const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, -5);
                    a.download = `secrets-export-${timestamp}.${extension}`;
                    document.body.appendChild(a);
                    a.click();
                    document.body.removeChild(a);
                    window.URL.revokeObjectURL(url);

                    deps.showAlert(`Successfully exported ${data.total_secrets} secret(s)!`, 'success');
                })
                .catch((error) => {
                    deps.showAlert(`Error exporting secrets: ${error.message}`, 'danger');
                    console.error('Export error:', error);
                });
        }

        return {
            exportSecrets
        };
    };
})(window);
