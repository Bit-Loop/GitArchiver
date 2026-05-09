-- Audit Logging System
-- Tracks all security-sensitive operations for compliance and forensics

CREATE TABLE IF NOT EXISTS audit_logs (
    id BIGSERIAL PRIMARY KEY,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    user_id BIGINT REFERENCES users(id) ON DELETE SET NULL,
    username VARCHAR(255) NOT NULL,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT,
    ip_address VARCHAR(45),  -- IPv4 or IPv6
    user_agent TEXT,
    status TEXT NOT NULL,  -- 'success', 'failure', 'warning'
    details JSONB NOT NULL DEFAULT '{}',
    error_message TEXT,
    
    -- Indexes for common queries
    CONSTRAINT check_status CHECK (status IN ('success', 'failure', 'warning'))
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_audit_logs_timestamp ON audit_logs(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_username ON audit_logs(username);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs(action);
CREATE INDEX IF NOT EXISTS idx_audit_logs_resource_type ON audit_logs(resource_type);
CREATE INDEX IF NOT EXISTS idx_audit_logs_status ON audit_logs(status);
CREATE INDEX IF NOT EXISTS idx_audit_logs_ip_address ON audit_logs(ip_address);

-- Composite index for common filter combinations
CREATE INDEX IF NOT EXISTS idx_audit_logs_user_timestamp ON audit_logs(user_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action_timestamp ON audit_logs(action, timestamp DESC);

-- JSONB index for details field
CREATE INDEX IF NOT EXISTS idx_audit_logs_details_gin ON audit_logs USING GIN (details);

-- Comments for documentation
COMMENT ON TABLE audit_logs IS 'Audit trail for all security-sensitive operations';
COMMENT ON COLUMN audit_logs.id IS 'Unique identifier for audit log entry';
COMMENT ON COLUMN audit_logs.timestamp IS 'When the action occurred';
COMMENT ON COLUMN audit_logs.user_id IS 'ID of user who performed the action (NULL if user deleted)';
COMMENT ON COLUMN audit_logs.username IS 'Username at time of action (preserved even if user deleted)';
COMMENT ON COLUMN audit_logs.action IS 'Type of action performed (JSON enum)';
COMMENT ON COLUMN audit_logs.resource_type IS 'Type of resource affected (JSON enum)';
COMMENT ON COLUMN audit_logs.resource_id IS 'Identifier of specific resource affected';
COMMENT ON COLUMN audit_logs.ip_address IS 'IP address of client making the request';
COMMENT ON COLUMN audit_logs.user_agent IS 'User agent string of client';
COMMENT ON COLUMN audit_logs.status IS 'Result of the action (success/failure/warning)';
COMMENT ON COLUMN audit_logs.details IS 'Additional context (JSONB for flexibility)';
COMMENT ON COLUMN audit_logs.error_message IS 'Error details if action failed';

-- Example queries for common use cases:
-- Find all actions by a user:
--   SELECT * FROM audit_logs WHERE user_id = 123 ORDER BY timestamp DESC;
--
-- Find all failed login attempts:
--   SELECT * FROM audit_logs WHERE action = '"login_failure"' ORDER BY timestamp DESC;
--
-- Find all actions in the last 24 hours:
--   SELECT * FROM audit_logs WHERE timestamp >= NOW() - INTERVAL '24 hours';
--
-- Find suspicious activity from an IP:
--   SELECT * FROM audit_logs WHERE ip_address = '1.2.3.4' AND status = 'failure';
--
-- Export for compliance audit:
--   COPY (SELECT * FROM audit_logs WHERE timestamp >= '2025-01-01') TO '/tmp/audit_export.csv' CSV HEADER;
