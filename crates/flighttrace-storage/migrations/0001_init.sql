-- Initial FlightTrace schema.
--
-- All timestamps are stored as RFC 3339 strings so the on-disk format matches
-- the JSON wire format. Run-scoped child tables cascade on parent delete.

CREATE TABLE mission_runs (
    id              TEXT PRIMARY KEY,
    vehicle         TEXT NOT NULL,
    mission         TEXT NOT NULL,
    source          TEXT NOT NULL,
    status          TEXT NOT NULL,
    failure_reason  TEXT,
    started_at      TEXT NOT NULL,
    ended_at        TEXT
);

CREATE INDEX mission_runs_started_at_idx ON mission_runs (started_at DESC);

CREATE TABLE mission_phase_transitions (
    run_id   TEXT    NOT NULL REFERENCES mission_runs(id) ON DELETE CASCADE,
    ordinal  INTEGER NOT NULL,
    at       TEXT    NOT NULL,
    phase    TEXT    NOT NULL,
    PRIMARY KEY (run_id, ordinal)
);

CREATE TABLE normalized_events (
    id       TEXT PRIMARY KEY,
    run_id   TEXT NOT NULL REFERENCES mission_runs(id) ON DELETE CASCADE,
    at       TEXT NOT NULL,
    kind     TEXT NOT NULL,
    payload  TEXT NOT NULL
);

CREATE INDEX normalized_events_run_at_idx ON normalized_events (run_id, at);

CREATE TABLE telemetry_samples (
    run_id                 TEXT NOT NULL REFERENCES mission_runs(id) ON DELETE CASCADE,
    at                     TEXT NOT NULL,
    altitude_m             REAL NOT NULL,
    vertical_speed_mps     REAL NOT NULL,
    mode                   TEXT NOT NULL,
    armed                  INTEGER NOT NULL,
    battery_remaining_pct  REAL NOT NULL,
    PRIMARY KEY (run_id, at)
);

CREATE INDEX telemetry_samples_run_at_idx ON telemetry_samples (run_id, at);

CREATE TABLE assertion_results (
    id            TEXT PRIMARY KEY,
    run_id        TEXT NOT NULL REFERENCES mission_runs(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    outcome       TEXT NOT NULL,
    detail        TEXT NOT NULL,
    evaluated_at  TEXT NOT NULL
);

CREATE INDEX assertion_results_run_idx ON assertion_results (run_id);

CREATE TABLE failure_reports (
    id             TEXT PRIMARY KEY,
    run_id         TEXT NOT NULL UNIQUE REFERENCES mission_runs(id) ON DELETE CASCADE,
    reason         TEXT NOT NULL,
    summary        TEXT NOT NULL,
    evidence_json  TEXT NOT NULL,
    likely_cause   TEXT NOT NULL,
    generated_at   TEXT NOT NULL
);
