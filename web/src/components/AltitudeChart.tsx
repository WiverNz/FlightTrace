import { useMemo } from 'react';

import type { TelemetrySampleDto } from '../types/api';
import { formatDurationMs, fmtNumber } from '../format';

export interface AltitudeChartMarker {
  at: string;
  label: string;
  tone: 'fail' | 'warn';
}

interface Props {
  samples: TelemetrySampleDto[];
  startedAt: string;
  targetAltitudeM?: number | null;
  markers?: AltitudeChartMarker[];
  variant?: 'compact' | 'full';
}

interface Layout {
  width: number;
  height: number;
  padLeft: number;
  padRight: number;
  padTop: number;
  padBottom: number;
}

const LAYOUTS: Record<NonNullable<Props['variant']>, Layout> = {
  compact: {
    width: 480,
    height: 160,
    padLeft: 36,
    padRight: 16,
    padTop: 12,
    padBottom: 22,
  },
  full: {
    width: 720,
    height: 240,
    padLeft: 44,
    padRight: 20,
    padTop: 16,
    padBottom: 28,
  },
};

export function AltitudeChart({
  samples,
  startedAt,
  targetAltitudeM,
  markers = [],
  variant = 'full',
}: Props) {
  const layout = LAYOUTS[variant];
  const originMs = Date.parse(startedAt);

  const data = useMemo(() => {
    if (samples.length === 0) return null;
    const points = samples
      .map((s) => ({
        tMs: Date.parse(s.at) - originMs,
        altitude: s.altitudeM,
      }))
      .filter(
        (p) =>
          Number.isFinite(p.tMs) &&
          p.tMs >= 0 &&
          Number.isFinite(p.altitude),
      );
    if (points.length === 0) return null;

    const tMax = points[points.length - 1]!.tMs;
    const tMin = points[0]!.tMs;
    const altMax = Math.max(
      ...points.map((p) => p.altitude),
      targetAltitudeM ?? 0,
    );
    const altMin = Math.min(...points.map((p) => p.altitude), 0);

    return { points, tMin, tMax, altMin, altMax };
  }, [samples, originMs, targetAltitudeM]);

  if (!data) {
    return <p className="muted">No altitude samples to plot.</p>;
  }

  const { points, tMin, tMax, altMin, altMax } = data;
  const innerWidth = layout.width - layout.padLeft - layout.padRight;
  const innerHeight = layout.height - layout.padTop - layout.padBottom;

  // Pad y-axis a bit so the target line and peaks aren't flush with the edge.
  const yPad = Math.max(1, (altMax - altMin) * 0.08);
  const yTop = altMax + yPad;
  const yBottom = Math.min(altMin, 0);
  const ySpan = yTop - yBottom || 1;

  const xSpan = tMax - tMin || 1;
  const xAt = (tMs: number) =>
    layout.padLeft + ((tMs - tMin) / xSpan) * innerWidth;
  const yAt = (alt: number) =>
    layout.padTop + (1 - (alt - yBottom) / ySpan) * innerHeight;

  const polylinePoints = points
    .map((p) => `${xAt(p.tMs).toFixed(2)},${yAt(p.altitude).toFixed(2)}`)
    .join(' ');

  const areaPath =
    `M ${xAt(points[0]!.tMs).toFixed(2)},${yAt(yBottom).toFixed(2)} ` +
    points
      .map((p) => `L ${xAt(p.tMs).toFixed(2)},${yAt(p.altitude).toFixed(2)}`)
      .join(' ') +
    ` L ${xAt(points[points.length - 1]!.tMs).toFixed(2)},${yAt(yBottom).toFixed(2)} Z`;

  // y-axis grid: aim for ~4 ticks.
  const tickValues = niceTicks(yBottom, yTop, 4);

  const target = targetAltitudeM ?? null;
  const showTarget =
    target != null && Number.isFinite(target) && target >= yBottom && target <= yTop;

  const validMarkers = markers
    .map((m) => ({ ...m, tMs: Date.parse(m.at) - originMs }))
    .filter((m) => Number.isFinite(m.tMs) && m.tMs >= tMin && m.tMs <= tMax);

  const peakAlt = Math.max(...points.map((p) => p.altitude));
  const gap = target != null ? target - peakAlt : null;

  return (
    <figure className={`altitude-chart altitude-chart--${variant}`}>
      <svg
        width="100%"
        viewBox={`0 0 ${layout.width} ${layout.height}`}
        role="img"
        aria-label="altitude over time"
        preserveAspectRatio="none"
      >
        {tickValues.map((tick) => (
          <g key={tick} className="altitude-chart-grid">
            <line
              x1={layout.padLeft}
              x2={layout.width - layout.padRight}
              y1={yAt(tick)}
              y2={yAt(tick)}
            />
            <text
              x={layout.padLeft - 6}
              y={yAt(tick)}
              textAnchor="end"
              dominantBaseline="central"
              className="altitude-chart-tick"
            >
              {fmtNumber(tick, tick >= 10 ? 0 : 1)}
            </text>
          </g>
        ))}

        <path d={areaPath} className="altitude-chart-area" />
        <polyline points={polylinePoints} className="altitude-chart-line" />

        {showTarget && target != null && (
          <g className="altitude-chart-target">
            <line
              x1={layout.padLeft}
              x2={layout.width - layout.padRight}
              y1={yAt(target)}
              y2={yAt(target)}
            />
            <text
              x={layout.padLeft + 4}
              y={yAt(target) - 4}
              textAnchor="start"
              className="altitude-chart-target-label"
            >
              target {fmtNumber(target, 1)} m
            </text>
          </g>
        )}

        {validMarkers.map((m, i) => {
          // Two rules for label placement:
          //  1. Different tones live at different vertical bands so a
          //     STATUSTEXT (warn) and a FAILED (fail) marker close in time
          //     never fight for the same pixels. warn → bottom, fail → top,
          //     everything else → below the previous run.
          //  2. Labels flip anchor when the marker sits near the right edge
          //     so text doesn't clip out of the chart.
          const nearRight = xAt(m.tMs) > layout.width - layout.padRight - 90;
          const anchor = nearRight ? 'end' : 'start';
          const xLabel = xAt(m.tMs) + (nearRight ? -4 : 4);
          const yLabel =
            m.tone === 'fail'
              ? layout.padTop + 10
              : layout.height - layout.padBottom - 6;
          return (
            <g key={`${m.tone}-${i}`} className={`altitude-chart-marker ${m.tone}`}>
              <line
                x1={xAt(m.tMs)}
                x2={xAt(m.tMs)}
                y1={layout.padTop}
                y2={layout.height - layout.padBottom}
              />
              <text
                x={xLabel}
                y={yLabel}
                textAnchor={anchor}
                className="altitude-chart-marker-label"
              >
                {m.label}
              </text>
            </g>
          );
        })}

        <line
          x1={layout.padLeft}
          x2={layout.width - layout.padRight}
          y1={layout.height - layout.padBottom}
          y2={layout.height - layout.padBottom}
          className="altitude-chart-axis"
        />
        <text
          x={layout.padLeft}
          y={layout.height - 6}
          className="altitude-chart-tick"
        >
          +0s
        </text>
        <text
          x={layout.width - layout.padRight}
          y={layout.height - 6}
          textAnchor="end"
          className="altitude-chart-tick"
        >
          +{formatDurationMs(tMax)}
        </text>
      </svg>

      <figcaption className="altitude-chart-legend">
        <span className="legend-item altitude">
          <span className="swatch line" />
          altitude
        </span>
        {showTarget && target != null && (
          <span className="legend-item target">
            <span className="swatch dashed" />
            target {fmtNumber(target, 1)} m
          </span>
        )}
        {target != null && gap != null && gap > 0 && (
          <span className="legend-item gap">
            gap <strong>{fmtNumber(gap, 1)} m</strong>
          </span>
        )}
        <span className="legend-item peak">
          peak <strong>{fmtNumber(peakAlt, 1)} m</strong>
        </span>
        {validMarkers.map((m, i) => (
          <span key={i} className={`legend-item marker ${m.tone}`}>
            <span className={`swatch marker ${m.tone}`} />
            {m.label}
          </span>
        ))}
      </figcaption>
    </figure>
  );
}

function niceTicks(min: number, max: number, count: number): number[] {
  if (!Number.isFinite(min) || !Number.isFinite(max) || max <= min) {
    return [min];
  }
  const rawStep = (max - min) / count;
  const magnitude = Math.pow(10, Math.floor(Math.log10(rawStep)));
  const candidates = [1, 2, 2.5, 5, 10].map((m) => m * magnitude);
  const step = candidates.find((c) => c >= rawStep) ?? candidates[candidates.length - 1]!;
  const start = Math.floor(min / step) * step;
  const ticks: number[] = [];
  for (let v = start; v <= max + step * 0.001; v += step) {
    if (v >= min - step * 0.001) ticks.push(round(v));
  }
  return ticks.length > 0 ? ticks : [min, max];
}

function round(n: number): number {
  return Math.round(n * 1e6) / 1e6;
}
