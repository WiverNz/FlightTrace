import { useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { createDemoRun } from '../api/runs';
import { ApiError } from '../api/client';

interface Props {
  onCreated?: () => void;
}

export function CreateDemoRunButton({ onCreated }: Props) {
  const navigate = useNavigate();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleClick() {
    setBusy(true);
    setError(null);
    try {
      const { id } = await createDemoRun();
      onCreated?.();
      navigate(`/runs/${id}`);
    } catch (err) {
      const message =
        err instanceof ApiError
          ? `[${err.status} ${err.code}] ${err.message}`
          : err instanceof Error
            ? err.message
            : 'failed to create demo run';
      setError(message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
      {error && <span className="badge fail">{error}</span>}
      <button
        type="button"
        className="button primary"
        onClick={handleClick}
        disabled={busy}
      >
        {busy ? 'creating…' : '+ Create demo failed run'}
      </button>
    </div>
  );
}
