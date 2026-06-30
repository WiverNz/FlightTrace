import { request } from './client';
import type {
  AssertionResultDto,
  FailureReportDto,
  MissionRunDto,
  NormalizedEventDto,
  TelemetrySampleDto,
} from '../types/api';

export function listRuns(signal?: AbortSignal): Promise<MissionRunDto[]> {
  return request<MissionRunDto[]>('/api/runs', { signal });
}

export function getRun(id: string, signal?: AbortSignal): Promise<MissionRunDto> {
  return request<MissionRunDto>(`/api/runs/${id}`, { signal });
}

export function getEvents(
  id: string,
  signal?: AbortSignal,
): Promise<NormalizedEventDto[]> {
  return request<NormalizedEventDto[]>(`/api/runs/${id}/events`, { signal });
}

export function getAssertions(
  id: string,
  signal?: AbortSignal,
): Promise<AssertionResultDto[]> {
  return request<AssertionResultDto[]>(`/api/runs/${id}/assertions`, { signal });
}

export function getTelemetry(
  id: string,
  signal?: AbortSignal,
): Promise<TelemetrySampleDto[]> {
  return request<TelemetrySampleDto[]>(`/api/runs/${id}/telemetry`, { signal });
}

export function getFailureReport(
  id: string,
  signal?: AbortSignal,
): Promise<FailureReportDto> {
  return request<FailureReportDto>(`/api/runs/${id}/failure-report`, { signal });
}

export function createDemoRun(): Promise<{ id: string }> {
  return request<{ id: string }>('/api/demo/run', { method: 'POST' });
}
