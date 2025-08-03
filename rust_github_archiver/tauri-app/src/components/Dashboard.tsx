import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { Chart, ChartConfiguration } from 'chart.js/auto';

interface DashboardProps {
  systemHealth: 'healthy' | 'warning' | 'critical';
  onHealthChange: (health: 'healthy' | 'warning' | 'critical') => void;
}

interface DashboardData {
  total_secrets: number;
  secrets_by_severity: {
    critical: number;
    high: number;
    medium: number;
    low: number;
  };
  recent_scans: Array<{
    repository: string;
    timestamp: string;
    secrets_found: number;
    status: string;
  }>;
  system_metrics: {
    cpu_usage: number;
    memory_usage: number;
    disk_usage: number;
    network_requests: number;
  };
  threat_timeline: Array<{
    timestamp: string;
    threat_level: number;
    description: string;
  }>;
}

export function Dashboard({ systemHealth, onHealthChange }: DashboardProps) {
  const [data, setData] = useState<DashboardData | null>(null);
  const [loading, setLoading] = useState(true);
  const [lastUpdated, setLastUpdated] = useState<Date>(new Date());

  useEffect(() => {
    loadDashboardData();
    const interval = setInterval(loadDashboardData, 30000); // Update every 30 seconds
    return () => clearInterval(interval);
  }, []);

  const loadDashboardData = async () => {
    try {
      const dashboardData = await invoke<DashboardData>('get_dashboard_data');
      setData(dashboardData);
      setLastUpdated(new Date());
      
      // Update system health based on metrics
      if (dashboardData.system_metrics.cpu_usage > 90 || 
          dashboardData.system_metrics.memory_usage > 90 ||
          dashboardData.secrets_by_severity.critical > 10) {
        onHealthChange('critical');
      } else if (dashboardData.system_metrics.cpu_usage > 70 || 
                 dashboardData.system_metrics.memory_usage > 70 ||
                 dashboardData.secrets_by_severity.critical > 5) {
        onHealthChange('warning');
      } else {
        onHealthChange('healthy');
      }
    } catch (error) {
      console.error('Failed to load dashboard data:', error);
      onHealthChange('critical');
    } finally {
      setLoading(false);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center">
          <div className="animate-spin w-12 h-12 border-4 border-lava-green border-t-transparent rounded-full mx-auto mb-4"></div>
          <p className="text-gray-300">Loading dashboard...</p>
        </div>
      </div>
    );
  }

  if (!data) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center">
          <p className="text-red-400 text-xl mb-4">Failed to load dashboard data</p>
          <button
            className="glow-button"
            onClick={loadDashboardData}
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
          <h1 className="text-3xl font-bold text-white mb-2">Security Dashboard</h1>
          <p className="text-gray-400">
            Last updated: {lastUpdated.toLocaleString()}
          </p>
        </div>
        <button
          className="glow-button"
          onClick={loadDashboardData}
        >
          🔄 Refresh
        </button>
      </div>

      {/* Key Metrics */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        <div className="metric-card">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-gray-400 text-sm">Total Secrets</p>
              <p className="text-3xl font-bold text-white">{data.total_secrets}</p>
            </div>
            <div className="text-4xl">🔐</div>
          </div>
        </div>

        <div className="metric-card">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-gray-400 text-sm">Critical Threats</p>
              <p className={`text-3xl font-bold ${
                data.secrets_by_severity.critical > 10 ? 'text-lava-red' :
                data.secrets_by_severity.critical > 5 ? 'text-lava-yellow' : 'text-lava-green'
              }`}>
                {data.secrets_by_severity.critical}
              </p>
            </div>
            <div className="text-4xl">⚠️</div>
          </div>
        </div>

        <div className="metric-card">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-gray-400 text-sm">CPU Usage</p>
              <p className={`text-3xl font-bold ${
                data.system_metrics.cpu_usage > 90 ? 'text-lava-red' :
                data.system_metrics.cpu_usage > 70 ? 'text-lava-yellow' : 'text-lava-green'
              }`}>
                {data.system_metrics.cpu_usage}%
              </p>
            </div>
            <div className="text-4xl">🖥️</div>
          </div>
        </div>

        <div className="metric-card">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-gray-400 text-sm">Memory Usage</p>
              <p className={`text-3xl font-bold ${
                data.system_metrics.memory_usage > 90 ? 'text-lava-red' :
                data.system_metrics.memory_usage > 70 ? 'text-lava-yellow' : 'text-lava-green'
              }`}>
                {data.system_metrics.memory_usage}%
              </p>
            </div>
            <div className="text-4xl">💾</div>
          </div>
        </div>
      </div>

      {/* Secrets by Severity */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="metric-card">
          <h3 className="text-xl font-semibold text-white mb-4">Secrets by Severity</h3>
          <div className="space-y-3">
            <div className="flex justify-between items-center">
              <span className="text-red-400">Critical</span>
              <div className="flex items-center gap-2">
                <div className="w-32 bg-gray-700 rounded-full h-2">
                  <div
                    className="bg-red-500 h-2 rounded-full"
                    style={{
                      width: `${(data.secrets_by_severity.critical / data.total_secrets) * 100}%`
                    }}
                  ></div>
                </div>
                <span className="text-white font-semibold">{data.secrets_by_severity.critical}</span>
              </div>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-orange-400">High</span>
              <div className="flex items-center gap-2">
                <div className="w-32 bg-gray-700 rounded-full h-2">
                  <div
                    className="bg-orange-500 h-2 rounded-full"
                    style={{
                      width: `${(data.secrets_by_severity.high / data.total_secrets) * 100}%`
                    }}
                  ></div>
                </div>
                <span className="text-white font-semibold">{data.secrets_by_severity.high}</span>
              </div>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-yellow-400">Medium</span>
              <div className="flex items-center gap-2">
                <div className="w-32 bg-gray-700 rounded-full h-2">
                  <div
                    className="bg-yellow-500 h-2 rounded-full"
                    style={{
                      width: `${(data.secrets_by_severity.medium / data.total_secrets) * 100}%`
                    }}
                  ></div>
                </div>
                <span className="text-white font-semibold">{data.secrets_by_severity.medium}</span>
              </div>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-green-400">Low</span>
              <div className="flex items-center gap-2">
                <div className="w-32 bg-gray-700 rounded-full h-2">
                  <div
                    className="bg-green-500 h-2 rounded-full"
                    style={{
                      width: `${(data.secrets_by_severity.low / data.total_secrets) * 100}%`
                    }}
                  ></div>
                </div>
                <span className="text-white font-semibold">{data.secrets_by_severity.low}</span>
              </div>
            </div>
          </div>
        </div>

        {/* Recent Scans */}
        <div className="metric-card">
          <h3 className="text-xl font-semibold text-white mb-4">Recent Scans</h3>
          <div className="space-y-3 max-h-64 overflow-y-auto">
            {data.recent_scans.map((scan, index) => (
              <div key={index} className="flex justify-between items-center p-3 glass-panel">
                <div>
                  <p className="text-white font-medium">{scan.repository}</p>
                  <p className="text-gray-400 text-sm">
                    {new Date(scan.timestamp).toLocaleString()}
                  </p>
                </div>
                <div className="text-right">
                  <p className={`font-semibold ${
                    scan.secrets_found > 0 ? 'text-lava-red' : 'text-lava-green'
                  }`}>
                    {scan.secrets_found} secrets
                  </p>
                  <p className={`text-sm ${
                    scan.status === 'completed' ? 'text-green-400' :
                    scan.status === 'failed' ? 'text-red-400' : 'text-yellow-400'
                  }`}>
                    {scan.status}
                  </p>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* System Health Indicators */}
      <div className="metric-card">
        <h3 className="text-xl font-semibold text-white mb-4">System Health</h3>
        <div className="grid grid-cols-1 md:grid-cols-4 gap-6">
          <div className="text-center">
            <div className={`w-16 h-16 rounded-full mx-auto mb-2 flex items-center justify-center text-2xl ${
              data.system_metrics.cpu_usage > 90 ? 'bg-red-500' :
              data.system_metrics.cpu_usage > 70 ? 'bg-yellow-500' : 'bg-green-500'
            }`}>
              🖥️
            </div>
            <p className="text-gray-400 text-sm">CPU</p>
            <p className="text-white font-semibold">{data.system_metrics.cpu_usage}%</p>
          </div>
          <div className="text-center">
            <div className={`w-16 h-16 rounded-full mx-auto mb-2 flex items-center justify-center text-2xl ${
              data.system_metrics.memory_usage > 90 ? 'bg-red-500' :
              data.system_metrics.memory_usage > 70 ? 'bg-yellow-500' : 'bg-green-500'
            }`}>
              💾
            </div>
            <p className="text-gray-400 text-sm">Memory</p>
            <p className="text-white font-semibold">{data.system_metrics.memory_usage}%</p>
          </div>
          <div className="text-center">
            <div className={`w-16 h-16 rounded-full mx-auto mb-2 flex items-center justify-center text-2xl ${
              data.system_metrics.disk_usage > 90 ? 'bg-red-500' :
              data.system_metrics.disk_usage > 70 ? 'bg-yellow-500' : 'bg-green-500'
            }`}>
              💿
            </div>
            <p className="text-gray-400 text-sm">Disk</p>
            <p className="text-white font-semibold">{data.system_metrics.disk_usage}%</p>
          </div>
          <div className="text-center">
            <div className="w-16 h-16 rounded-full mx-auto mb-2 flex items-center justify-center text-2xl bg-blue-500">
              🌐
            </div>
            <p className="text-gray-400 text-sm">Network</p>
            <p className="text-white font-semibold">{data.system_metrics.network_requests}</p>
          </div>
        </div>
      </div>

      {/* Quick Actions */}
      <div className="flex gap-4">
        <button
          className="glow-button"
          onClick={() => invoke('start_hunting')}
        >
          🚀 Start Hunt
        </button>
        <button
          className="glow-button"
          onClick={() => invoke('export_secrets', { format: 'pdf' })}
        >
          📊 Export Report
        </button>
        <button
          className="danger-button"
          onClick={() => confirm('Are you sure you want to stop all scans?') && invoke('stop_hunting')}
        >
          🛑 Emergency Stop
        </button>
      </div>
    </div>
  );
}
