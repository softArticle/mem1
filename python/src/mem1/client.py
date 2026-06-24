# HTTP client for mem1 server (T024).

import httpx
from typing import Any, Optional

from mem1.models import (
    AddResponse,
    DeleteAllResponse,
    HistoryResponse,
    MemoryResult,
    SearchResponse,
    UsersResponse,
)


class ClientError(Exception):
    def __init__(self, code: str, message: str, trace_id: Optional[str] = None):
        self.code = code
        self.message = message
        self.trace_id = trace_id
        super().__init__(f"[{code}] {message}")


class Mem1Client:
    def __init__(self, base_url: str = "http://127.0.0.1:8080", api_key: Optional[str] = None):
        self.base_url = base_url.rstrip("/")
        self._api_key = api_key

    def _headers(self) -> dict[str, str]:
        h = {"Content-Type": "application/json"}
        if self._api_key:
            h["Authorization"] = f"Bearer {self._api_key}"
        return h

    @staticmethod
    def _raise(r: httpx.Response) -> None:
        body = r.json() if r.headers.get("content-type", "").startswith("application/json") else {}
        raise ClientError(
            body.get("code", "UNKNOWN"),
            body.get("message", r.text),
            body.get("trace_id"),
        )

    def add(self, user_id: str, content: str, metadata: Optional[dict] = None) -> AddResponse:
        with httpx.Client() as client:
            r = client.post(
                f"{self.base_url}/memories",
                json={"user_id": user_id, "content": content, "metadata": metadata or {}},
                headers=self._headers(),
                timeout=120.0,
            )
        if r.status_code != 201:
            self._raise(r)
        return AddResponse.model_validate(r.json())

    def add_messages(
        self,
        user_id: str,
        messages: list[dict[str, str]],
        metadata: Optional[dict] = None,
    ) -> AddResponse:
        with httpx.Client() as client:
            r = client.post(
                f"{self.base_url}/memories",
                json={"user_id": user_id, "messages": messages, "metadata": metadata or {}},
                headers=self._headers(),
                timeout=120.0,
            )
        if r.status_code != 201:
            self._raise(r)
        return AddResponse.model_validate(r.json())

    def search(
        self,
        user_id: str,
        query: str,
        limit: int = 10,
        filters: Optional[dict[str, Any]] = None,
    ) -> SearchResponse:
        with httpx.Client() as client:
            r = client.post(
                f"{self.base_url}/memories/search",
                json={"user_id": user_id, "query": query, "limit": limit, "filters": filters or {}},
                headers=self._headers(),
                timeout=120.0,
            )
        if r.status_code != 200:
            self._raise(r)
        return SearchResponse.model_validate(r.json())

    def list(
        self,
        user_id: str,
        limit: int = 10,
        offset: int = 0,
        filters: Optional[dict[str, Any]] = None,
    ) -> AddResponse:
        params: dict[str, Any] = {"user_id": user_id, "limit": limit, "offset": offset}
        params.update(filters or {})
        with httpx.Client() as client:
            r = client.get(
                f"{self.base_url}/memories",
                params=params,
                headers=self._headers(),
                timeout=120.0,
            )
        if r.status_code != 200:
            self._raise(r)
        return AddResponse.model_validate(r.json())

    def get(self, memory_id: str, user_id: str) -> Optional[MemoryResult]:
        with httpx.Client() as client:
            r = client.get(
                f"{self.base_url}/memories/{memory_id}",
                params={"user_id": user_id},
                headers=self._headers(),
                timeout=120.0,
            )
        if r.status_code == 404:
            return None
        if r.status_code != 200:
            self._raise(r)
        return MemoryResult.model_validate(r.json())

    def update(
        self,
        memory_id: str,
        user_id: str,
        content: Optional[str] = None,
        metadata: Optional[dict[str, Any]] = None,
    ) -> MemoryResult:
        with httpx.Client() as client:
            r = client.patch(
                f"{self.base_url}/memories/{memory_id}",
                json={"user_id": user_id, "content": content, "metadata": metadata or {}},
                headers=self._headers(),
                timeout=120.0,
            )
        if r.status_code != 200:
            self._raise(r)
        return MemoryResult.model_validate(r.json())

    def delete(self, memory_id: str, user_id: str) -> bool:
        with httpx.Client() as client:
            r = client.delete(
                f"{self.base_url}/memories/{memory_id}",
                params={"user_id": user_id},
                headers=self._headers(),
                timeout=120.0,
            )
        if r.status_code == 404:
            return False
        if r.status_code != 204:
            self._raise(r)
        return True

    def delete_all(
        self,
        user_id: str,
        filters: Optional[dict[str, Any]] = None,
    ) -> DeleteAllResponse:
        params: dict[str, Any] = {"user_id": user_id}
        params.update(filters or {})
        with httpx.Client() as client:
            r = client.delete(
                f"{self.base_url}/memories",
                params=params,
                headers=self._headers(),
                timeout=120.0,
            )
        if r.status_code != 200:
            self._raise(r)
        return DeleteAllResponse.model_validate(r.json())

    def history(self, memory_id: str, user_id: str) -> HistoryResponse:
        with httpx.Client() as client:
            r = client.get(
                f"{self.base_url}/memories/{memory_id}/history",
                params={"user_id": user_id},
                headers=self._headers(),
                timeout=120.0,
            )
        if r.status_code != 200:
            self._raise(r)
        return HistoryResponse.model_validate(r.json())

    def users(self) -> UsersResponse:
        with httpx.Client() as client:
            r = client.get(
                f"{self.base_url}/users",
                headers=self._headers(),
                timeout=120.0,
            )
        if r.status_code != 200:
            self._raise(r)
        return UsersResponse.model_validate(r.json())

    def reset(self) -> DeleteAllResponse:
        with httpx.Client() as client:
            r = client.post(
                f"{self.base_url}/reset",
                headers=self._headers(),
                timeout=120.0,
            )
        if r.status_code != 200:
            self._raise(r)
        return DeleteAllResponse.model_validate(r.json())
