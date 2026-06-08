interface ProcessStatusDotsProps {
  testId: string;
}

export function ProcessStatusDots({ testId }: ProcessStatusDotsProps) {
  return (
    <span data-testid={testId} className="forge-status-dots">
      <span className="forge-status-dot" />
      <span className="forge-status-dot" style={{ animationDelay: "0.18s" }} />
      <span className="forge-status-dot" style={{ animationDelay: "0.36s" }} />
    </span>
  );
}
