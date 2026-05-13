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
    metadata: Optional[dict[str, Any]] = None


class SearchRequest(BaseModel):
    user_id: str
    query: str
    limit: int = Field(default=10, ge=1, le=100)
    filters: dict[str, Any] = Field(default_factory=dict)


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
    formatted_context: Optional[str] = None


class DeleteAllResponse(BaseModel):
    deleted: int


class UsersResponse(BaseModel):
    users: list[str]


class MemoryHistoryResult(BaseModel):
    id: str
    memory_id: str
    user_id: str
    operation: str
    previous: Optional[MemoryResult] = None
    current: Optional[MemoryResult] = None
    created_at: str


class HistoryResponse(BaseModel):
    results: list[MemoryHistoryResult]
