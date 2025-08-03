import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';

interface PerformanceMetrics {
  scan_performance: {
    total_scans: number;
    avg_scan_time: number;
    successful_scans: number;
    failed_scans: number;
    scans_per_hour: number;
  };
  resource_usage: {
    cpu_usage_history: number[];
    memory_usage_history: number[];
    disk_io_history: number[];
    network_io_history: number[];
    timestamps: string[];
  };
  detection_metrics: {
    secrets_detected: number;
    false_positives: number;
    accuracy_rate: number;
    detection_types: Record<string, number>;
  };
  optimization_suggestions: Array<{
    category: string;
    suggestion: string;
    impact: 'high' | 'medium' | 'low';
    estimated_improvement: string;
  }>;
}

export function PerformanceReport() {
  const [metrics, setMetrics] = useState<PerformanceMetrics | null>(null);
  const [loading, setLoading] = useState(true);
  const [timeRange, setTimeRange] = useState<'1h' | '6h' | '24h' | '7d'>('24h');
  const [autoRefresh, setAutoRefresh] = useState(true);

  useEffect(() => {
    loadPerformanceData();
    
    let interval: NodeJS.Timeout;
    if (autoRefresh) {
      interval = setInterval(loadPerformanceData, 30000); // Refresh every 30 seconds
    }
    
    return () => {
      if (interval) clearInterval(interval);
    };
  }, [timeRange, autoRefresh]);

  const loadPerformanceData = async () => {
    try {
      const performanceData = await invoke<PerformanceMetrics>('get_performance_report', {
        timeRange
      });
      setMetrics(performanceData);
    } catch (error) {
      console.error('Failed to load performance data:', error);
    } finally {
      setLoading(false);
    }
  };

  const optimizeSystem = async () => {
    try {
      await invoke('optimize_system');
      await loadPerformanceData(); // Reload data after optimization
    } catch (error) {
      console.error('Failed to optimize system:', error);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center">
          <div className="animate-spin w-12 h-12 border-4 border-lava-green border-t-transparent rounded-full mx-auto mb-4"></div>
          <p className="text-gray-300">Loading performance data...</p>
        </div>
      </div>
    );
  }

  if (!metrics) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center">
          <p className="text-red-400 text-xl mb-4">Failed to load performance data</p>
          <button
            className="glow-button"
            onClick={loadPerformanceData}
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex justify-between items-center">
        <div>
          <h1 className="text-3xl font-bold text-white mb-2">Performance Report</h1>
          <p className="text-gray-400">
            System performance metrics and optimization insights
          </p>
        </div>
        <div className="flex gap-3">
          <select
            value={timeRange}
            onChange={(e) => setTimeRange(e.target.value as any)}
            className="px-4 py-2 bg-gray-800 border border-gray-600 rounded-lg text-white focus:border-lava-green focus:outline-none"
          >
            <option value="1h">Last Hour</option>
            <option value="6h">Last 6 Hours</option>
            <option value="24h">Last 24 Hours</option>
            <option value="7d">Last 7 Days</option>
          </select>
          <button
            className={`px-4 py-2 rounded-lg font-semibold transition-all duration-300 ${
              autoRefresh ? 'glow-button' : 'bg-gray-600 text-gray-300'
            }`}
            onClick={() => setAutoRefresh(!autoRefresh)}
          >
            {autoRefresh ? '⏸️ Pause' : '▶️ Resume'} Auto-refresh
          </button>
          <button
            className="glow-button"
            onClick={optimizeSystem}
          >
            ⚡ Optimize System
          </button>
        </div>
      </div>

      {/* Performance Overview */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        <div className="metric-card">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-gray-400 text-sm">Total Scans</p>
              <p className="text-3xl font-bold text-white">{metrics.scan_performance.total_scans}</p>
              <p className="text-green-400 text-sm">
                {metrics.scan_performance.scans_per_hour} scans/hour
              </p>
            </div>
            <div className="text-4xl">🔍</div>
          </div>
        </div>

        <div className="metric-card">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-gray-400 text-sm">Avg Scan Time</p>
              <p className="text-3xl font-bold text-white">
                {(metrics.scan_performance.avg_scan_time / 1000).toFixed(1)}s
              </p>
              <p className="text-gray-400 text-sm">per repository</p>
            </div>
            <div className="text-4xl">⏱️</div>
          </div>
        </div>

        <div className="metric-card">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-gray-400 text-sm">Success Rate</p>
              <p className="text-3xl font-bold text-lava-green">
                {((metrics.scan_performance.successful_scans / metrics.scan_performance.total_scans) * 100).toFixed(1)}%
              </p>
              <p className="text-gray-400 text-sm">
                {metrics.scan_performance.failed_scans} failures
              </p>
            </div>
            <div className="text-4xl">✅</div>
          </div>
        </div>

        <div className="metric-card">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-gray-400 text-sm">Detection Accuracy</p>
              <p className="text-3xl font-bold text-lava-green">
                {(metrics.detection_metrics.accuracy_rate * 100).toFixed(1)}%
              </p>
              <p className="text-gray-400 text-sm">
                {metrics.detection_metrics.false_positives} false positives
              </p>
            </div>
            <div className="text-4xl">🎯</div>
          </div>
        </div>
      </div>

      {/* Resource Usage Charts */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="metric-card">
          <h3 className="text-xl font-semibold text-white mb-4">CPU & Memory Usage</h3>
          <div className="space-y-4">
            {/* CPU Usage Chart */}
            <div>
              <div className="flex justify-between items-center mb-2">
                <span className="text-gray-400 text-sm">CPU Usage</span>
                <span className="text-white text-sm">
                  {metrics.resource_usage.cpu_usage_history[metrics.resource_usage.cpu_usage_history.length - 1]}%
                </span>
              </div>
              <div className="h-20 bg-gray-800 rounded-lg p-2 flex items-end gap-1">
                {metrics.resource_usage.cpu_usage_history.slice(-20).map((usage, index) => (
                  <div
                    key={index}
                    className={`flex-1 rounded-sm ${
                      usage > 90 ? 'bg-red-500' :
                      usage > 70 ? 'bg-yellow-500' : 'bg-green-500'
                    }`}
                    style={{ height: `${usage}%` }}
                  />
                ))}
              </div>
            </div>

            {/* Memory Usage Chart */}
            <div>
              <div className="flex justify-between items-center mb-2">
                <span className="text-gray-400 text-sm">Memory Usage</span>
                <span className="text-white text-sm">
                  {metrics.resource_usage.memory_usage_history[metrics.resource_usage.memory_usage_history.length - 1]}%
                </span>
              </div>
              <div className="h-20 bg-gray-800 rounded-lg p-2 flex items-end gap-1">
                {metrics.resource_usage.memory_usage_history.slice(-20).map((usage, index) => (
                  <div
                    key={index}
                    className={`flex-1 rounded-sm ${
                      usage > 90 ? 'bg-red-500' :
                      usage > 70 ? 'bg-yellow-500' : 'bg-blue-500'
                    }`}
                    style={{ height: `${usage}%` }}
                  />
                ))}
              </div>
            </div>
          </div>
        </div>

        <div className="metric-card">
          <h3 className="text-xl font-semibold text-white mb-4">Detection Breakdown</h3>
          <div className="space-y-3">
            {Object.entries(metrics.detection_metrics.detection_types).map(([type, count]) => (
              <div key={type} className="flex justify-between items-center">
                <span className="text-gray-300 capitalize">{type.replace('_', ' ')}</span>
                <div className="flex items-center gap-2">
                  <div className="w-32 bg-gray-700 rounded-full h-2">
                    <div
                      className="bg-lava-green h-2 rounded-full"
                      style={{
                        width: `${(count / metrics.detection_metrics.secrets_detected) * 100}%`
                      }}
                    />
                  </div>
                  <span className="text-white font-semibold w-8 text-right">{count}</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Network & Disk I/O */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="metric-card">
          <h3 className="text-xl font-semibold text-white mb-4">Network I/O</h3>
          <div className="h-32 bg-gray-800 rounded-lg p-2 flex items-end gap-1">
            {metrics.resource_usage.network_io_history.slice(-30).map((io, index) => (
              <div
                key={index}
                className="flex-1 bg-purple-500 rounded-sm"
                style={{ height: `${Math.min((io / 1000) * 100, 100)}%` }}
              />
            ))}
          </div>
          <p className="text-gray-400 text-sm mt-2">
            Current: {(metrics.resource_usage.network_io_history[metrics.resource_usage.network_io_history.length - 1] / 1000).toFixed(1)} KB/s
          </p>
        </div>

        <div className="metric-card">
          <h3 className="text-xl font-semibold text-white mb-4">Disk I/O</h3>
          <div className="h-32 bg-gray-800 rounded-lg p-2 flex items-end gap-1">
            {metrics.resource_usage.disk_io_history.slice(-30).map((io, index) => (
              <div
                key={index}
                className="flex-1 bg-indigo-500 rounded-sm"
                style={{ height: `${Math.min((io / 1000) * 100, 100)}%` }}
              />
            ))}
          </div>
          <p className="text-gray-400 text-sm mt-2">
            Current: {(metrics.resource_usage.disk_io_history[metrics.resource_usage.disk_io_history.length - 1] / 1000).toFixed(1)} KB/s
          </p>
        </div>
      </div>

      {/* Optimization Suggestions */}
      <div className="metric-card">
        <h3 className="text-xl font-semibold text-white mb-4">Optimization Suggestions</h3>
        {metrics.optimization_suggestions.length === 0 ? (
          <div className="text-center py-8">
            <div className="text-6xl mb-4">🎉</div>
            <p className="text-green-400 text-lg">System is running optimally!</p>
            <p className="text-gray-400 text-sm">No optimization suggestions at this time</p>
          </div>
        ) : (
          <div className="space-y-3">
            {metrics.optimization_suggestions.map((suggestion, index) => (
              <div key={index} className="flex items-start gap-4 p-4 glass-panel">
                <div className={`w-3 h-3 rounded-full mt-2 ${
                  suggestion.impact === 'high' ? 'bg-red-500' :
                  suggestion.impact === 'medium' ? 'bg-yellow-500' : 'bg-green-500'
                }`} />
                <div className="flex-1">
                  <div className="flex justify-between items-start mb-2">
                    <h4 className="text-white font-semibold">{suggestion.category}</h4>
                    <span className={`px-2 py-1 rounded text-xs font-semibold ${
                      suggestion.impact === 'high' ? 'bg-red-500 bg-opacity-20 text-red-400' :
                      suggestion.impact === 'medium' ? 'bg-yellow-500 bg-opacity-20 text-yellow-400' :
                      'bg-green-500 bg-opacity-20 text-green-400'
                    }`}>
                      {suggestion.impact.toUpperCase()} IMPACT
                    </span>
                  </div>
                  <p className="text-gray-300 text-sm mb-2">{suggestion.suggestion}</p>
                  <p className="text-lava-green text-sm">
                    Expected improvement: {suggestion.estimated_improvement}
                  </p>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
