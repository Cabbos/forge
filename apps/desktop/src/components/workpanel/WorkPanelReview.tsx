import { useEffect, useMemo, useState } from "react";
import ReactDiffViewer from "react-diff-viewer-continued";
import { FileDiff, RefreshCw } from "lucide-react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { ForgeButton } from "@/components/primitives/button";
import { ForgeIconButton } from "@/components/primitives/icon-button";
import { useWorkspaceReviewQuery } from "@/hooks/queries/useWorkspaceReviewQuery";
import type { WorkspaceReview } from "@/lib/tauri";
import { useActiveWorkspace, useStore } from "@/store";

interface ReviewFeedbackTarget {
  path: string;
  line: number;
}

export function WorkPanelReview() {
  const activeSessionId = useStore((state) => state.activeSessionId);
  const activeSession = useStore((state) => activeSessionId ? state.sessions.get(activeSessionId) ?? null : null);
  const activeWorkspace = useActiveWorkspace();
  const setPendingInput = useStore((state) => state.setPendingInput);
  const workingDir = activeSession?.workingDir ?? activeWorkspace?.path ?? null;
  const reviewQuery = useWorkspaceReviewQuery(activeSessionId, workingDir);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [feedbackTarget, setFeedbackTarget] = useState<ReviewFeedbackTarget | null>(null);
  const [feedback, setFeedback] = useState("");

  useEffect(() => {
    const paths = reviewQuery.data?.files.map((file) => file.path) ?? [];
    setSelectedPath((current) => current && paths.includes(current) ? current : paths[0] ?? null);
  }, [reviewQuery.data]);

  const values = useMemo(
    () => diffValuesForFile(reviewQuery.data ?? null, selectedPath),
    [reviewQuery.data, selectedPath],
  );

  if (!activeSessionId && !workingDir) {
    return <ReviewMessage title="还没有可审阅的项目" detail="选择项目或打开对话后再查看当前改动。" />;
  }
  if (reviewQuery.isPending) {
    return <ReviewMessage title="正在读取当前改动" detail="只显示工作区的最新结果。" />;
  }
  if (reviewQuery.isError) {
    return <ReviewMessage title="无法读取当前改动" detail={String(reviewQuery.error)} />;
  }
  if (!reviewQuery.data || reviewQuery.data.files.length === 0) {
    return <ReviewMessage title="当前没有改动" detail="工作区与最近一次提交一致。" onRefresh={() => reviewQuery.refetch()} />;
  }

  const submitFeedback = (event: React.FormEvent) => {
    event.preventDefault();
    const message = feedback.trim();
    if (!feedbackTarget || !message) return;
    setPendingInput(`审阅反馈（${feedbackTarget.path}:${feedbackTarget.line}）：\n${message}`);
    setFeedback("");
    setFeedbackTarget(null);
  };

  return (
    <section className="forge-work-panel-review" data-testid="work-panel-review" aria-label="当前改动审阅">
      <header className="forge-work-panel-content-toolbar">
        <div className="forge-work-panel-content-title">
          <FileDiff className="size-4" />
          <span>当前改动</span>
          <small>{reviewQuery.data.files.length} 个文件</small>
        </div>
        <ForgeIconButton aria-label="刷新当前改动" title="刷新当前改动" onClick={() => reviewQuery.refetch()}>
          <RefreshCw className="size-3.5" />
        </ForgeIconButton>
      </header>
      {reviewQuery.data.truncated ? (
        <div className="forge-work-panel-review-notice" role="status">改动较大，当前只显示前 2 MiB。</div>
      ) : null}
      <div className="forge-work-panel-review-body">
        <nav className="forge-work-panel-review-files" aria-label="改动文件">
          {reviewQuery.data.files.map((file) => (
            <ButtonPrimitive
              key={file.path}
              type="button"
              data-active={selectedPath === file.path ? "true" : "false"}
              onClick={() => {
                setSelectedPath(file.path);
                setFeedbackTarget(null);
              }}
            >
              <span>{file.path}</span>
              <small>+{file.additions} / -{file.deletions}</small>
            </ButtonPrimitive>
          ))}
        </nav>
        <div className="forge-work-panel-review-diff">
          <ReactDiffViewer
            oldValue={values.oldValue}
            newValue={values.newValue}
            splitView={false}
            showDiffOnly={false}
            disableWorker
            hideLineNumbers
            leftTitle={selectedPath ?? undefined}
            renderGutter={({ additionalLineNumber, lineNumber }) => {
              const line = additionalLineNumber ?? lineNumber;
              return (
                <td className="forge-work-panel-review-gutter">
                  {selectedPath && line > 0 ? (
                    <button
                      type="button"
                      aria-label={`${selectedPath} 第 ${line} 行`}
                      onClick={() => {
                        setFeedbackTarget({ path: selectedPath, line });
                        setFeedback("");
                      }}
                    >
                      {line}
                    </button>
                  ) : null}
                </td>
              );
            }}
          />
        </div>
      </div>
      {feedbackTarget ? (
        <form className="forge-work-panel-review-feedback" onSubmit={submitFeedback}>
          <div>
            <strong>{feedbackTarget.path}:{feedbackTarget.line}</strong>
            <ButtonPrimitive type="button" onClick={() => setFeedbackTarget(null)}>取消</ButtonPrimitive>
          </div>
          <textarea
            autoFocus
            value={feedback}
            onChange={(event) => setFeedback(event.target.value)}
            placeholder="写下这一行需要调整的地方"
            rows={3}
          />
          <ForgeButton type="submit" size="sm" disabled={!feedback.trim()}>发送到对话</ForgeButton>
        </form>
      ) : null}
    </section>
  );
}

function ReviewMessage({
  detail,
  onRefresh,
  title,
}: {
  detail: string;
  onRefresh?: () => void;
  title: string;
}) {
  return (
    <div className="forge-work-panel-placeholder">
      <FileDiff className="size-5" />
      <strong>{title}</strong>
      <span>{detail}</span>
      {onRefresh ? <ForgeButton variant="outline" size="sm" onClick={onRefresh}>重新检查</ForgeButton> : null}
    </div>
  );
}

function diffValuesForFile(review: WorkspaceReview | null, path: string | null) {
  if (!review || !path) return { oldValue: "", newValue: "" };
  const marker = `diff --git a/${path} b/${path}`;
  const start = review.patch.indexOf(marker);
  if (start < 0) return { oldValue: "", newValue: "" };
  const next = review.patch.indexOf("\ndiff --git ", start + marker.length);
  const segment = review.patch.slice(start, next < 0 ? undefined : next);
  const oldLines: string[] = [];
  const newLines: string[] = [];

  for (const line of segment.split("\n")) {
    if (line.startsWith("@@") || line.startsWith("diff --git") || line.startsWith("index ")
      || line.startsWith("---") || line.startsWith("+++") || line.startsWith("\\ No newline")) {
      continue;
    }
    if (line.startsWith("-")) oldLines.push(line.slice(1));
    else if (line.startsWith("+")) newLines.push(line.slice(1));
    else if (line.startsWith(" ")) {
      oldLines.push(line.slice(1));
      newLines.push(line.slice(1));
    }
  }
  return { oldValue: oldLines.join("\n"), newValue: newLines.join("\n") };
}
