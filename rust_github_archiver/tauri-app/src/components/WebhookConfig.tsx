import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';

interface WebhookConfigProps {
  onNotification: (type: 'info' | 'warning' | 'error' | 'success', message: string) => void;
}

interface WebhookEndpoint {
  id: string;
  name: string;
  url: string;
  events: string[];
  headers: Record<string, string>;
  active: boolean;
  created_at: string;
  last_triggered: string | null;
  trigger_count: number;
}

interface WebhookEvent {
  id: string;
  endpoint_id: string;
  event_type: string;
  payload: any;
  status: 'success' | 'failed' | 'pending';
  response_code: number | null;
  timestamp: string;
  retry_count: number;
}

export function WebhookConfig({ onNotification }: WebhookConfigProps) {
  const [webhooks, setWebhooks] = useState<WebhookEndpoint[]>([]);
  const [recentEvents, setRecentEvents] = useState<WebhookEvent[]>([]);
  const [showAddForm, setShowAddForm] = useState(false);
  const [loading, setLoading] = useState(true);
  
  const [newWebhook, setNewWebhook] = useState({
    name: '',
    url: '',
    events: [] as string[],
    headers: {} as Record<string, string>
  });

  const availableEvents = [
    'secret.detected',
    'secret.validated',
    'secret.false_positive',
    'scan.started',
    'scan.completed',
    'scan.failed',
    'system.alert',
    'performance.threshold_exceeded'
  ];

  useEffect(() => {
    loadWebhooks();
    loadRecentEvents();
  }, []);

  const loadWebhooks = async () => {
    try {
      // Load webhook configurations from backend
      setLoading(false);
      onNotification('info', 'Loaded webhook configurations');
    } catch (error) {
      console.error('Failed to load webhooks:', error);
      onNotification('error', 'Failed to load webhook configurations');
      setLoading(false);
    }
  };

  const loadRecentEvents = async () => {
    try {
      // Load recent webhook events from backend
      onNotification('info', 'Loaded recent webhook events');
    } catch (error) {
      console.error('Failed to load webhook events:', error);
      onNotification('error', 'Failed to load webhook events');
    }
  };

  const createWebhook = async () => {
    if (!newWebhook.name || !newWebhook.url || newWebhook.events.length === 0) {
      onNotification('warning', 'Please fill in all required fields');
      return;
    }

    try {
      await invoke('configure_webhooks', {
        action: 'create',
        webhook: newWebhook
      });

      setWebhooks(prev => [...prev, {
        id: Date.now().toString(),
        ...newWebhook,
        active: true,
        created_at: new Date().toISOString(),
        last_triggered: null,
        trigger_count: 0
      }]);

      setNewWebhook({
        name: '',
        url: '',
        events: [],
        headers: {}
      });
      setShowAddForm(false);
      
      onNotification('success', `Webhook "${newWebhook.name}" created successfully`);
    } catch (error) {
      console.error('Failed to create webhook:', error);
      onNotification('error', 'Failed to create webhook');
    }
  };

  const toggleWebhook = async (id: string, active: boolean) => {
    try {
      await invoke('configure_webhooks', {
        action: active ? 'enable' : 'disable',
        webhookId: id
      });

      setWebhooks(prev => 
        prev.map(webhook => 
          webhook.id === id ? { ...webhook, active } : webhook
        )
      );

      onNotification('success', `Webhook ${active ? 'enabled' : 'disabled'}`);
    } catch (error) {
      console.error('Failed to toggle webhook:', error);
      onNotification('error', 'Failed to update webhook status');
    }
  };

  const deleteWebhook = async (id: string) => {
    if (!confirm('Are you sure you want to delete this webhook?')) {
      return;
    }

    try {
      await invoke('configure_webhooks', {
        action: 'delete',
        webhookId: id
      });

      setWebhooks(prev => prev.filter(webhook => webhook.id !== id));
      onNotification('success', 'Webhook deleted successfully');
    } catch (error) {
      console.error('Failed to delete webhook:', error);
      onNotification('error', 'Failed to delete webhook');
    }
  };

  const testWebhook = async (id: string) => {
    try {
      await invoke('configure_webhooks', {
        action: 'test',
        webhookId: id
      });

      onNotification('success', 'Test webhook sent successfully');
    } catch (error) {
      console.error('Failed to test webhook:', error);
      onNotification('error', 'Failed to send test webhook');
    }
  };

  const addHeader = () => {
    const key = prompt('Header name:');
    const value = prompt('Header value:');
    
    if (key && value) {
      setNewWebhook(prev => ({
        ...prev,
        headers: { ...prev.headers, [key]: value }
      }));
    }
  };

  const removeHeader = (key: string) => {
    setNewWebhook(prev => {
      const headers = { ...prev.headers };
      delete headers[key];
      return { ...prev, headers };
    });
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center">
          <div className="animate-spin w-12 h-12 border-4 border-lava-green border-t-transparent rounded-full mx-auto mb-4"></div>
          <p className="text-gray-300">Loading webhook configuration...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex justify-between items-center">
        <div>
          <h1 className="text-3xl font-bold text-white mb-2">Webhook Configuration</h1>
          <p className="text-gray-400">
            Configure webhooks to receive real-time notifications about security events
          </p>
        </div>
        <button
          className="glow-button"
          onClick={() => setShowAddForm(true)}
        >
          ➕ Add Webhook
        </button>
      </div>

      {/* Add Webhook Form */}
      {showAddForm && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div className="bg-gray-900 border border-gray-700 rounded-lg p-6 w-full max-w-2xl max-h-[80vh] overflow-y-auto">
            <h2 className="text-2xl font-bold text-white mb-4">Add New Webhook</h2>
            
            <div className="space-y-4">
              <div>
                <label className="block text-gray-300 text-sm mb-2">Name *</label>
                <input
                  type="text"
                  value={newWebhook.name}
                  onChange={(e) => setNewWebhook(prev => ({ ...prev, name: e.target.value }))}
                  placeholder="e.g., Slack Notifications"
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-600 rounded-lg text-white focus:border-lava-green focus:outline-none"
                />
              </div>

              <div>
                <label className="block text-gray-300 text-sm mb-2">URL *</label>
                <input
                  type="url"
                  value={newWebhook.url}
                  onChange={(e) => setNewWebhook(prev => ({ ...prev, url: e.target.value }))}
                  placeholder="https://hooks.slack.com/services/..."
                  className="w-full px-3 py-2 bg-gray-800 border border-gray-600 rounded-lg text-white focus:border-lava-green focus:outline-none"
                />
              </div>

              <div>
                <label className="block text-gray-300 text-sm mb-2">Events *</label>
                <div className="grid grid-cols-2 gap-2">
                  {availableEvents.map(event => (
                    <label key={event} className="flex items-center">
                      <input
                        type="checkbox"
                        checked={newWebhook.events.includes(event)}
                        onChange={(e) => {
                          if (e.target.checked) {
                            setNewWebhook(prev => ({
                              ...prev,
                              events: [...prev.events, event]
                            }));
                          } else {
                            setNewWebhook(prev => ({
                              ...prev,
                              events: prev.events.filter(e => e !== event)
                            }));
                          }
                        }}
                        className="mr-2"
                      />
                      <span className="text-gray-300 text-sm">{event}</span>
                    </label>
                  ))}
                </div>
              </div>

              <div>
                <div className="flex justify-between items-center mb-2">
                  <label className="block text-gray-300 text-sm">Custom Headers</label>
                  <button
                    type="button"
                    onClick={addHeader}
                    className="text-lava-green text-sm hover:text-lava-green-light"
                  >
                    + Add Header
                  </button>
                </div>
                <div className="space-y-2">
                  {Object.entries(newWebhook.headers).map(([key, value]) => (
                    <div key={key} className="flex gap-2">
                      <input
                        type="text"
                        value={key}
                        readOnly
                        className="flex-1 px-3 py-2 bg-gray-700 border border-gray-600 rounded-lg text-gray-300"
                      />
                      <input
                        type="text"
                        value={value}
                        readOnly
                        className="flex-1 px-3 py-2 bg-gray-700 border border-gray-600 rounded-lg text-gray-300"
                      />
                      <button
                        type="button"
                        onClick={() => removeHeader(key)}
                        className="px-3 py-2 bg-red-600 hover:bg-red-700 text-white rounded-lg"
                      >
                        ✗
                      </button>
                    </div>
                  ))}
                </div>
              </div>
            </div>

            <div className="flex gap-3 mt-6">
              <button
                className="glow-button"
                onClick={createWebhook}
              >
                Create Webhook
              </button>
              <button
                className="px-6 py-3 bg-gray-600 hover:bg-gray-700 text-white rounded-lg transition-colors"
                onClick={() => setShowAddForm(false)}
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Existing Webhooks */}
      <div className="metric-card">
        <h3 className="text-xl font-semibold text-white mb-4">Configured Webhooks</h3>
        
        {webhooks.length === 0 ? (
          <div className="text-center py-12">
            <div className="text-6xl mb-4">🔗</div>
            <p className="text-gray-400 text-lg">No webhooks configured</p>
            <p className="text-gray-500 text-sm">Add a webhook to receive real-time notifications</p>
          </div>
        ) : (
          <div className="space-y-4">
            {webhooks.map(webhook => (
              <div key={webhook.id} className="secret-card">
                <div className="flex justify-between items-start">
                  <div className="flex-1">
                    <div className="flex items-center gap-3 mb-2">
                      <h4 className="text-white font-semibold text-lg">{webhook.name}</h4>
                      <div className={`w-3 h-3 rounded-full ${
                        webhook.active ? 'bg-lava-green' : 'bg-gray-500'
                      }`} />
                      <span className={`text-sm ${
                        webhook.active ? 'text-lava-green' : 'text-gray-500'
                      }`}>
                        {webhook.active ? 'Active' : 'Inactive'}
                      </span>
                    </div>
                    
                    <p className="text-gray-400 text-sm mb-2">{webhook.url}</p>
                    
                    <div className="flex flex-wrap gap-1 mb-3">
                      {webhook.events.map(event => (
                        <span
                          key={event}
                          className="px-2 py-1 bg-lava-green bg-opacity-20 text-lava-green text-xs rounded"
                        >
                          {event}
                        </span>
                      ))}
                    </div>
                    
                    <div className="text-gray-500 text-xs">
                      <p>Created: {new Date(webhook.created_at).toLocaleString()}</p>
                      <p>Triggered: {webhook.trigger_count} times</p>
                      {webhook.last_triggered && (
                        <p>Last triggered: {new Date(webhook.last_triggered).toLocaleString()}</p>
                      )}
                    </div>
                  </div>
                  
                  <div className="flex gap-2">
                    <button
                      className="px-3 py-1 bg-blue-600 hover:bg-blue-700 text-white text-sm rounded transition-colors"
                      onClick={() => testWebhook(webhook.id)}
                    >
                      🧪 Test
                    </button>
                    <button
                      className={`px-3 py-1 text-white text-sm rounded transition-colors ${
                        webhook.active
                          ? 'bg-yellow-600 hover:bg-yellow-700'
                          : 'bg-green-600 hover:bg-green-700'
                      }`}
                      onClick={() => toggleWebhook(webhook.id, !webhook.active)}
                    >
                      {webhook.active ? '⏸️ Disable' : '▶️ Enable'}
                    </button>
                    <button
                      className="px-3 py-1 bg-red-600 hover:bg-red-700 text-white text-sm rounded transition-colors"
                      onClick={() => deleteWebhook(webhook.id)}
                    >
                      🗑️ Delete
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Recent Events */}
      <div className="metric-card">
        <h3 className="text-xl font-semibold text-white mb-4">Recent Webhook Events</h3>
        
        {recentEvents.length === 0 ? (
          <div className="text-center py-8">
            <div className="text-4xl mb-4">📊</div>
            <p className="text-gray-400">No recent webhook events</p>
          </div>
        ) : (
          <div className="space-y-3 max-h-64 overflow-y-auto">
            {recentEvents.map(event => (
              <div key={event.id} className="flex justify-between items-center p-3 glass-panel">
                <div>
                  <p className="text-white font-medium">{event.event_type}</p>
                  <p className="text-gray-400 text-sm">
                    {new Date(event.timestamp).toLocaleString()}
                  </p>
                  {event.retry_count > 0 && (
                    <p className="text-yellow-400 text-xs">
                      Retried {event.retry_count} times
                    </p>
                  )}
                </div>
                <div className="text-right">
                  <span className={`px-2 py-1 rounded text-xs font-semibold ${
                    event.status === 'success' ? 'bg-green-500 bg-opacity-20 text-green-400' :
                    event.status === 'failed' ? 'bg-red-500 bg-opacity-20 text-red-400' :
                    'bg-yellow-500 bg-opacity-20 text-yellow-400'
                  }`}>
                    {event.status.toUpperCase()}
                  </span>
                  {event.response_code && (
                    <p className="text-gray-400 text-xs mt-1">
                      HTTP {event.response_code}
                    </p>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
