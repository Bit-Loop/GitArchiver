CREATE TABLE IF NOT EXISTS research_findings (
    id UUID PRIMARY KEY,
    title TEXT NOT NULL,
    status VARCHAR(40) NOT NULL DEFAULT 'draft',
    source_type VARCHAR(40) NOT NULL,
    source_detection_id UUID REFERENCES secret_detections(detection_id) ON DELETE SET NULL,
    source_event_id BIGINT REFERENCES github_events(event_id) ON DELETE SET NULL,
    program_name TEXT,
    scope_asset TEXT,
    scope_status VARCHAR(40) NOT NULL DEFAULT 'unknown',
    playbook VARCHAR(80),
    severity VARCHAR(50),
    repository TEXT,
    raw_evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    derived_metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    notes TEXT,
    readiness_score INTEGER NOT NULL DEFAULT 0,
    readiness_blockers JSONB NOT NULL DEFAULT '[]'::jsonb,
    ai_outputs JSONB NOT NULL DEFAULT '[]'::jsonb,
    export_history JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_by VARCHAR(255) NOT NULL,
    updated_by VARCHAR(255) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_research_findings_updated
    ON research_findings (updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_research_findings_source_detection
    ON research_findings (source_detection_id);
CREATE INDEX IF NOT EXISTS idx_research_findings_source_event
    ON research_findings (source_event_id);
CREATE INDEX IF NOT EXISTS idx_research_findings_repository
    ON research_findings (repository);
