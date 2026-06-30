import { ApiError } from '../api/client';

interface Props {
  error: unknown;
  context?: string;
}

export function ErrorState({ error, context }: Props) {
  const message = formatError(error);
  return (
    <div className="error">
      <strong>{context ?? 'request failed'}:</strong> {message}
    </div>
  );
}

function formatError(error: unknown): string {
  if (error instanceof ApiError) {
    return `[${error.status} ${error.code}] ${error.message}`;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}
