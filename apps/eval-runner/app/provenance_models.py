import json
import re
from decimal import Decimal
from pathlib import PurePosixPath
from typing import Annotated, Any, Literal

from pydantic import Field, StringConstraints, field_validator, model_validator

from app.models import EvalModel

Sha256 = Annotated[str, StringConstraints(pattern=r"^[0-9a-f]{64}$")]
GitSha1 = Annotated[str, StringConstraints(pattern=r"^[0-9a-f]{40}$")]

SECRET_PATTERN = re.compile(
    # Match token-shaped values, not source-code identifiers such as
    # ``api_key = os.getenv(...)`` that legitimately appear in traces.
    r"(?i)(?:sk-[a-z0-9_-]{16,}|bearer\s+[a-z0-9._-]{12,})"
)
SAFE_ID_PATTERN = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")


def _contains_secret(value: Any) -> bool:
    serialized = json.dumps(value, sort_keys=True, default=str)
    return SECRET_PATTERN.search(serialized) is not None


class DigestRef(EvalModel):
    algorithm: Literal["sha256"] = "sha256"
    digest: Sha256


class ArtifactDigest(EvalModel):
    id: str
    path: str
    sha256: Sha256

    @field_validator("id")
    @classmethod
    def validate_id(cls, value: str) -> str:
        if not value.strip():
            raise ValueError("artifact id must not be empty")
        return value

    @field_validator("path")
    @classmethod
    def validate_path(cls, value: str) -> str:
        path = PurePosixPath(value)
        if not value or path.is_absolute() or ".." in path.parts or value == ".":
            raise ValueError("artifact path must be a safe campaign-relative path")
        return value


class VerificationDigest(EvalModel):
    command: str
    exit_code: int
    stdout_sha256: Sha256
    stderr_sha256: Sha256
    verified_commit: GitSha1
    passed: bool

    @model_validator(mode="after")
    def validate_result(self) -> "VerificationDigest":
        if self.passed != (self.exit_code == 0):
            raise ValueError("verification passed must match exit code")
        if _contains_secret(self.model_dump(mode="json")):
            raise ValueError("verification metadata contains secret-like content")
        return self


class ControllerIntervention(EvalModel):
    sequence: int = Field(ge=0)
    kind: Literal["campaign_init", "manifest_finalize", "product_source_edit"]
    reason: str
    invalidates_campaign: bool

    @model_validator(mode="after")
    def validate_intervention(self) -> "ControllerIntervention":
        if not self.reason.strip():
            raise ValueError("controller intervention reason must not be empty")
        if self.invalidates_campaign != (self.kind == "product_source_edit"):
            raise ValueError("only product source edits invalidate a campaign")
        if _contains_secret(self.model_dump(mode="json")):
            raise ValueError("controller intervention contains secret-like content")
        return self


class ForgeProvenanceEntry(EvalModel):
    schema_version: Literal[1] = 1
    sequence: int = Field(ge=1)
    previous_entry_sha256: Sha256 | None
    task_id: str
    run_id: str
    agent: Literal["forge"] = "forge"
    provider: str
    model: str
    transport: str
    input_commit: GitSha1
    patch_sha256: Sha256
    output_commit: GitSha1
    trace_sha256: Sha256
    report_sha256: Sha256
    harness_fingerprint: Sha256
    dataset_fingerprint: Sha256
    verification_digest: Sha256
    trust_status: Literal["trusted"] = "trusted"
    cost_cny: Decimal = Field(ge=0)

    @model_validator(mode="after")
    def validate_entry(self) -> "ForgeProvenanceEntry":
        for field_name in ("task_id", "run_id", "provider", "model", "transport"):
            if not str(getattr(self, field_name)).strip():
                raise ValueError(f"{field_name} must not be empty")
        for field_name in ("task_id", "run_id"):
            if SAFE_ID_PATTERN.fullmatch(str(getattr(self, field_name))) is None:
                raise ValueError(f"{field_name} must be a safe identifier")
        if _contains_secret(self.model_dump(mode="json")):
            raise ValueError("provenance entry contains secret-like content")
        return self


class ForgeBuildManifest(EvalModel):
    schema_version: Literal[1] = 1
    campaign_id: str
    status: Literal["complete", "invalid"]
    baseline_commit: GitSha1
    final_commit: GitSha1
    fixture: ArtifactDigest
    development_dataset: ArtifactDigest
    holdout_dataset: ArtifactDigest
    harness_profile: ArtifactDigest
    provenance_ledger: ArtifactDigest
    artifact_index: ArtifactDigest
    final_tree: ArtifactDigest
    tree_digest_profile: Literal["forgebench-v1"]
    final_verification: VerificationDigest
    ledger_verified: bool
    accepted_tasks: list[str]
    controller_interventions: list[ControllerIntervention]

    @model_validator(mode="after")
    def validate_manifest(self) -> "ForgeBuildManifest":
        if not self.campaign_id.strip():
            raise ValueError("campaign id must not be empty")
        if len(self.accepted_tasks) != len(set(self.accepted_tasks)):
            raise ValueError("accepted task ids must be unique")
        if any(not task_id.strip() for task_id in self.accepted_tasks):
            raise ValueError("accepted task ids must not be empty")

        expected_sequences = list(range(len(self.controller_interventions)))
        actual_sequences = [item.sequence for item in self.controller_interventions]
        if actual_sequences != expected_sequences:
            raise ValueError("controller intervention sequence must be contiguous")

        artifacts = (
            self.fixture,
            self.development_dataset,
            self.holdout_dataset,
            self.harness_profile,
            self.provenance_ledger,
            self.artifact_index,
            self.final_tree,
        )
        if len({artifact.id for artifact in artifacts}) != len(artifacts):
            raise ValueError("manifest artifact ids must be unique")
        if self.final_verification.verified_commit != self.final_commit:
            raise ValueError("final verification commit must match final commit")

        if self.status == "complete":
            if not self.ledger_verified:
                raise ValueError("complete manifest requires a valid ledger")
            if not self.final_verification.passed:
                raise ValueError("complete manifest requires passing final verification")
            if not self.accepted_tasks:
                raise ValueError("complete manifest requires at least one accepted Forge task")
            if not self.controller_interventions:
                raise ValueError("complete manifest requires controller interventions")
            if self.controller_interventions[0].kind != "campaign_init":
                raise ValueError("complete manifest must begin with campaign initialization")
            if self.controller_interventions[-1].kind != "manifest_finalize":
                raise ValueError("complete manifest must end with manifest finalization")
            if any(item.invalidates_campaign for item in self.controller_interventions):
                raise ValueError("controller product edits invalidate a complete campaign")

        if _contains_secret(self.model_dump(mode="json")):
            raise ValueError("build manifest contains secret-like content")
        return self
