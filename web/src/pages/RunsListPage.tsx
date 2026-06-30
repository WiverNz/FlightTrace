import { useCallback, useEffect, useState } from 'react';

import { listRuns } from '../api/runs';
import type { MissionRunDto } from '../types/api';
import { RunsTable } from '../components/RunsTable';
import { CreateDemoRunButton } from '../components/CreateDemoRunButton';
import { LoadingState } from '../components/LoadingState';
import { ErrorState } from '../components/ErrorState';

export function RunsListPage() {
  const [runs, setRuns] = useState<MissionRunDto[] | null>(null);
  const [error, setError] = useState<unknown>(null);

  const refresh = useCallback(async (signal?: AbortSignal) => {
    try {
      const data = await listRuns(signal);
      setRuns(data);
      setError(null);
    } catch (err) {
      if ((err as { name?: string }).name === 'AbortError') return;
      setError(err);
    }
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    refresh(controller.signal);
    return () => controller.abort();
  }, [refresh]);

  return (
    <>
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: 24,
        }}
      >
        <div>
          <h2 style={{ margin: 0, fontSize: 18 }}>Mission runs</h2>
          <p className="muted" style={{ margin: '4px 0 0 0' }}>
            Click a run to inspect why it failed.
          </p>
        </div>
        <CreateDemoRunButton onCreated={() => refresh()} />
      </div>

      <section className="section">
        <header className="section-header">
          <h2>Runs</h2>
          {runs && <span className="muted">{runs.length} total</span>}
        </header>
        <div className="section-body flush">
          {error != null && (
            <ErrorState error={error} context="failed to load runs" />
          )}
          {error == null && runs === null && (
            <LoadingState label="loading runs…" />
          )}
          {error == null && runs !== null && <RunsTable runs={runs} />}
        </div>
      </section>
    </>
  );
}
