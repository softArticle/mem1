"""
Add memories to mem1-server from a LOCOMO-format dataset (same as mem0 evaluation).
Each conversation has speaker_a and speaker_b; we add messages as mem1 memories
with user_id = {speaker}_{conv_idx}.
"""

import json
import sys
from pathlib import Path

# Allow importing mem1 from repo python/src when not installed
_REPO_ROOT = Path(__file__).resolve().parent.parent.parent
_PYTHON_SRC = _REPO_ROOT / "python" / "src"
if _PYTHON_SRC.is_dir() and str(_PYTHON_SRC) not in sys.path:
    sys.path.insert(0, str(_PYTHON_SRC))

from tqdm import tqdm

from mem1 import Memory


def load_data(data_path: str) -> list:
    with open(data_path, "r") as f:
        return json.load(f)


def add_memories_for_speaker(
    memory: Memory,
    user_id: str,
    messages: list[dict],
    desc: str,
) -> None:
    """Add a batch of messages as separate memories (one content per add)."""
    for msg in tqdm(messages, desc=desc, leave=False):
        content = msg.get("content", "").strip()
        if not content:
            continue
        memory.add(content, user_id=user_id)


def process_conversation(memory: Memory, item: dict, idx: int) -> None:
    """Process one conversation: add all messages for speaker_a and speaker_b."""
    conversation = item["conversation"]
    speaker_a = conversation["speaker_a"]
    speaker_b = conversation["speaker_b"]

    speaker_a_user_id = f"{speaker_a}_{idx}"
    speaker_b_user_id = f"{speaker_b}_{idx}"

    messages_a = []
    messages_b = []
    for key in conversation:
        if key in ("speaker_a", "speaker_b") or "date" in key or "timestamp" in key:
            continue
        chats = conversation.get(key, [])
        if not isinstance(chats, list):
            continue
        for chat in chats:
            speaker = chat.get("speaker", "")
            text = chat.get("text", "").strip()
            if not text:
                continue
            if speaker == speaker_a:
                messages_a.append({"role": "user", "content": f"{speaker_a}: {text}"})
            elif speaker == speaker_b:
                messages_b.append({"role": "user", "content": f"{speaker_b}: {text}"})

    add_memories_for_speaker(
        memory, speaker_a_user_id, messages_a, f"Adding memories for {speaker_a}_{idx}"
    )
    add_memories_for_speaker(
        memory, speaker_b_user_id, messages_b, f"Adding memories for {speaker_b}_{idx}"
    )


def process_all_conversations(
    data_path: str,
    base_url: str = "http://127.0.0.1:8080",
) -> None:
    data = load_data(data_path)
    memory = Memory(base_url=base_url)
    for idx, item in enumerate(tqdm(data, desc="Conversations")):
        process_conversation(memory, item, idx)
    print("Add complete.")


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("data_path", nargs="?", default="dataset/locomo10.json")
    parser.add_argument("--base_url", default="http://127.0.0.1:8080")
    args = parser.parse_args()
    process_all_conversations(args.data_path, args.base_url)
