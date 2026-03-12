"""
Run and Metrics data structures per data-model.md.
"""

from dataclasses import dataclass, field
from typing import Any, Optional


@dataclass
class Run:
    """A single iteration run (eval → collect → analyze)."""

    id: str
    timestamp: str  # ISO 8601
    code_version: str
    outcome: str  # "success" | "failure"
    failure_step: Optional[str] = None
    failure_detail: Optional[str] = None
    metrics_ref: Optional[str] = None
    log_ref: Optional[str] = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "timestamp": self.timestamp,
            "code_version": self.code_version,
            "outcome": self.outcome,
            "failure_step": self.failure_step,
            "failure_detail": self.failure_detail,
            "metrics_ref": self.metrics_ref,
            "log_ref": self.log_ref,
        }

    @classmethod
    def from_dict(cls, d: dict[str, Any]) -> "Run":
        return cls(
            id=d["id"],
            timestamp=d["timestamp"],
            code_version=d["code_version"],
            outcome=d["outcome"],
            failure_step=d.get("failure_step"),
            failure_detail=d.get("failure_detail"),
            metrics_ref=d.get("metrics_ref"),
            log_ref=d.get("log_ref"),
        )


@dataclass
class Metrics:
    """Numeric/structured results from evaluation for a run."""

    run_id: str
    overall: Optional[dict[str, Any]] = None
    by_category: Optional[list[dict[str, Any]]] = None
    latency_ms: Optional[float] = None
    raw_path: Optional[str] = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "run_id": self.run_id,
            "overall": self.overall,
            "by_category": self.by_category,
            "latency_ms": self.latency_ms,
            "raw_path": self.raw_path,
        }

    @classmethod
    def from_dict(cls, d: dict[str, Any]) -> "Metrics":
        return cls(
            run_id=d["run_id"],
            overall=d.get("overall"),
            by_category=d.get("by_category"),
            latency_ms=d.get("latency_ms"),
            raw_path=d.get("raw_path"),
        )
