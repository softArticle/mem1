# Analyzer Output Contract (Machine-Readable)

**Feature**: 001-ai-iterative-development  
**Date**: 2026-03-09

## Format

JSON object or array, emitted to a file (e.g. `suggestions.json`) or stdout so that an AI coding assistant or other tools can consume it programmatically.

## Schema (advisory)

```json
{
  "run_id": "uuid-or-run-id",
  "code_version": "git-commit-hash",
  "analyzed_at": "ISO8601",
  "suggestions": [
    {
      "id": "suggestion-1",
      "type": "regression | weak_category | bottleneck | failure_pattern | other",
      "summary": "Short human-readable summary",
      "detail": "Optional longer description or evidence",
      "metric_ref": "Optional reference (e.g. run_id, category name, metric name)",
      "priority": "high | medium | low"
    }
  ]
}
```

- **run_id**: The run this analysis is for (and optionally previous run for comparison).
- **suggestions**: Array of at least zero items. Each item MUST have `id`, `type`, `summary`; other fields optional.
- **type**: Extensible; suggested values: `regression`, `weak_category`, `bottleneck`, `failure_pattern`, `other`.

## Compatibility

- Consumers MUST allow unknown fields and new suggestion `type` values so that the analyzer can be extended without breaking clients.
- Adding new optional fields to each suggestion is backward-compatible.
