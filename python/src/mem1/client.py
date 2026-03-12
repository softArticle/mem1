# HTTP client for mem1 server (T024).

import httpx
from typing import Any, Optional

from mem1.models import AddResponse, MemoryResult, SearchResponse


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

    def add(self, user_id: str, content: str, metadata: Optional[dict] = None) -> AddResponse:
        with httpx.Client() as client:
            r = client.post(
                f"{self.base_url}/memories",
                json={"user_id": user_id, "content": content, "metadata": metadata or {}},
                headers=self._headers(),
                timeout=30.0,
            )
        if r.status_code != 201:
            body = r.json() if r.headers.get("content-type", "").startswith("application/json") else {}
            raise ClientError(
                body.get("code", "UNKNOWN"),
                body.get("message", r.text),
                body.get("trace_id"),
            )
        return AddResponse.model_validate(r.json())

    def search(self, user_id: str, query: str, limit: int = 10) -> SearchResponse:
        with httpx.Client() as client:
            r = client.post(
                f"{self.base_url}/memories/search",
                json={"user_id": user_id, "query": query, "limit": limit},
                headers=self._headers(),
                timeout=30.0,
            )
        if r.status_code != 200:
            body = r.json() if r.headers.get("content-type", "").startswith("application/json") else {}
            raise ClientError(
                body.get("code", "UNKNOWN"),
                body.get("message", r.text),
                body.get("trace_id"),
            )
        return SearchResponse.model_validate(r.json())

    def get(self, memory_id: str, user_id: str) -> Optional[MemoryResult]:
        with httpx.Client() as client:
            r = client.get(
                f"{self.base_url}/memories/{memory_id}",
                params={"user_id": user_id},
                headers=self._headers(),
                timeout=30.0,
            )
        if r.status_code == 404:
            return None
        if r.status_code != 200:
            body = r.json() if r.headers.get("content-type", "").startswith("application/json") else {}
            raise ClientError(
                body.get("code", "UNKNOWN"),
                body.get("message", r.text),
                body.get("trace_id"),
            )
        return MemoryResult.model_validate(r.json())

    def delete(self, memory_id: str, user_id: str) -> bool:
        with httpx.Client() as client:
            r = client.delete(
                f"{self.base_url}/memories/{memory_id}",
                params={"user_id": user_id},
                headers=self._headers(),
                timeout=30.0,
            )
        if r.status_code == 404:
            return False
        if r.status_code != 204:
            body = r.json() if r.headers.get("content-type", "").startswith("application/json") else {}
            raise ClientError(
                body.get("code", "UNKNOWN"),
                body.get("message", r.text),
                body.get("trace_id"),
            )
        return True
