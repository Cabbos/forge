import shutil
import subprocess
from pathlib import Path

from app.models import AgentTrace, LeakageCheck, WorkspaceCheck

FUTURE_STATE_LOOKUP_COMMANDS = (
    "git log --all",
    "git reflog",
    "git branch -a",
    "git remote -v",
)
SOLUTION_NOTE_MARKERS = ("solution", "answer", "spoiler", "future-fix", "fix-notes")


def assert_clean_workspace(workspace: Path, *, allowed_untracked: list[str]) -> WorkspaceCheck:
    allowed = set(allowed_untracked)
    completed = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=workspace,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        untracked = [
            str(path.relative_to(workspace))
            for path in workspace.rglob("*")
            if path.is_file() and str(path.relative_to(workspace)) not in allowed
        ]
        return WorkspaceCheck(
            ok=not untracked,
            untracked_files=untracked,
            message="Workspace is not a git repository; checked filesystem files.",
        )

    untracked_files: list[str] = []
    modified_files: list[str] = []
    for line in completed.stdout.splitlines():
        status = line[:2]
        path = line[3:]
        if path in allowed:
            continue
        if status == "??":
            untracked_files.append(path)
        else:
            modified_files.append(path)
    return WorkspaceCheck(
        ok=not untracked_files and not modified_files,
        untracked_files=untracked_files,
        modified_files=modified_files,
    )


def scrub_future_repo_state(workspace: Path) -> LeakageCheck:
    scrubbed_items: list[str] = []
    if (workspace / ".git").exists():
        scrubbed_items.extend(remove_git_remotes(workspace))
        scrubbed_items.extend(remove_extra_git_branches(workspace))
        scrubbed_items.extend(remove_git_tags(workspace))
        scrubbed_items.extend(unset_upstream(workspace))
        scrubbed_items.extend(remove_git_metadata_cache(workspace))
    scrubbed_items.extend(remove_solution_notes(workspace))

    findings = workspace_future_state_findings(workspace)
    return LeakageCheck(
        ok=not findings,
        findings=findings,
        scrubbed_items=scrubbed_items,
    )


def detect_future_state_lookup(trace: AgentTrace) -> LeakageCheck:
    findings: list[str] = []
    for output in [*trace.tool_calls, *trace.shell_outputs]:
        command = normalized_command(output.command)
        future_lookup = any(pattern in command for pattern in FUTURE_STATE_LOOKUP_COMMANDS)
        if future_lookup or command.startswith("git show "):
            findings.append(output.command)
    return LeakageCheck(ok=not findings, findings=findings)


def remove_git_remotes(workspace: Path) -> list[str]:
    scrubbed: list[str] = []
    for remote in git_lines(workspace, "remote"):
        run_git(workspace, "remote", "remove", remote)
        scrubbed.append(f"remote:{remote}")
    remote_refs = workspace / ".git" / "refs" / "remotes"
    if remote_refs.exists():
        shutil.rmtree(remote_refs)
        scrubbed.append("metadata:refs/remotes")
    return scrubbed


def remove_extra_git_branches(workspace: Path) -> list[str]:
    scrubbed: list[str] = []
    current = git_stdout(workspace, "branch", "--show-current").strip()
    for branch in git_lines(workspace, "for-each-ref", "--format=%(refname:short)", "refs/heads"):
        if branch == current:
            continue
        run_git(workspace, "branch", "-D", branch)
        scrubbed.append(f"branch:{branch}")
    return scrubbed


def remove_git_tags(workspace: Path) -> list[str]:
    scrubbed: list[str] = []
    for tag in git_lines(workspace, "tag", "--list"):
        run_git(workspace, "tag", "-d", tag)
        scrubbed.append(f"tag:{tag}")
    return scrubbed


def unset_upstream(workspace: Path) -> list[str]:
    completed = run_git(workspace, "branch", "--unset-upstream")
    return ["metadata:upstream"] if completed.returncode == 0 else []


def remove_git_metadata_cache(workspace: Path) -> list[str]:
    scrubbed: list[str] = []
    git_dir = workspace / ".git"
    for relative in ["logs", "FETCH_HEAD", "ORIG_HEAD", "MERGE_HEAD"]:
        path = git_dir / relative
        if path.is_dir():
            shutil.rmtree(path)
            scrubbed.append(f"metadata:{relative}")
        elif path.exists():
            path.unlink()
            scrubbed.append(f"metadata:{relative}")
    return scrubbed


def remove_solution_notes(workspace: Path) -> list[str]:
    scrubbed: list[str] = []
    for path in workspace.rglob("*"):
        relative = path.relative_to(workspace)
        if ".git" in relative.parts or not path.is_file():
            continue
        name = path.name.casefold()
        if any(marker in name for marker in SOLUTION_NOTE_MARKERS):
            path.unlink()
            scrubbed.append(f"file:{relative}")
    return scrubbed


def workspace_future_state_findings(workspace: Path) -> list[str]:
    findings: list[str] = []
    if not (workspace / ".git").exists():
        return findings
    current = git_stdout(workspace, "branch", "--show-current").strip()
    for branch in git_lines(workspace, "for-each-ref", "--format=%(refname:short)", "refs/heads"):
        if branch != current:
            findings.append(f"branch:{branch}")
    findings.extend(f"remote:{remote}" for remote in git_lines(workspace, "remote"))
    findings.extend(f"tag:{tag}" for tag in git_lines(workspace, "tag", "--list"))
    if (workspace / ".git" / "logs").exists():
        findings.append("metadata:logs")
    return findings


def normalized_command(command: str) -> str:
    return " ".join(command.casefold().split())


def git_stdout(workspace: Path, *args: str) -> str:
    return run_git(workspace, *args).stdout


def git_lines(workspace: Path, *args: str) -> list[str]:
    return [line for line in git_stdout(workspace, *args).splitlines() if line]


def run_git(workspace: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=workspace,
        text=True,
        capture_output=True,
        check=False,
    )
