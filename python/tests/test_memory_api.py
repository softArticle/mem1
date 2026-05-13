from mem1.memory import Memory


class Dumpable:
    def __init__(self, value):
        self.value = value

    def model_dump(self):
        return self.value


class FakeClient:
    def __init__(self):
        self.calls = []

    def add(self, **kwargs):
        self.calls.append(("add", kwargs))
        return Dumpable(
            {
                "results": [
                    {
                        "id": "m1",
                        "content": "Alice likes Rust.",
                        "user_id": kwargs["user_id"],
                        "metadata": {"source_index": 0},
                        "created_at": "2026-05-13T00:00:00Z",
                    },
                    {
                        "id": "m2",
                        "content": "Alice lives in Paris.",
                        "user_id": kwargs["user_id"],
                        "metadata": {"source_index": 0},
                        "created_at": "2026-05-13T00:00:01Z",
                    },
                ]
            }
        )

    def add_messages(self, **kwargs):
        self.calls.append(("add_messages", kwargs))
        return Dumpable(
            {
                "results": [
                    {
                        "id": "m1",
                        "content": "I prefer tea.",
                        "user_id": kwargs["user_id"],
                        "metadata": {"source_role": "user", "source_index": 0},
                        "created_at": "2026-05-13T00:00:00Z",
                    },
                    {
                        "id": "m2",
                        "content": "Noted.",
                        "user_id": kwargs["user_id"],
                        "metadata": {"source_role": "assistant", "source_index": 1},
                        "created_at": "2026-05-13T00:00:01Z",
                    },
                ]
            }
        )

    def search(self, **kwargs):
        self.calls.append(("search", kwargs))
        return Dumpable({"results": [], "formatted_context": "ctx"})

    def list(self, **kwargs):
        self.calls.append(("list", kwargs))
        return Dumpable({"results": []})

    def update(self, **kwargs):
        self.calls.append(("update", kwargs))
        return Dumpable({"id": "m1", "content": "new"})

    def delete_all(self, **kwargs):
        self.calls.append(("delete_all", kwargs))
        return Dumpable({"deleted": 2})

    def history(self, **kwargs):
        self.calls.append(("history", kwargs))
        return Dumpable({"results": []})

    def users(self):
        self.calls.append(("users", {}))
        return Dumpable({"users": ["u1"]})

    def reset(self):
        self.calls.append(("reset", {}))
        return Dumpable({"deleted": 3})


def memory_with_fake_client():
    memory = Memory()
    fake = FakeClient()
    memory._client = fake
    return memory, fake


def test_add_content_returns_all_fanned_out_results():
    memory, fake = memory_with_fake_client()

    result = memory.add(
        "Alice likes Rust. Alice lives in Paris.",
        user_id="u1",
        scope="profile",
    )

    assert [item["content"] for item in result["results"]] == [
        "Alice likes Rust.",
        "Alice lives in Paris.",
    ]
    assert fake.calls == [
        (
            "add",
            {
                "user_id": "u1",
                "content": "Alice likes Rust. Alice lives in Paris.",
                "metadata": {"scope": "profile"},
            },
        )
    ]


def test_add_messages_forwards_message_payload_and_returns_fanned_out_results():
    memory, fake = memory_with_fake_client()
    messages = [
        {"role": "user", "content": "I prefer tea."},
        {"role": "assistant", "content": "Noted."},
    ]

    result = memory.add(messages, user_id="u1", agent_id="agent-a")

    assert [item["content"] for item in result["results"]] == [
        "I prefer tea.",
        "Noted.",
    ]
    assert result["results"][0]["metadata"] == {"source_role": "user", "source_index": 0}
    assert result["results"][1]["metadata"] == {
        "source_role": "assistant",
        "source_index": 1,
    }
    assert fake.calls == [
        (
            "add_messages",
            {
                "user_id": "u1",
                "messages": messages,
                "metadata": {"agent_id": "agent-a"},
            },
        )
    ]


def test_search_forwards_mem0_style_filters():
    memory, fake = memory_with_fake_client()

    result = memory.search(
        "find preferences",
        user_id="u1",
        limit=5,
        filters={"agent_id": "agent-a"},
        run_id="run-1",
    )

    assert result["formatted_context"] == "ctx"
    assert fake.calls == [
        (
            "search",
            {
                "user_id": "u1",
                "query": "find preferences",
                "limit": 5,
                "filters": {"agent_id": "agent-a", "run_id": "run-1"},
            },
        )
    ]


def test_memory_exposes_basic_service_read_management_methods():
    memory, fake = memory_with_fake_client()

    assert memory.get_all(user_id="u1", limit=20, scope="project") == {"results": []}
    assert memory.update("m1", "new", user_id="u1") == {"id": "m1", "content": "new"}
    assert memory.delete_all(user_id="u1", run_id="run-1") == {"deleted": 2}
    assert memory.history("m1", user_id="u1") == {"results": []}
    assert memory.users() == {"users": ["u1"]}
    assert memory.reset() == {"deleted": 3}

    assert fake.calls == [
        ("list", {"user_id": "u1", "limit": 20, "offset": 0, "filters": {"scope": "project"}}),
        ("update", {"memory_id": "m1", "user_id": "u1", "content": "new", "metadata": None}),
        ("delete_all", {"user_id": "u1", "filters": {"run_id": "run-1"}}),
        ("history", {"memory_id": "m1", "user_id": "u1"}),
        ("users", {}),
        ("reset", {}),
    ]
