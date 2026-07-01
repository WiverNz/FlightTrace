import { useMemo } from 'react';

import type { TelemetrySampleDto } from '../types/api';
import { fmtNumber } from '../format';

interface Props {
  samples: TelemetrySampleDto[];
}

interface VSpeedStats {
  values: number[];
  maxClimb: number;
  final: number;
  stalled: boolean;
}

const WIDTH = 220;
const HEIGHT = 40;

// A short tail of the run whose average absolute vertical speed we compare
// against the stall threshold. Small enough to react to the last few seconds
// of a demo run (~200 ms samples); large enough to ride out a single blip.
const STALL_WINDOW = 5;
const STALL_ABS_MPS = 0.5;

export function VerticalSpeedSummary({ samples }: Props) {
  const stats = useMemo<VSpeedStats | null>(() => {
    if (samples.length === 0) return null;
    const values = samples.map((s) => s.verticalSpeedMps).filter(Number.isFinite);
    if (values.length === 0) return null;

    const maxClimb = Math.max(...values);
    const final = values[values.length - 1]!;

    // Stall = both the recent-tail average AND the final sample are near
    // zero. "Near" is generous — a real climb is > 1 m/s in the demo, so
    // 0.5 m/s comfortably rejects noise.
    const tail = values.slice(-STALL_WINDOW);
    const tailMean = tail.reduce((acc, v) => acc + v, 0) / tail.length;
    const stalled =
      Math.abs(tailMean) < STALL_ABS_MPS && Math.abs(final) < STALL_ABS_MPS;

    return { values, maxClimb, final, stalled };
  }, [samples]);

  if (!stats) {
    return null;
  }

  const { values, maxClimb, final, stalled } = stats;
  const min = Math.min(...values, 0);
  const max = Math.max(...values, 0);
  const span = Math.max(max - min, 0.5);
  const stepX = values.length > 1 ? WIDTH / (values.length - 1) : WIDTH;
  const zeroY = HEIGHT - ((0 - min) / span) * HEIGHT;
  const polyline = values
    .map((v, i) => {
      const x = i * stepX;
      const y = HEIGHT - ((v - min) / span) * HEIGHT;
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(' ');

  return (
    <div className="vspeed-summary">
      <header>
        <h3>Vertical speed (m/s)</h3>
      </header>
      <svg
        viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
        role="img"
        aria-label="vertical speed sparkline"
        preserveAspectRatio="none"
        className="vspeed-spark"
      >
        <line
          x1={0}
          x2={WIDTH}
          y1={zeroY}
          y2={zeroY}
          className="vspeed-zero"
        />
        <polyline points={polyline} className="vspeed-line" />
      </svg>
      <dl className="vspeed-stats">
        <div>
          <dt>max climb</dt>
          <dd className="up">{fmtNumber(maxClimb, 2)}</dd>
        </div>
        <div>
          <dt>final</dt>
          <dd className={final < 0 ? 'down' : ''}>{fmtNumber(final, 2)}</dd>
        </div>
        <div>
          <dt>climb</dt>
          <dd className={stalled ? 'stalled' : 'ok'}>
            {stalled ? 'stalled' : 'ok'}
          </dd>
        </div>
      </dl>
    </div>
  );
}
