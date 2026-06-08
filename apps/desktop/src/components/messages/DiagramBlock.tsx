import { Network } from "lucide-react";
import { deriveDiagramView } from "@/components/messages/diagramPresentation";
import { ReaderCaptionAction } from "@/components/messages/ReaderCaptionAction";

interface DiagramBlockProps {
  code: string;
  lang: string;
}

export function DiagramBlock({ code, lang }: DiagramBlockProps) {
  const view = deriveDiagramView(code, lang);

  return (
    <figure data-testid="diagram-surface" data-diagram-kind={view.kind} className="diagram-surface">
      <figcaption className="diagram-caption">
        <div className="diagram-caption-title">
          <Network className="size-3.5" />
          <span>{view.title}</span>
          <span className="diagram-caption-meta">{view.meta}</span>
        </div>
        <ReaderCaptionAction text={code} idleLabel="复制图示源码" />
      </figcaption>
      <div data-testid="diagram-viewport" className="diagram-viewport">
        <pre className="diagram-code">
          <code>{code}</code>
        </pre>
      </div>
    </figure>
  );
}
