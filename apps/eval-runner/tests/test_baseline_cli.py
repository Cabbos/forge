import json
import os
import shlex
import subprocess
import sys
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
EVAL_ROOT = Path(__file__).resolve().parents[1]
FORGEBENCH_ROOT = EVAL_ROOT / "forgebench"
DEVELOPMENT_CASES = FORGEBENCH_ROOT / "cases" / "development"
HOLDOUT_CASES = FORGEBENCH_ROOT / "cases" / "holdout"
PROFILE_PATH = FORGEBENCH_ROOT / "profiles" / "deepseek-v4-flash-anthropic-baseline-v1.json"
GOLDEN_FIXTURE = FORGEBENCH_ROOT / "forgebench-monorepo"


def run_baseline_cli(
    *arguments: str,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    child_env = os.environ.copy()
    child_env.pop("FORGE_EVAL_FORGE_AGENT_COMMAND", None)
    if env is not None:
        child_env.update(env)
    return subprocess.run(
        [sys.executable, "-m", "app.cli", "baseline", *arguments],
        cwd=EVAL_ROOT,
        env=child_env,
        text=True,
        capture_output=True,
        check=False,
    )


@pytest.mark.parametrize(
    ("command", "extra_arguments"),
    [
        ("init", ["--campaign-id", "h1-cli-test"]),
        ("run", ["--max-attempts", "1"]),
        ("status", []),
        ("recover", []),
        ("report", []),
        ("verify", []),
    ],
)
def test_baseline_commands_require_explicit_campaign_and_profile(
    command: str,
    extra_arguments: list[str],
) -> None:
    missing_both = run_baseline_cli(command, *extra_arguments)
    missing_profile = run_baseline_cli(
        command,
        "--campaign",
        "explicit-campaign",
        *extra_arguments,
    )
    missing_campaign = run_baseline_cli(
        command,
        "--profile",
        str(PROFILE_PATH),
        *extra_arguments,
    )

    assert missing_both.returncode == 2
    assert missing_profile.returncode == 2
    assert missing_campaign.returncode == 2


@pytest.mark.parametrize(
    "arguments",
    [
        [],
        ["--control-campaign", "control"],
        ["--control-campaign", "control", "--control-profile", "control.json"],
        [
            "--control-campaign",
            "control",
            "--control-profile",
            "control.json",
            "--candidate-campaign",
            "candidate",
            "--candidate-profile",
            "candidate.json",
        ],
    ],
)
def test_baseline_compare_requires_explicit_pair_and_output(arguments: list[str]) -> None:
    completed = run_baseline_cli("compare", *arguments)

    assert completed.returncode == 2
    assert completed.stdout == ""


def test_baseline_init_rejects_holdout_without_leaking_inputs(tmp_path: Path) -> None:
    campaign = tmp_path / "should-not-exist"
    secret = "sk-this-must-not-appear-123456789"

    completed = run_baseline_cli(
        "init",
        "--campaign",
        str(campaign),
        "--campaign-id",
        "h1-holdout-rejected",
        "--profile",
        str(PROFILE_PATH),
        "--cases",
        str(HOLDOUT_CASES),
        env={"DEEPSEEK_API_KEY": secret},
    )

    assert completed.returncode == 1
    assert completed.stderr == "error: baseline_init_failed\n"
    assert completed.stdout == ""
    assert not campaign.exists()
    combined = completed.stdout + completed.stderr
    assert str(tmp_path) not in combined
    assert secret not in combined
    assert "holdout" not in combined


def test_baseline_run_requires_configured_forge_command_before_reservation(
    tmp_path: Path,
) -> None:
    campaign = tmp_path / "baseline"
    initialized = run_baseline_cli(
        "init",
        "--campaign",
        str(campaign),
        "--campaign-id",
        "h1-command-required",
        "--profile",
        str(PROFILE_PATH),
    )
    assert initialized.returncode == 0
    budget_before = (campaign / "budget.jsonl").read_bytes()

    completed = run_baseline_cli(
        "run",
        "--campaign",
        str(campaign),
        "--profile",
        str(PROFILE_PATH),
        "--max-attempts",
        "1",
    )

    assert completed.returncode == 1
    assert completed.stderr == "error: baseline_run_command_missing\n"
    assert completed.stdout == ""
    assert (campaign / "budget.jsonl").read_bytes() == budget_before == b""
    assert list((campaign / "pending").iterdir()) == []
    assert list((campaign / "attempts").iterdir()) == []


def test_baseline_run_failed_preflight_does_not_reserve_budget(
    tmp_path: Path,
) -> None:
    campaign = tmp_path / "baseline"
    fake_agent = tmp_path / "preflight_failure.py"
    fake_agent.write_text(
        """
import json
import sys

payload = json.load(sys.stdin)
assert payload["mode"] == "credential_preflight"
json.dump(
    {
        "credential_preflight": {
            "ok": False,
            "provider": "deepseek",
            "model": "deepseek-v4-flash",
            "provider_call_started": False,
            "error": "missing_api_key",
        }
    },
    sys.stdout,
)
""".strip()
        + "\n",
        encoding="utf-8",
    )
    env = {"FORGE_EVAL_FORGE_AGENT_COMMAND": shlex.join([sys.executable, str(fake_agent)])}
    initialized = run_baseline_cli(
        "init",
        "--campaign",
        str(campaign),
        "--campaign-id",
        "h1-preflight-failed",
        "--profile",
        str(PROFILE_PATH),
        env=env,
    )
    assert initialized.returncode == 0
    campaign_before = (campaign / "campaign.json").read_bytes()

    completed = run_baseline_cli(
        "run",
        "--campaign",
        str(campaign),
        "--profile",
        str(PROFILE_PATH),
        "--max-attempts",
        "1",
        env=env,
    )

    assert completed.returncode == 1
    assert completed.stderr == "error: baseline_run_preflight_failed\n"
    assert completed.stdout == ""
    assert (campaign / "campaign.json").read_bytes() == campaign_before
    assert (campaign / "budget.jsonl").read_bytes() == b""
    assert list((campaign / "pending").iterdir()) == []
    assert list((campaign / "attempts").iterdir()) == []


def test_baseline_run_preflight_spawn_failure_keeps_campaign_immutable(
    tmp_path: Path,
) -> None:
    campaign = tmp_path / "baseline"
    env = {"FORGE_EVAL_FORGE_AGENT_COMMAND": str(tmp_path / "missing-forge-command")}
    initialized = run_baseline_cli(
        "init",
        "--campaign",
        str(campaign),
        "--campaign-id",
        "h1-preflight-spawn-failed",
        "--profile",
        str(PROFILE_PATH),
        env=env,
    )
    assert initialized.returncode == 0
    campaign_before = (campaign / "campaign.json").read_bytes()

    completed = run_baseline_cli(
        "run",
        "--campaign",
        str(campaign),
        "--profile",
        str(PROFILE_PATH),
        "--max-attempts",
        "1",
        env=env,
    )

    assert completed.returncode == 1
    assert completed.stderr == "error: baseline_run_preflight_failed\n"
    assert completed.stdout == ""
    assert (campaign / "campaign.json").read_bytes() == campaign_before
    assert (campaign / "budget.jsonl").read_bytes() == b""
    assert list((campaign / "pending").iterdir()) == []
    assert list((campaign / "attempts").iterdir()) == []


def test_baseline_cli_runs_one_offline_attempt_and_keeps_reads_immutable(
    tmp_path: Path,
) -> None:
    campaign = tmp_path / "baseline"
    fake_agent = tmp_path / "fake_forge_agent.py"
    fake_agent.write_text(
        """
import difflib
import json
import pathlib
import sys

payload = json.loads(sys.stdin.read())
if payload.get("mode") == "credential_preflight":
    assert payload["use_provider_credential_store"] is False
    assert payload["provider_route"] == {
        "transport": "anthropic_messages",
        "base_url": "https://api.deepseek.com/anthropic",
    }
    json.dump(
        {
                "credential_preflight": {
                    "ok": True,
                    "provider": "deepseek",
                    "model": "deepseek-v4-flash",
                    "transport": "anthropic_messages",
                    "endpoint_family": "anthropic",
                    "base_url": "https://api.deepseek.com/anthropic",
                    "provider_call_started": False,
                    "error": None,
                }
        },
        sys.stdout,
    )
    raise SystemExit(0)

workspace = pathlib.Path(payload["workspace_path"])
golden = pathlib.Path(sys.argv[1])
paths = [
    "apps/admin-console/src/bulk-actions.ts",
    "packages/domain/src/task.ts",
]
diffs = []
for relative in paths:
    target = workspace / relative
    before = target.read_text(encoding="utf-8")
    if relative.endswith("bulk-actions.ts"):
        after = (golden / relative).read_text(encoding="utf-8")
    else:
        after = before + "\\n// Forge eval proof: domain contract reviewed.\\n"
    target.write_text(after, encoding="utf-8")
    unified = "".join(
        difflib.unified_diff(
            before.splitlines(keepends=True),
            after.splitlines(keepends=True),
            fromfile=f"a/{relative}",
            tofile=f"b/{relative}",
        )
    )
    diffs.append(
        {
            "path": relative,
            "change_type": "modified",
            "diff": f"diff --git a/{relative} b/{relative}\\n{unified}",
        }
    )

execution_identity = dict(payload["execution_identity"])
execution_identity.update(
    {
        "event_type": "eval_execution_identity",
        "schema_version": 1,
        "provider": payload["provider"],
        "model": payload["model"],
        "transport": payload["provider_route"]["transport"],
        "endpoint_family": "anthropic",
        "base_url": payload["provider_route"]["base_url"],
    }
)
json.dump(
    {
        "changed_files": paths,
        "file_diffs": diffs,
        "final_answer": "Done.",
        "forge_agent_provider": "deepseek",
        "forge_agent_model": "deepseek-v4-flash",
        "forge_agent_transport": "anthropic_messages",
        "forge_agent_endpoint_family": "anthropic",
        "forge_agent_base_url": "https://api.deepseek.com/anthropic",
        "raw_events": [execution_identity],
        "model_rounds": 2,
        "forge_run_evidence": {
            "provider_usage": {
                "events": [
                    {
                        "provider_id": "deepseek",
                        "model": "deepseek-v4-flash",
                        "input_tokens": 1000,
                        "cache_read_tokens": 250,
                        "output_tokens": 100,
                        "reasoning_tokens": None,
                        "source": "deepseek",
                        "reason": "provider_reported",
                    }
                ]
            }
        },
    },
    sys.stdout,
)
""".strip()
        + "\n",
        encoding="utf-8",
    )
    command = shlex.join([sys.executable, str(fake_agent), str(GOLDEN_FIXTURE)])
    env = {
        "FORGE_EVAL_FORGE_AGENT_COMMAND": command,
        "DEEPSEEK_API_KEY": "sk-offline-test-must-never-appear",
    }

    initialized = run_baseline_cli(
        "init",
        "--campaign",
        str(campaign),
        "--campaign-id",
        "h1-offline-cli",
        "--profile",
        str(PROFILE_PATH),
        env=env,
    )
    assert initialized.returncode == 0, initialized.stderr
    init_payload = json.loads(initialized.stdout)
    assert init_payload["status"] == "active"
    assert init_payload["task_count"] == 10
    assert init_payload["expected_attempts"] == 30
    assert init_payload["budget_cny"] == "8"
    assert init_payload["spent_cny"] == "0"

    attempted = run_baseline_cli(
        "run",
        "--campaign",
        str(campaign),
        "--profile",
        str(PROFILE_PATH),
        "--max-attempts",
        "1",
        env=env,
    )
    assert attempted.returncode == 0, attempted.stderr
    attempt_payload = json.loads(attempted.stdout)
    assert attempt_payload == {
        "blockers": [],
        "calls_started": 1,
        "spent_cny": "0.000955",
        "status": "paused",
        "terminal_attempts": 1,
    }

    head_before = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=REPOSITORY_ROOT,
        text=True,
        capture_output=True,
        check=True,
    ).stdout
    budget_before = (campaign / "budget.jsonl").read_bytes()
    attempts_before = {
        path.name: path.read_bytes() for path in sorted((campaign / "attempts").glob("*.json"))
    }

    status = run_baseline_cli(
        "status",
        "--campaign",
        str(campaign),
        "--profile",
        str(PROFILE_PATH),
        env=env,
    )
    reported = run_baseline_cli(
        "report",
        "--campaign",
        str(campaign),
        "--profile",
        str(PROFILE_PATH),
        env=env,
    )
    frozen_reported = run_baseline_cli(
        "report",
        "--campaign",
        str(campaign),
        "--profile",
        str(PROFILE_PATH),
        "--frozen",
        env=env,
    )
    verified = run_baseline_cli(
        "verify",
        "--campaign",
        str(campaign),
        "--profile",
        str(PROFILE_PATH),
        env=env,
    )

    assert status.returncode == 0, status.stderr
    assert json.loads(status.stdout)["attempt_count"] == 1
    assert reported.returncode == 0, reported.stderr
    assert json.loads(reported.stdout)["status"] == "incomplete"
    assert frozen_reported.returncode == 0, frozen_reported.stderr
    assert json.loads(frozen_reported.stdout)["status"] == "incomplete"
    assert verified.returncode == 0, verified.stderr
    assert json.loads(verified.stdout)["ok"] is True
    assert (campaign / "budget.jsonl").read_bytes() == budget_before
    assert {
        path.name: path.read_bytes() for path in sorted((campaign / "attempts").glob("*.json"))
    } == attempts_before
    assert (
        subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=REPOSITORY_ROOT,
            text=True,
            capture_output=True,
            check=True,
        ).stdout
        == head_before
    )

    recovered = run_baseline_cli(
        "recover",
        "--campaign",
        str(campaign),
        "--profile",
        str(PROFILE_PATH),
        env=env,
    )
    assert recovered.returncode == 0, recovered.stderr
    assert json.loads(recovered.stdout)["calls_started"] == 0
    assert (campaign / "budget.jsonl").read_bytes() == budget_before

    all_output = "".join(
        result.stdout + result.stderr
        for result in (
            initialized,
            attempted,
            status,
            reported,
            frozen_reported,
            verified,
            recovered,
        )
    )
    assert str(tmp_path) not in all_output
    assert "sk-offline-test-must-never-appear" not in all_output
    assert "Ensure an invalid status transition" not in all_output
