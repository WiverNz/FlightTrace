import { useMemo } from 'react';

import type { TelemetrySampleDto } from '../types/api';
import { fmtNumber } from '../format';

interface Props {
  samples: TelemetrySampleDto[];
}

const WIDTH = 220;
const HEIGHT = 40;

export function VerticalSpeedSummary({ samples }: Props) {
  const stats = useMemo(() => {
    if (samples.length === 0) return null;
    const values = samples.map((s) => s.verticalSpeedMps).filter(Number.isFinite);
    if (values.length === 0) return null;
    const peakClimb = Math.max(...values);
    const peakDescent = Math.min(...values);
    const mean =
      values.reduce((acc, v) => acc + v, 0) / values.length;
    return { values, peakClimb, peakDescent, mean };
  }, [samples]);

  if (!stats) {
    return null;
  }

  const { values, peakClimb, peakDescent, mean } = stats;
  const min = peakDescent;
  const max = peakClimb;
  const span = Math.max(max - min, 0.5);
  const stepX = values.length > 1 ? WIDTH / (values.length - 1) : WIDTH;
  // y=0 baseline so positive (climb) is above, negative (descent) below.
  const zeroY = HEIGHT * (max / span);
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
          <dt>peak climb</dt>
          <dd className="up">{fmtNumber(peakClimb, 2)}</dd>
        </div>
        <div>
          <dt>peak descent</dt>
          <dd className="down">{fmtNumber(peakDescent, 2)}</dd>
        </div>
        <div>
          <dt>mean</dt>
          <dd>{fmtNumber(mean, 2)}</dd>
        </div>
      </dl>
    </div>
  );
}
