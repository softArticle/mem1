# mem0-style Memory class (T026). Add, search, get, delete.

from typing import Any, Optional, Union

from mem1.client import Mem1Client


class Memory:
    """mem0-style interface: add(messages|text, user_id), search(query, user_id, limit)."""

    def __init__(self, base_url: str = "http://127.0.0.1:8080", api_key: Optional[str] = None):
        self._client = Mem1Client(base_url=base_url, api_key=api_key)

    @staticmethod
    def _merge_filters(filters: Optional[dict[str, Any]], kwargs: dict[str, Any]) -> dict[str, Any]:
        merged = dict(filters or {})
        for key, value in kwargs.items():
            if value is not None:
                merged[key] = value
        return merged

    def add(
        self,
        messages: Union[list[dict], str],
        user_id: str = "default_user",
        **kwargs: Any,
    ) -> dict:
        if isinstance(messages, str):
            resp = self._client.add(user_id=user_id, content=messages, metadata=kwargs)
        else:
            normalized_messages = [
                {
                    "role": str(m.get("role") or "message"),
                    "content": str(m.get("content") or ""),
                }
                for m in messages
                if isinstance(m, dict) and m.get("content")
            ]
            if not normalized_messages:
                normalized_messages = [{"role": "message", "content": "(no content)"}]
            resp = self._client.add_messages(
                user_id=user_id,
                messages=normalized_messages,
                metadata=kwargs,
            )
        return resp.model_dump()

    def search(
        self,
        query: str,
        user_id: str = "default_user",
        limit: int = 10,
        filters: Optional[dict[str, Any]] = None,
        **kwargs: Any,
    ) -> dict:
        resp = self._client.search(
            user_id=user_id,
            query=query,
            limit=limit,
            filters=self._merge_filters(filters, kwargs),
        )
        return resp.model_dump()

    def get_all(
        self,
        user_id: str = "default_user",
        limit: int = 10,
        offset: int = 0,
        filters: Optional[dict[str, Any]] = None,
        **kwargs: Any,
    ) -> dict:
        resp = self._client.list(
            user_id=user_id,
            limit=limit,
            offset=offset,
            filters=self._merge_filters(filters, kwargs),
        )
        return resp.model_dump()

    def update(
        self,
        memory_id: str,
        data: Optional[str] = None,
        user_id: str = "default_user",
        metadata: Optional[dict[str, Any]] = None,
        **kwargs: Any,
    ) -> dict:
        merged_metadata = metadata
        if kwargs:
            merged_metadata = dict(metadata or {})
            merged_metadata.update(kwargs)
        resp = self._client.update(
            memory_id=memory_id,
            user_id=user_id,
            content=data,
            metadata=merged_metadata,
        )
        return resp.model_dump()

    def get(self, memory_id: str, user_id: str = "default_user") -> Optional[dict]:
        r = self._client.get(memory_id=memory_id, user_id=user_id)
        return r.model_dump() if r else None

    def delete(self, memory_id: str, user_id: str = "default_user") -> bool:
        return self._client.delete(memory_id=memory_id, user_id=user_id)

    def delete_all(
        self,
        user_id: str = "default_user",
        filters: Optional[dict[str, Any]] = None,
        **kwargs: Any,
    ) -> dict:
        resp = self._client.delete_all(
            user_id=user_id,
            filters=self._merge_filters(filters, kwargs),
        )
        return resp.model_dump()

    def history(self, memory_id: str, user_id: str = "default_user") -> dict:
        resp = self._client.history(memory_id=memory_id, user_id=user_id)
        return resp.model_dump()

    def users(self) -> dict:
        return self._client.users().model_dump()

    def reset(self) -> dict:
        return self._client.reset().model_dump()
