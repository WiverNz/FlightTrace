import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';

import {
  getAssertions,
  getEvents,
  getFailureReport,
  getRun,
  getTelemetry,
} from '../api/runs';
import { ApiError } from '../api/client';
import type {
  AssertionResultDto,
  FailureReportDto,
  MissionRunDto,
  NormalizedEventDto,
  TelemetrySampleDto,
} from '../types/api';

import { StatusBadge } from '../components/StatusBadge';
import { FailureReportCard } from '../components/FailureReportCard';
import { FailureSummary } from '../components/FailureSummary';
import { MissionTimeline } from '../components/MissionTimeline';
import { AssertionTable } from '../components/AssertionTable';
import { TelemetryOverview } from '../components/TelemetryOverview';
import { RawEventLog } from '../components/RawEventLog';
import { LoadingState } from '../components/LoadingState';
import { ErrorState } from '../components/ErrorState';
import { formatDurationMs, formatIso, shortId } from '../format';

interface Loadable<T> {
  data: T | null;
  error: unknown;
}

const initial = <T,>(): Loadable<T> => ({ data: null, error: null });

export function RunDetailPage() {
  const { id } = useParams<{ id: string }>();
  const [run, setRun] = useState<Loadable<MissionRunDto>>(initial);
  const [report, setReport] = useState<Loadable<FailureReportDto>>(initial);
  const [events, setEvents] = useState<Loadable<NormalizedEventDto[]>>(initial);
  const [assertions, setAssertions] =
    useState<Loadable<AssertionResultDto[]>>(initial);
  const [telemetry, setTelemetry] =
    useState<Loadable<TelemetrySampleDto[]>>(initial);

  useEffect(() => {
    if (!id) return;
    const controller = new AbortController();
    const signal = controller.signal;

    setRun(initial());
    setReport(initial());
    setEvents(initial());
    setAssertions(initial());
    setTelemetry(initial());

    getRun(id, signal)
      .then((data) => setRun({ data, error: null }))
      .catch((err) => guard(err, () => setRun({ data: null, error: err })));

    getFailureReport(id, signal)
      .then((data) => setReport({ data, error: null }))
      .catch((err) => {
        if (err instanceof ApiError && err.isNotFound()) {
          setReport({ data: null, error: null });
          return;
        }
        guard(err, () => setReport({ data: null, error: err }));
      });

    getEvents(id, signal)
      .then((data) => setEvents({ data, error: null }))
      .catch((err) =>
        guard(err, () => setEvents({ data: null, error: err })),
      );

    getAssertions(id, signal)
      .then((data) => setAssertions({ data, error: null }))
      .catch((err) =>
        guard(err, () => setAssertions({ data: null, error: err })),
      );

    getTelemetry(id, signal)
      .then((data) => setTelemetry({ data, error: null }))
      .catch((err) =>
        guard(err, () => setTelemetry({ data: null, error: err })),
      );

    return () => controller.abort();
  }, [id]);

  if (!id) {
    return <p className="error">Missing run id in the URL.</p>;
  }

  if (run.error instanceof ApiError && run.error.isNotFound()) {
    return <RunNotFound id={id} />;
  }

  return (
    <>
      <Link to="/" className="back-link">
        ← all runs
      </Link>

      <header className="run-detail-header">
        <div>
          <h2>{run.data?.mission ?? '…'}</h2>
          <div className="run-detail-meta">
            <span>
              <span className="label">Run id</span>
              <span className="value">{shortId(id)}</span>
              <span className="faint mono"> ({id})</span>
            </span>
            {run.data && (
              <>
                <span>
                  <span className="label">Vehicle</span>
                  <span className="value">{run.data.vehicle}</span>
                </span>
                <span>
                  <span className="label">Source</span>
                  <span className="value">{run.data.source}</span>
                </span>
                <span>
                  <span className="label">Started</span>
                  <span className="value">{formatIso(run.data.startedAt)}</span>
                </span>
                <span>
                  <span className="label">Ended</span>
                  <span className="value">
                    {run.data.endedAt ? formatIso(run.data.endedAt) : '—'}
                  </span>
                </span>
                <span>
                  <span className="label">Duration</span>
                  <span className="value">
                    {run.data.endedAt
                      ? formatDurationMs(
                          Date.parse(run.data.endedAt) -
                            Date.parse(run.data.startedAt),
                        )
                      : '—'}
                  </span>
                </span>
              </>
            )}
          </div>
        </div>
        <div className="run-detail-status">
          {run.data ? (
            <StatusBadge
              status={run.data.status}
              failureReason={run.data.failureReason}
            />
          ) : (
            <span className="muted">loading…</span>
          )}
        </div>
      </header>

      {run.error != null &&
        !(run.error instanceof ApiError && run.error.isNotFound()) && (
          <ErrorState error={run.error} context="failed to load run" />
        )}

      {report.data && (
        <>
          <FailureSummary
            report={report.data}
            run={run.data}
            assertions={assertions.data}
            telemetry={telemetry.data}
            events={events.data}
          />
          <FailureReportCard
            report={report.data}
            events={events.data ?? []}
            startedAt={run.data?.startedAt ?? null}
          />
        </>
      )}
      {report.error != null && (
        <ErrorState error={report.error} context="failed to load report" />
      )}

      <section className="section">
        <header className="section-header">
          <h2>Mission timeline</h2>
          {run.data && (
            <span className="muted">{run.data.phases.length} phases</span>
          )}
        </header>
        <div className="section-body flush">
          {run.data ? (
            <MissionTimeline
              phases={run.data.phases}
              startedAt={run.data.startedAt}
            />
          ) : (
            <LoadingState label="loading timeline…" />
          )}
        </div>
      </section>

      <section className="section">
        <header className="section-header">
          <h2>Assertions</h2>
          {assertions.data && (
            <span className="muted">{assertions.data.length} evaluated</span>
          )}
        </header>
        <div className="section-body flush">
          {assertions.error != null && (
            <ErrorState
              error={assertions.error}
              context="failed to load assertions"
            />
          )}
          {assertions.error == null &&
            (assertions.data ? (
              <AssertionTable assertions={assertions.data} />
            ) : (
              <LoadingState label="loading assertions…" />
            ))}
        </div>
      </section>

      <section className="section">
        <header className="section-header">
          <h2>Telemetry overview</h2>
          {telemetry.data && (
            <span className="muted">{telemetry.data.length} samples</span>
          )}
        </header>
        <div className="section-body">
          {telemetry.error != null && (
            <ErrorState
              error={telemetry.error}
              context="failed to load telemetry"
            />
          )}
          {telemetry.error == null &&
            (telemetry.data ? (
              <TelemetryOverview samples={telemetry.data} />
            ) : (
              <LoadingState label="loading telemetry…" />
            ))}
        </div>
      </section>

      {events.error != null && (
        <ErrorState error={events.error} context="failed to load events" />
      )}
      {events.error == null &&
        (events.data ? (
          <RawEventLog events={events.data} />
        ) : (
          <section className="section">
            <header className="section-header">
              <h2>Raw event log</h2>
            </header>
            <div className="section-body">
              <LoadingState label="loading events…" />
            </div>
          </section>
        ))}
    </>
  );
}

function RunNotFound({ id }: { id: string }) {
  return (
    <div className="section">
      <div className="section-body">
        <h2 style={{ marginTop: 0 }}>Run not found</h2>
        <p className="muted">
          No run with id <code className="mono">{id}</code> exists on the
          backend.
        </p>
        <Link to="/" className="button">
          ← back to runs
        </Link>
      </div>
    </div>
  );
}

function guard(err: unknown, fn: () => void) {
  if ((err as { name?: string }).name === 'AbortError') return;
  fn();
}
