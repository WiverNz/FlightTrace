import type { TelemetrySampleDto } from '../types/api';
import { fmtNumber, formatTimeOfDay } from '../format';
import { AltitudeChart, type AltitudeChartMarker } from './AltitudeChart';
import { VerticalSpeedSummary } from './VerticalSpeedSummary';

interface Props {
  samples: TelemetrySampleDto[];
  startedAt: string | null;
  targetAltitudeM?: number | null;
  markers?: AltitudeChartMarker[];
}

export function TelemetryOverview({
  samples,
  startedAt,
  targetAltitudeM,
  markers,
}: Props) {
  if (samples.length === 0) {
    return <p className="muted">No telemetry samples for this run.</p>;
  }

  const altitudes = samples.map((s) => s.altitudeM);
  const max = Math.max(...altitudes);
  const last = samples[samples.length - 1]!;
  const first = samples[0]!;

  return (
    <div className="telemetry-overview">
      <div className="telemetry-charts">
        {startedAt ? (
          <AltitudeChart
            samples={samples}
            startedAt={startedAt}
            targetAltitudeM={targetAltitudeM}
            markers={markers}
            variant="full"
          />
        ) : (
          <p className="muted">Run start time unavailable.</p>
        )}

        <div className="telemetry-side">
          <VerticalSpeedSummary samples={samples} />
          <dl className="telemetry-side-stats">
            <div>
              <dt>max altitude</dt>
              <dd>{fmtNumber(max)} m</dd>
            </div>
            <div>
              <dt>last altitude</dt>
              <dd>{fmtNumber(last.altitudeM)} m</dd>
            </div>
            <div>
              <dt>samples</dt>
              <dd>{samples.length}</dd>
            </div>
            <div>
              <dt>window</dt>
              <dd className="mono">
                {formatTimeOfDay(first.at)} → {formatTimeOfDay(last.at)}
              </dd>
            </div>
          </dl>
        </div>
      </div>

      <div className="telemetry-table-wrap">
        <table className="data">
          <thead>
            <tr>
              <th>Time</th>
              <th>Alt (m)</th>
              <th>V-speed (m/s)</th>
              <th>Mode</th>
              <th>Armed</th>
              <th>Battery %</th>
            </tr>
          </thead>
          <tbody>
            {samples.map((s, i) => (
              <tr key={i}>
                <td className="mono">{formatTimeOfDay(s.at)}</td>
                <td className="mono">{fmtNumber(s.altitudeM)}</td>
                <td className="mono">{fmtNumber(s.verticalSpeedMps)}</td>
                <td className="mono">{s.mode}</td>
                <td className="mono">{s.armed ? 'yes' : 'no'}</td>
                <td className="mono">{fmtNumber(s.batteryRemainingPct, 1)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
