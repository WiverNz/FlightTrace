import type {
  AssertionResultDto,
  FailureReportDto,
  MissionRunDto,
  NormalizedEventDto,
  TelemetrySampleDto,
} from '../types/api';
import { formatDurationMs } from '../format';

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
  const timeoutMs =
    run && altitudeAssertion
      ? Date.parse(altitudeAssertion.evaluatedAt) - Date.parse(run.startedAt)
      : null;
  const lastStatustext = findLastStatustext(events ?? [], report);

  const takeoffExplanation =
    report.reason === 'TAKEOFF_TIMEOUT' &&
    targetAltitude != null &&
    actualAltitude != null
      ? `TAKEOFF_TIMEOUT happened because target altitude was ${targetAltitude.toFixed(1)} m, but the vehicle only reached ${actualAltitude.toFixed(1)} m and then stopped climbing${lastStatustext ? ` after “${lastStatustext}”` : ''}.`
      : report.summary;

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
            label="Timeout window"
            value={
              timeoutMs != null && timeoutMs >= 0
                ? formatDurationMs(timeoutMs)
                : '—'
            }
          />
          <Fact label="Last STATUSTEXT" value={lastStatustext ?? '—'} />
        </dl>
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
): string | null {
  const event = [...events]
    .filter((item) => item.kind === 'StatustextReceived')
    .sort((a, b) => Date.parse(b.at) - Date.parse(a.at))[0];
  if (event?.kind === 'StatustextReceived') return event.text;

  const evidence = [...report.evidence]
    .filter((item) => item.kind === 'Statustext')
    .sort((a, b) => Date.parse(b.at) - Date.parse(a.at))[0];
  return evidence?.kind === 'Statustext' ? evidence.text : null;
}

function formatMetres(value: number | null | undefined): string {
  return value == null ? '—' : `${value.toFixed(1)} m`;
}
