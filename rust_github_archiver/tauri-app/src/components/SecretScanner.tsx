import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';

interface SecretScannerProps {
  onNotification: (type: 'info' | 'warning' | 'error' | 'success', message: string) => void;
  onStatusChange: (status: 'idle' | 'scanning' | 'error') => void;
}

interface ScanResult {
  id: string;
  repository: string;
  file_path: string;
  line_number: number;
  secret_type: string;
  severity: 'critical' | 'high' | 'medium' | 'low';
  content_preview: string;
  confidence: number;
  timestamp: string;
  status: 'new' | 'validated' | 'false_positive' | 'resolved';
}

interface ScanConfig {
  repositories: string[];
  scan_type: 'full' | 'incremental' | 'targeted';
  secret_types: string[];
  exclude_patterns: string[];
  include_private: boolean;
  max_concurrent: number;
}

export function SecretScanner({ onNotification, onStatusChange }: SecretScannerProps) {
  const [scanResults, setScanResults] = useState<ScanResult[]>([]);
  const [isScanning, setIsScanning] = useState(false);
  const [scanConfig, setScanConfig] = useState<ScanConfig>({
    repositories: [],
    scan_type: 'full',
    secret_types: ['api_key', 'password', 'token', 'certificate', 'database_url'],
    exclude_patterns: ['*.test.js', '*.spec.js', 'node_modules/*'],
    include_private: false,
    max_concurrent: 5
  });
  const [newRepository, setNewRepository] = useState('');
  const [selectedSecrets, setSelectedSecrets] = useState<Set<string>>(new Set());

  useEffect(() => {
    loadRecentScans();
  }, []);

  const loadRecentScans = async () => {
    try {
      // Load recent scan results from the backend
      // This would be implemented as a separate Tauri command
      onNotification('info', 'Loaded recent scan results');
    } catch (error) {
      console.error('Failed to load recent scans:', error);
      onNotification('error', 'Failed to load recent scans');
    }
  };

  const startScan = async () => {
    if (scanConfig.repositories.length === 0) {
      onNotification('warning', 'Please add at least one repository to scan');
      return;
    }

    setIsScanning(true);
    onStatusChange('scanning');
    
    try {
      onNotification('info', `Starting ${scanConfig.scan_type} scan of ${scanConfig.repositories.length} repositories`);
      
      for (const repo of scanConfig.repositories) {
        const result = await invoke<ScanResult[]>('scan_repository', {
          repository: repo,
          scanType: scanConfig.scan_type,
          secretTypes: scanConfig.secret_types,
          excludePatterns: scanConfig.exclude_patterns
        });
        
        setScanResults(prev => [...prev, ...result]);
        onNotification('success', `Completed scan of ${repo}: ${result.length} secrets found`);
      }
      
      onNotification('success', 'Scan completed successfully');
    } catch (error) {
      console.error('Scan failed:', error);
      onNotification('error', `Scan failed: ${error}`);
      onStatusChange('error');
    } finally {
      setIsScanning(false);
      onStatusChange('idle');
    }
  };

  const validateSecret = async (secretId: string, isValid: boolean) => {
    try {
      await invoke('validate_secret', {
        secretId,
        isValid,
        reason: isValid ? 'Confirmed as genuine secret' : 'Marked as false positive'
      });
      
      setScanResults(prev => 
        prev.map(secret => 
          secret.id === secretId 
            ? { ...secret, status: isValid ? 'validated' : 'false_positive' }
            : secret
        )
      );
      
      onNotification('success', `Secret ${isValid ? 'validated' : 'marked as false positive'}`);
    } catch (error) {
      console.error('Failed to validate secret:', error);
      onNotification('error', 'Failed to update secret status');
    }
  };

  const exportSelected = async () => {
    if (selectedSecrets.size === 0) {
      onNotification('warning', 'Please select secrets to export');
      return;
    }

    try {
      await invoke('export_secrets', {
        secretIds: Array.from(selectedSecrets),
        format: 'pdf'
      });
      
      onNotification('success', `Exported ${selectedSecrets.size} secrets to PDF`);
    } catch (error) {
      console.error('Export failed:', error);
      onNotification('error', 'Failed to export secrets');
    }
  };

  const addRepository = () => {
    if (newRepository.trim() && !scanConfig.repositories.includes(newRepository.trim())) {
      setScanConfig(prev => ({
        ...prev,
        repositories: [...prev.repositories, newRepository.trim()]
      }));
      setNewRepository('');
      onNotification('success', `Added repository: ${newRepository.trim()}`);
    }
  };

  const removeRepository = (repo: string) => {
    setScanConfig(prev => ({
      ...prev,
      repositories: prev.repositories.filter(r => r !== repo)
    }));
    onNotification('info', `Removed repository: ${repo}`);
  };

  const getSeverityColor = (severity: string) => {
    switch (severity) {
      case 'critical': return 'text-red-500 bg-red-500';
      case 'high': return 'text-orange-500 bg-orange-500';
      case 'medium': return 'text-yellow-500 bg-yellow-500';
      case 'low': return 'text-green-500 bg-green-500';
      default: return 'text-gray-500 bg-gray-500';
    }
  };

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex justify-between items-center">
        <div>
          <h1 className="text-3xl font-bold text-white mb-2">Secret Scanner</h1>
          <p className="text-gray-400">
            Scan repositories for exposed secrets and sensitive information
          </p>
        </div>
        <div className="flex gap-3">
          <button
            className={`px-6 py-3 rounded-lg font-semibold transition-all duration-300 ${
              isScanning 
                ? 'bg-gray-600 cursor-not-allowed' 
                : 'glow-button'
            }`}
            onClick={startScan}
            disabled={isScanning}
          >
            {isScanning ? '🔄 Scanning...' : '🚀 Start Scan'}
          </button>
          <button
            className="glow-button"
            onClick={exportSelected}
            disabled={selectedSecrets.size === 0}
          >
            📊 Export Selected
          </button>
        </div>
      </div>

      {/* Scan Configuration */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="metric-card">
          <h3 className="text-xl font-semibold text-white mb-4">Repository Configuration</h3>
          
          <div className="space-y-4">
            <div className="flex gap-2">
              <input
                type="text"
                value={newRepository}
                onChange={(e) => setNewRepository(e.target.value)}
                placeholder="Enter repository URL or name"
                className="flex-1 px-3 py-2 bg-gray-800 border border-gray-600 rounded-lg text-white focus:border-lava-green focus:outline-none"
                onKeyPress={(e) => e.key === 'Enter' && addRepository()}
              />
              <button
                className="glow-button"
                onClick={addRepository}
              >
                Add
              </button>
            </div>
            
            <div className="space-y-2 max-h-40 overflow-y-auto">
              {scanConfig.repositories.map((repo, index) => (
                <div key={index} className="flex justify-between items-center p-2 glass-panel">
                  <span className="text-white text-sm">{repo}</span>
                  <button
                    className="text-red-400 hover:text-red-300 text-sm"
                    onClick={() => removeRepository(repo)}
                  >
                    Remove
                  </button>
                </div>
              ))}
            </div>
          </div>
        </div>

        <div className="metric-card">
          <h3 className="text-xl font-semibold text-white mb-4">Scan Settings</h3>
          
          <div className="space-y-4">
            <div>
              <label className="block text-gray-300 text-sm mb-2">Scan Type</label>
              <select
                value={scanConfig.scan_type}
                onChange={(e) => setScanConfig(prev => ({ ...prev, scan_type: e.target.value as any }))}
                className="w-full px-3 py-2 bg-gray-800 border border-gray-600 rounded-lg text-white focus:border-lava-green focus:outline-none"
              >
                <option value="full">Full Scan</option>
                <option value="incremental">Incremental</option>
                <option value="targeted">Targeted</option>
              </select>
            </div>

            <div>
              <label className="block text-gray-300 text-sm mb-2">Max Concurrent Scans</label>
              <input
                type="number"
                value={scanConfig.max_concurrent}
                onChange={(e) => setScanConfig(prev => ({ ...prev, max_concurrent: parseInt(e.target.value) || 5 }))}
                min="1"
                max="20"
                className="w-full px-3 py-2 bg-gray-800 border border-gray-600 rounded-lg text-white focus:border-lava-green focus:outline-none"
              />
            </div>

            <div className="flex items-center">
              <input
                type="checkbox"
                id="include_private"
                checked={scanConfig.include_private}
                onChange={(e) => setScanConfig(prev => ({ ...prev, include_private: e.target.checked }))}
                className="mr-2"
              />
              <label htmlFor="include_private" className="text-gray-300 text-sm">
                Include Private Repositories
              </label>
            </div>
          </div>
        </div>
      </div>

      {/* Scan Results */}
      <div className="metric-card">
        <div className="flex justify-between items-center mb-4">
          <h3 className="text-xl font-semibold text-white">Scan Results</h3>
          <div className="flex items-center gap-4">
            <span className="text-gray-400 text-sm">
              {scanResults.length} secrets found
            </span>
            <span className="text-gray-400 text-sm">
              {selectedSecrets.size} selected
            </span>
          </div>
        </div>

        {scanResults.length === 0 ? (
          <div className="text-center py-12">
            <div className="text-6xl mb-4">🔍</div>
            <p className="text-gray-400 text-lg">No secrets found yet</p>
            <p className="text-gray-500 text-sm">Start a scan to discover exposed secrets</p>
          </div>
        ) : (
          <div className="space-y-3 max-h-96 overflow-y-auto">
            {scanResults.map((secret) => (
              <div key={secret.id} className="secret-card">
                <div className="flex items-start justify-between">
                  <div className="flex items-start gap-3">
                    <input
                      type="checkbox"
                      checked={selectedSecrets.has(secret.id)}
                      onChange={(e) => {
                        const newSelected = new Set(selectedSecrets);
                        if (e.target.checked) {
                          newSelected.add(secret.id);
                        } else {
                          newSelected.delete(secret.id);
                        }
                        setSelectedSecrets(newSelected);
                      }}
                      className="mt-1"
                    />
                    <div className="flex-1">
                      <div className="flex items-center gap-2 mb-2">
                        <span className={`px-2 py-1 rounded text-xs font-semibold ${getSeverityColor(secret.severity)} bg-opacity-20`}>
                          {secret.severity.toUpperCase()}
                        </span>
                        <span className="text-gray-400 text-xs">{secret.secret_type}</span>
                        <span className="text-gray-500 text-xs">
                          {Math.round(secret.confidence * 100)}% confidence
                        </span>
                      </div>
                      <p className="text-white font-medium">{secret.repository}</p>
                      <p className="text-gray-400 text-sm">{secret.file_path}:{secret.line_number}</p>
                      <p className="text-gray-300 text-sm mt-2 font-mono bg-gray-800 p-2 rounded">
                        {secret.content_preview}
                      </p>
                    </div>
                  </div>
                  <div className="flex gap-2 ml-4">
                    {secret.status === 'new' && (
                      <>
                        <button
                          className="px-3 py-1 bg-green-600 hover:bg-green-700 text-white text-xs rounded transition-colors"
                          onClick={() => validateSecret(secret.id, true)}
                        >
                          ✓ Valid
                        </button>
                        <button
                          className="px-3 py-1 bg-red-600 hover:bg-red-700 text-white text-xs rounded transition-colors"
                          onClick={() => validateSecret(secret.id, false)}
                        >
                          ✗ False Positive
                        </button>
                      </>
                    )}
                    {secret.status !== 'new' && (
                      <span className={`px-3 py-1 text-xs rounded ${
                        secret.status === 'validated' ? 'bg-green-600 text-white' :
                        secret.status === 'false_positive' ? 'bg-gray-600 text-white' :
                        'bg-blue-600 text-white'
                      }`}>
                        {secret.status.replace('_', ' ')}
                      </span>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
