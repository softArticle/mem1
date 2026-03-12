# mem0-style Memory class (T026). Add, search, get, delete.

from typing import Any, Optional, Union

from mem1.client import Mem1Client
from mem1.models import AddResponse, SearchResponse


class Memory:
    """mem0-style interface: add(messages|text, user_id), search(query, user_id, limit)."""

    def __init__(self, base_url: str = "http://127.0.0.1:8080", api_key: Optional[str] = None):
        self._client = Mem1Client(base_url=base_url, api_key=api_key)

    def add(
        self,
        messages: Union[list[dict], str],
        user_id: str = "default_user",
        **kwargs: Any,
    ) -> dict:
        if isinstance(messages, str):
            resp = self._client.add(user_id=user_id, content=messages, metadata=kwargs)
        else:
            # Messages form: send as content (concatenate) or first message content for MVP
            content = " ".join(
                m.get("content", "") for m in messages if isinstance(m, dict) and m.get("content")
            )
            if not content:
                content = "(no content)"
            resp = self._client.add(user_id=user_id, content=content, metadata=kwargs)
        return resp.model_dump()

    def search(
        self,
        query: str,
        user_id: str = "default_user",
        limit: int = 10,
        **kwargs: Any,
    ) -> dict:
        resp = self._client.search(user_id=user_id, query=query, limit=limit)
        return resp.model_dump()

    def get(self, memory_id: str, user_id: str = "default_user") -> Optional[dict]:
        r = self._client.get(memory_id=memory_id, user_id=user_id)
        return r.model_dump() if r else None

    def delete(self, memory_id: str, user_id: str = "default_user") -> bool:
        return self._client.delete(memory_id=memory_id, user_id=user_id)
