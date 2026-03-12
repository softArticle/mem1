# Request/response models (T025). Pydantic models aligned with api-http contract.

from typing import Any, Optional

from pydantic import BaseModel, Field


class AddByContent(BaseModel):
    user_id: str
    content: str
    metadata: Optional[dict[str, Any]] = None


class AddByMessages(BaseModel):
    user_id: str
    messages: list[dict[str, str]]


class SearchRequest(BaseModel):
    user_id: str
    query: str
    limit: int = Field(default=10, ge=1, le=100)


class MemoryResult(BaseModel):
    id: str
    content: str
    user_id: str
    metadata: dict[str, Any] = Field(default_factory=dict)
    created_at: str
    score: Optional[float] = None


class AddResponse(BaseModel):
    results: list[MemoryResult]


class SearchResponse(BaseModel):
    results: list[MemoryResult]
