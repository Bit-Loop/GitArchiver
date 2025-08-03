import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { LavaLampBackground } from './components/LavaLampBackground';
import { Dashboard } from './components/Dashboard';
import { SecretScanner } from './components/SecretScanner';
import { PerformanceReport } from './components/PerformanceReport';
import { WebhookConfig } from './components/WebhookConfig';
import './style.css';

interface AppState {
  currentView: 'dashboard' | 'scanner' | 'performance' | 'webhooks';
  hunterStatus: 'idle' | 'scanning' | 'error';
  systemHealth: 'healthy' | 'warning' | 'critical';
  isInitialized: boolean;
}

function App() {
  const [state, setState] = useState<AppState>({
    currentView: 'dashboard',
    hunterStatus: 'idle',
    systemHealth: 'healthy',
    isInitialized: false
  });

  const [notifications, setNotifications] = useState<Array<{
    id: string;
    type: 'info' | 'warning' | 'error' | 'success';
    message: string;
    timestamp: Date;
  }>>([]);

  useEffect(() => {
    initializeHunter();
  }, []);

  const initializeHunter = async () => {
    try {
      await invoke('initialize_hunter');
      setState(prev => ({ ...prev, isInitialized: true }));
      addNotification('success', 'GitHub Secret Hunter initialized successfully');
    } catch (error) {
      console.error('Failed to initialize hunter:', error);
      setState(prev => ({ ...prev, systemHealth: 'critical' }));
      addNotification('error', 'Failed to initialize Secret Hunter');
    }
  };

  const addNotification = (type: 'info' | 'warning' | 'error' | 'success', message: string) => {
    const id = Date.now().toString();
    setNotifications(prev => [
      ...prev,
      { id, type, message, timestamp: new Date() }
    ]);

    // Auto-remove after 5 seconds
    setTimeout(() => {
      setNotifications(prev => prev.filter(n => n.id !== id));
    }, 5000);
  };

  const renderCurrentView = () => {
    switch (state.currentView) {
      case 'dashboard':
        return <Dashboard systemHealth={state.systemHealth} onHealthChange={(health) => setState(prev => ({ ...prev, systemHealth: health }))} />;
      case 'scanner':
        return <SecretScanner onNotification={addNotification} onStatusChange={(status) => setState(prev => ({ ...prev, hunterStatus: status }))} />;
      case 'performance':
        return <PerformanceReport />;
      case 'webhooks':
        return <WebhookConfig onNotification={addNotification} />;
      default:
        return <Dashboard systemHealth={state.systemHealth} onHealthChange={(health) => setState(prev => ({ ...prev, systemHealth: health }))} />;
    }
  };

  return (
    <div className="relative w-full h-screen overflow-hidden">
      {/* Lava Lamp Background */}
      <LavaLampBackground 
        health={state.systemHealth}
        isScanning={state.hunterStatus === 'scanning'}
      />

      {/* Main Application */}
      <div className="relative z-10 flex h-full">
        {/* Sidebar Navigation */}
        <div className="w-64 glass-panel m-4 p-6 flex flex-col">
          <div className="mb-8">
            <h1 className="text-2xl font-bold text-lava-green mb-2">Secret Hunter</h1>
            <div className="flex items-center gap-2">
              <div className={`status-indicator ${
                state.systemHealth === 'healthy' ? 'status-healthy' :
                state.systemHealth === 'warning' ? 'status-warning' : 'status-critical'
              }`}></div>
              <span className="text-sm text-gray-300">
                {state.systemHealth === 'healthy' ? 'System Healthy' :
                 state.systemHealth === 'warning' ? 'System Warning' : 'System Critical'}
              </span>
            </div>
          </div>

          <nav className="flex-1">
            <ul className="space-y-2">
              <li>
                <div
                  className={`nav-item ${state.currentView === 'dashboard' ? 'active' : ''}`}
                  onClick={() => setState(prev => ({ ...prev, currentView: 'dashboard' }))}
                >
                  <span className="text-lg">📊 Dashboard</span>
                </div>
              </li>
              <li>
                <div
                  className={`nav-item ${state.currentView === 'scanner' ? 'active' : ''}`}
                  onClick={() => setState(prev => ({ ...prev, currentView: 'scanner' }))}
                >
                  <span className="text-lg">🔍 Secret Scanner</span>
                </div>
              </li>
              <li>
                <div
                  className={`nav-item ${state.currentView === 'performance' ? 'active' : ''}`}
                  onClick={() => setState(prev => ({ ...prev, currentView: 'performance' }))}
                >
                  <span className="text-lg">⚡ Performance</span>
                </div>
              </li>
              <li>
                <div
                  className={`nav-item ${state.currentView === 'webhooks' ? 'active' : ''}`}
                  onClick={() => setState(prev => ({ ...prev, currentView: 'webhooks' }))}
                >
                  <span className="text-lg">🔗 Webhooks</span>
                </div>
              </li>
            </ul>
          </nav>

          {/* Hunter Status */}
          <div className="mt-8 p-4 glass-panel">
            <h3 className="text-sm font-semibold text-gray-300 mb-2">Hunter Status</h3>
            <div className="flex items-center gap-2">
              <div className={`w-2 h-2 rounded-full ${
                state.hunterStatus === 'scanning' ? 'bg-lava-yellow animate-pulse' :
                state.hunterStatus === 'error' ? 'bg-lava-red' : 'bg-lava-green'
              }`}></div>
              <span className="text-sm">
                {state.hunterStatus === 'scanning' ? 'Scanning...' :
                 state.hunterStatus === 'error' ? 'Error' : 'Idle'}
              </span>
            </div>
          </div>
        </div>

        {/* Main Content */}
        <div className="flex-1 p-4 overflow-hidden">
          <div className="h-full glass-panel p-6 overflow-y-auto">
            {state.isInitialized ? renderCurrentView() : (
              <div className="flex items-center justify-center h-full">
                <div className="text-center">
                  <div className="animate-spin w-12 h-12 border-4 border-lava-green border-t-transparent rounded-full mx-auto mb-4"></div>
                  <p className="text-gray-300">Initializing Secret Hunter...</p>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Notifications */}
      <div className="fixed top-4 right-4 z-50 space-y-2">
        {notifications.map(notification => (
          <div
            key={notification.id}
            className={`glass-panel p-4 max-w-sm transform transition-all duration-300 ${
              notification.type === 'error' ? 'border-lava-red' :
              notification.type === 'warning' ? 'border-lava-yellow' :
              notification.type === 'success' ? 'border-lava-green' : 'border-gray-600'
            }`}
          >
            <div className="flex items-start gap-3">
              <div className={`w-2 h-2 rounded-full mt-2 ${
                notification.type === 'error' ? 'bg-lava-red' :
                notification.type === 'warning' ? 'bg-lava-yellow' :
                notification.type === 'success' ? 'bg-lava-green' : 'bg-blue-400'
              }`}></div>
              <div className="flex-1">
                <p className="text-sm text-white">{notification.message}</p>
                <p className="text-xs text-gray-400 mt-1">
                  {notification.timestamp.toLocaleTimeString()}
                </p>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export default App;
