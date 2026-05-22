interface ShellOutputSection {
  label: string;
  content: string;
}

export function ShellOutputSections({ sections }: { sections: ShellOutputSection[] }) {
  return (
    <div data-testid="log-detail-output" className="forge-log-output">
      {sections.map((section, index) => (
        <div
          key={`${section.label}-${index}`}
          data-testid="shell-output-section"
          className="forge-shell-output-section"
          data-tone={section.label === "stderr" ? "error" : "default"}
        >
          <div className="forge-shell-output-label">{section.label}</div>
          <pre>{section.content || " "}</pre>
        </div>
      ))}
    </div>
  );
}
