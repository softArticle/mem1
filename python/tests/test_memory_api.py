from mem1.memory import Memory


class Dumpable:
    def __init__(self, value):
        self.value = value

    def model_dump(self):
        return self.value


class FakeClient:
    def __init__(self):
        self.calls = []

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
