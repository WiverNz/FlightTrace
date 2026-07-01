import type {
  AssertionResultDto,
  EvidenceDto,
  FailureReportDto,
  MissionRunDto,
  NormalizedEventDto,
  TelemetrySampleDto,
} from '../types/api';
import { formatDurationMs, formatRelativeTime } from '../format';
import { AltitudeChart, type AltitudeChartMarker } from './AltitudeChart';

interface Props {
  report: FailureReportDto;
  run: MissionRunDto | null;
  assertions: AssertionResultDto[] | null;
  telemetry: TelemetrySampleDto[] | null;
  events: NormalizedEventDto[] | null;
}

interface AltitudeResult {
  actual: number;
  target: number;
}

interface StatustextHit {
  text: string;
  at: string;
}

export function FailureSummary({
  report,
  run,
  assertions,
  telemetry,
  events,
}: Props) {
  const altitudeAssertion = assertions?.find(
    (assertion) =>
      assertion.outcome === 'FAIL' &&
      assertion.name.endsWith('.altitude_at_least_m'),
  );
  const parsedAltitude = parseAltitudeResult(altitudeAssertion?.detail);
  const actualAltitude =
    parsedAltitude?.actual ?? maximumAltitude(telemetry ?? []);
  const targetAltitude = parsedAltitude?.target;
  const altitudeGap =
    targetAltitude != null && actualAltitude != null
      ? Math.max(0, targetAltitude - actualAltitude)
      : null;
  // Two distinct concepts, previously conflated in one "Failure time" tile:
  //  - "Failed at": when the mission actually terminated (last phase, run.endedAt).
  //  - "Timeout budget": how long the failing check waited before firing, i.e.
  //    the step-window end minus the run start. The altitude check sets
  //    evaluatedAt = window.end, so this is a straightforward subtraction.
  const failedAtIso = run?.endedAt ?? null;
  const failedAtRelative =
    run && failedAtIso ? formatRelativeTime(failedAtIso, run.startedAt) : null;

  const budgetIso = altitudeAssertion?.evaluatedAt ?? null;
  const budgetMs =
    run && budgetIso ? Date.parse(budgetIso) - Date.parse(run.startedAt) : null;

  // Failure marker on the chart uses the actual mission end when we have
  // one, otherwise falls back to the assertion's evaluation timestamp so
  // there's always *something* pinned to the timeline.
  const markerAt = failedAtIso ?? budgetIso;
  const statustextHit = findLastStatustext(events ?? [], report);

  const takeoffExplanation =
    report.reason === 'TAKEOFF_TIMEOUT' &&
    targetAltitude != null &&
    actualAltitude != null
      ? `TAKEOFF_TIMEOUT happened because target altitude was ${targetAltitude.toFixed(1)} m, but the vehicle only reached ${actualAltitude.toFixed(1)} m and stopped climbing${statustextHit ? ` after ${statustextHit.text}` : ''}.`
      : report.summary;

  const markers: AltitudeChartMarker[] = [];
  if (statustextHit) {
    markers.push({
      at: statustextHit.at,
      label: 'STATUSTEXT',
      tone: 'warn',
    });
  }
  if (markerAt) {
    markers.push({
      at: markerAt,
      label: 'FAILED',
      tone: 'fail',
    });
  }

  const showChart =
    telemetry != null && telemetry.length > 0 && run != null;

  return (
    <section className="section failure-summary">
      <header className="section-header">
        <h2>Failure summary</h2>
        <span className="badge fail">{report.reason}</span>
      </header>
      <div className="section-body">
        <p className="failure-summary-explanation">{takeoffExplanation}</p>
        <dl className="failure-summary-facts">
          <Fact
            label="Target altitude"
            value={formatMetres(targetAltitude)}
            tone="expected"
          />
          <Fact
            label="Max reached"
            value={formatMetres(actualAltitude)}
            tone="actual"
          />
          <Fact
            label="Altitude gap"
            value={altitudeGap != null ? `${altitudeGap.toFixed(1)} m` : '—'}
            tone={altitudeGap != null && altitudeGap > 0 ? 'actual' : undefined}
          />
          <Fact
            label="Failed at"
            value={failedAtRelative ?? '—'}
            tone="actual"
          />
          <Fact
            label="Timeout budget"
            value={
              budgetMs != null && budgetMs >= 0
                ? formatDurationMs(budgetMs)
                : '—'
            }
          />
          <Fact
            label="Last STATUSTEXT"
            value={statustextHit?.text ?? '—'}
          />
        </dl>

        {showChart && (
          <div className="failure-summary-chart">
            <AltitudeChart
              samples={telemetry!}
              startedAt={run!.startedAt}
              targetAltitudeM={targetAltitude ?? null}
              markers={markers}
              variant="compact"
            />
          </div>
        )}
      </div>
    </section>
  );
}

function Fact({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone?: 'expected' | 'actual';
}) {
  return (
    <div className={tone ? `failure-fact ${tone}` : 'failure-fact'}>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function parseAltitudeResult(detail: string | undefined): AltitudeResult | null {
  if (!detail) return null;
  const match = detail.match(
    /max_altitude=([-+]?\d+(?:\.\d+)?)m\s+target=([-+]?\d+(?:\.\d+)?)m/i,
  );
  if (!match) return null;
  const actual = Number(match[1]);
  const target = Number(match[2]);
  return Number.isFinite(actual) && Number.isFinite(target)
    ? { actual, target }
    : null;
}

function maximumAltitude(samples: TelemetrySampleDto[]): number | null {
  if (samples.length === 0) return null;
  const maximum = Math.max(...samples.map((sample) => sample.altitudeM));
  return Number.isFinite(maximum) ? maximum : null;
}

function findLastStatustext(
  events: NormalizedEventDto[],
  report: FailureReportDto,
): StatustextHit | null {
  const event = [...events]
    .filter((item) => item.kind === 'StatustextReceived')
    .sort((a, b) => Date.parse(b.at) - Date.parse(a.at))[0];
  if (event?.kind === 'StatustextReceived') {
    return { text: event.text, at: event.at };
  }

  const evidence = [...report.evidence]
    .filter((item): item is Extract<EvidenceDto, { kind: 'Statustext' }> =>
      item.kind === 'Statustext',
    )
    .sort((a, b) => Date.parse(b.at) - Date.parse(a.at))[0];
  return evidence ? { text: evidence.text, at: evidence.at } : null;
}

function formatMetres(value: number | null | undefined): string {
  return value == null ? '—' : `${value.toFixed(1)} m`;
}
