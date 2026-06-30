// TypeScript mirror of the Axum API DTOs defined in
// crates/flighttrace-api/src/dto.rs. Field names use camelCase and enum
// payloads use SCREAMING_SNAKE_CASE / PascalCase to match what the server
// actually emits.

export type IsoTimestamp = string;

export type MissionSource = 'DEMO' | 'MAVLINK';

export type MissionStatus = 'RUNNING' | 'SUCCEEDED' | 'FAILED';

export type MissionPhase =
  | 'DISCONNECTED'
  | 'CONNECTED'
  | 'ARMING'
  | 'ARMED'
  | 'TAKEOFF_REQUESTED'
  | 'CLIMBING'
  | 'TARGET_ALTITUDE_REACHED'
  | 'NAVIGATING_TO_WAYPOINT'
  | 'RETURNING_HOME'
  | 'LANDING'
  | 'LANDED'
  | 'FAILED';

export type FailureReason =
  | 'NO_HEARTBEAT'
  | 'COMMAND_TIMEOUT'
  | 'TAKEOFF_TIMEOUT'
  | 'WAYPOINT_NOT_REACHED'
  | 'UNEXPECTED_MODE_CHANGE'
  | 'TELEMETRY_GAP'
  | 'LOW_BATTERY'
  | 'GPS_UNHEALTHY'
  | 'UNKNOWN';

export type AssertionOutcome = 'PASS' | 'WARN' | 'FAIL';

export interface PhaseTransitionDto {
  at: IsoTimestamp;
  phase: MissionPhase;
}

export interface MissionRunDto {
  id: string;
  vehicle: string;
  mission: string;
  source: MissionSource;
  status: MissionStatus;
  failureReason: FailureReason | null;
  startedAt: IsoTimestamp;
  endedAt: IsoTimestamp | null;
  phases: PhaseTransitionDto[];
}

export type NormalizedEventKindDto =
  | { kind: 'VehicleConnected' }
  | { kind: 'HeartbeatReceived' }
  | { kind: 'ArmedChanged'; armed: boolean }
  | { kind: 'ModeChanged'; mode: string }
  | { kind: 'CommandAccepted'; command: string }
  | { kind: 'CommandRejected'; command: string; reason: string }
  | { kind: 'PositionUpdated'; lat: number; lon: number }
  | { kind: 'AltitudeUpdated'; altitudeM: number }
  | { kind: 'BatteryUpdated'; remainingPct: number; voltageV: number }
  | { kind: 'WaypointChanged'; index: number }
  | { kind: 'TelemetryGapDetected'; gapSec: number }
  | { kind: 'StatustextReceived'; severity: number; text: string }
  | { kind: 'AssertionPassed'; assertionName: string }
  | { kind: 'AssertionFailed'; assertionName: string; detail: string }
  | { kind: 'MissionFailed'; reason: FailureReason };

export type NormalizedEventDto = {
  id: string;
  runId: string;
  at: IsoTimestamp;
} & NormalizedEventKindDto;

export interface TelemetrySampleDto {
  runId: string;
  at: IsoTimestamp;
  altitudeM: number;
  verticalSpeedMps: number;
  mode: string;
  armed: boolean;
  batteryRemainingPct: number;
}

export interface AssertionResultDto {
  id: string;
  runId: string;
  name: string;
  outcome: AssertionOutcome;
  detail: string;
  evaluatedAt: IsoTimestamp;
}

export type EvidenceDto =
  | { kind: 'Note'; text: string }
  | { kind: 'Event'; eventId: string }
  | { kind: 'Sample'; at: IsoTimestamp; field: string; value: string }
  | { kind: 'Statustext'; at: IsoTimestamp; text: string };

export interface FailureReportDto {
  id: string;
  runId: string;
  reason: FailureReason;
  summary: string;
  evidence: EvidenceDto[];
  likelyCause: string;
  generatedAt: IsoTimestamp;
}

export interface ApiErrorBody {
  error: {
    code: string;
    message: string;
  };
}
