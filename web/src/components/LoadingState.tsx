interface Props {
  label?: string;
}

export function LoadingState({ label = 'loading…' }: Props) {
  return <div className="loading">{label}</div>;
}
