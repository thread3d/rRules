-- Phase 19-04: Historical Coverage Storage Schema
-- Stores rule execution coverage data for trend analysis and long-term monitoring

-- Coverage history: records each rule evaluation/firing event
CREATE TABLE IF NOT EXISTS coverage_history (
    id SERIAL PRIMARY KEY,
    rule_id UUID NOT NULL,
    rule_name VARCHAR(255) NOT NULL,
    evaluated BOOLEAN NOT NULL DEFAULT FALSE,
    fired BOOLEAN NOT NULL DEFAULT FALSE,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    
    -- Execution context for analysis
    session_id TEXT,
    engine_type VARCHAR(50),
    facts_count INTEGER,
    execution_duration_ms DECIMAL(10, 3)
);

-- Coverage snapshots: periodic aggregations for trend analysis
CREATE TABLE IF NOT EXISTS coverage_snapshots (
    id SERIAL PRIMARY KEY,
    snapshot_time TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    
    -- Aggregated metrics
    total_rules INTEGER NOT NULL,
    evaluated_rules INTEGER NOT NULL,
    fired_rules INTEGER NOT NULL,
    overall_coverage_percent DECIMAL(5, 2) NOT NULL,
    
    -- Per-engine breakdown (JSONB)
    engine_breakdown JSONB,
    
    UNIQUE(snapshot_time)
);

-- Daily rollup: aggregated daily metrics for efficient trend queries
CREATE TABLE IF NOT EXISTS coverage_daily_rollup (
    id SERIAL PRIMARY KEY,
    date DATE NOT NULL UNIQUE,
    
    total_rules INTEGER NOT NULL,
    evaluated_rules INTEGER NOT NULL,
    fired_rules INTEGER NOT NULL,
    overall_coverage_percent DECIMAL(5, 2) NOT NULL,
    
    -- Aggregate execution stats
    total_evaluations BIGINT NOT NULL DEFAULT 0,
    total_fires BIGINT NOT NULL DEFAULT 0,
    avg_execution_duration_ms DECIMAL(10, 3),
    
    UNIQUE(date)
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_coverage_history_rule ON coverage_history(rule_id);
CREATE INDEX IF NOT EXISTS idx_coverage_history_timestamp ON coverage_history(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_coverage_snapshots_time ON coverage_snapshots(snapshot_time DESC);
CREATE INDEX IF NOT EXISTS idx_coverage_daily_rollup_date ON coverage_daily_rollup(date DESC);

-- Function to update daily rollup from recent history
CREATE OR REPLACE FUNCTION update_coverage_rollup()
RETURNS TRIGGER AS $$
DECLARE
    yesterday DATE;
BEGIN
    -- Update the rollup for today based on current data
    INSERT INTO coverage_daily_rollup (date, total_rules, evaluated_rules, fired_rules, 
                                       overall_coverage_percent, total_evaluations, total_fires)
    SELECT 
        CURRENT_DATE,
        COALESCE(COUNT(DISTINCT rule_id), 0),
        COALESCE(SUM(CASE WHEN evaluated THEN 1 ELSE 0 END), 0),
        COALESCE(SUM(CASE WHEN fired THEN 1 ELSE 0 END), 0),
        COALESCE(ROUND(
            (SUM(CASE WHEN evaluated THEN 1.0 ELSE 0.0 END) / NULLIF(COUNT(*), 0)) * 100, 2
        ), 0.0),
        COUNT(*) AS total_evaluations,
        SUM(CASE WHEN fired THEN 1 ELSE 0 END) AS total_fires
    FROM coverage_history
    WHERE timestamp::date = CURRENT_DATE
    ON CONFLICT (date) DO UPDATE SET
        total_rules = EXCLUDED.total_rules,
        evaluated_rules = EXCLUDED.evaluated_rules,
        fired_rules = EXCLUDED.fired_rules,
        overall_coverage_percent = EXCLUDED.overall_coverage_percent,
        total_evaluations = coverage_daily_rollup.total_evaluations + EXCLUDED.total_evaluations,
        total_fires = coverage_daily_rollup.total_fires + EXCLUDED.total_fires;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to update rollup on new history records
CREATE TRIGGER trg_update_coverage_rollup
    AFTER INSERT ON coverage_history
    EXECUTE FUNCTION update_coverage_rollup();
