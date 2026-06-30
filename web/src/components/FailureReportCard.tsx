import type { FailureReportDto } from '../types/api';
import { EvidenceList } from './EvidenceList';

interface Props {
  report: FailureReportDto;
}

export function FailureReportCard({ report }: Props) {
  return (
    <section className="section failure-card">
      <header className="section-header">
        <h2>Failure report</h2>
        <span className="badge fail">{report.reason}</span>
      </header>
      <div className="section-body">
        <p className="summary">{report.summary}</p>
        <EvidenceList evidence={report.evidence} />
        <div className="likely-cause">{report.likelyCause}</div>
      </div>
    </section>
  );
}
