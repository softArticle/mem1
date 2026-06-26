// Run SAG over LOCOMO medium, same gateway answerer, emit sag_results.json for evals.py.
// Each (speaker, conv) is an isolated SAG source. Ingest that speaker's turns as one
// document; per QA, search both speakers' sources, take event contents as context,
// answer via the same GPT-5.5 ANSWER_PROMPT as mem1/mem0.
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { ingestionService } from "../src/services/ingestion-service.js";
import { searchService } from "../src/services/search-service.js";
import { closePool } from "../src/db/pool.js";

const API_KEY = process.env.LLM_API_KEY!;
const BASE_URL = process.env.LLM_BASE_URL!.replace(/\/$/, "");
const MODEL = process.env.LLM_MODEL!;
const DATA = "/Users/lishuo121/mem1/evaluation/dataset/medium_locomo.json";
const OUT = "/Users/lishuo121/mem1/evaluation/results/sag_results.json";

// Deterministic UUID v4-ish from a string (so sourceIds are stable + valid uuid).
function uuidFrom(s: string): string {
  const h = createHash("sha256").update(s).digest("hex");
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-4${h.slice(13, 16)}-8${h.slice(17, 20)}-${h.slice(20, 32)}`;
}

const ANSWER_PROMPT = (s1: string, m1: string, s2: string, m2: string, q: string) =>
  `You are an intelligent memory assistant. Use only the provided memories to answer the question.
Resolve relative dates using the memory Date shown. Answer the inferred event date/month/year.

Memories for user ${s1}:
${m1 || "(none)"}

Memories for user ${s2}:
${m2 || "(none)"}

Question: ${q}

Answer in 5-6 words or less:`;

async function answer(prompt: string): Promise<string> {
  for (let i = 0; i < 4; i++) {
    try {
      const r = await fetch(`${BASE_URL}/chat/completions`, {
        method: "POST",
        headers: { "Content-Type": "application/json", Authorization: `Bearer ${API_KEY}` },
        body: JSON.stringify({ model: MODEL, messages: [{ role: "user", content: prompt }] }),
        signal: AbortSignal.timeout(60000),
      });
      if (r.ok) {
        const d: any = await r.json();
        return d?.choices?.[0]?.message?.content ?? "";
      }
    } catch {}
    await new Promise((res) => setTimeout(res, 1500 * (i + 1)));
  }
  return "(LLM error)";
}

function speakerMessages(conv: any): [string, string, string[], string[]] {
  const sa = conv.speaker_a, sb = conv.speaker_b;
  const ma: string[] = [], mb: string[] = [];
  for (const key of Object.keys(conv)) {
    if (key === "speaker_a" || key === "speaker_b" || key.includes("date") || key.includes("timestamp")) continue;
    const chats = conv[key];
    if (!Array.isArray(chats)) continue;
    for (const c of chats) {
      const sp = c.speaker, txt = (c.text || "").trim();
      if (!txt) continue;
      const line = `${sp}: ${txt}`;
      if (sp === sa) ma.push(line); else if (sp === sb) mb.push(line);
    }
  }
  return [sa, sb, ma, mb];
}

async function main() {
  const data = JSON.parse(readFileSync(DATA, "utf8"));
  // INGEST (skip if SKIP_INGEST=1 and DB already populated)
  for (let idx = 0; process.env.SKIP_INGEST !== "1" && idx < data.length; idx++) {
    const conv = data[idx].conversation;
    const [sa, sb, ma, mb] = speakerMessages(conv);
    for (const [name, msgs] of [[`${sa}_${idx}`, ma], [`${sb}_${idx}`, mb]] as [string, string[]][]) {
      if (msgs.length === 0) continue;
      const sourceId = uuidFrom(name);
      // Ingest EACH message as its own document so SAG extracts a per-message event
      // (preserving detail) — matching mem1's one-memory-per-message granularity.
      // One whole-speaker document collapses ~200 turns into a single fused event,
      // which is unfair to a conversational-memory benchmark.
      for (let mi = 0; mi < msgs.length; mi++) {
        try {
          await ingestionService.ingestDocument({ sourceId, title: `${name}#${mi}`, content: msgs[mi], extract: true });
        } catch (e) { process.stderr.write(`ingest err: ${e}\n`); }
      }
      process.stderr.write(`ingested ${name} (${msgs.length} msgs)\n`);
    }
  }
  process.stderr.write("INGEST done\n");
  // SEARCH + ANSWER
  const out: Record<string, any[]> = {};
  for (let idx = 0; idx < data.length; idx++) {
    const conv = data[idx].conversation;
    const sa = conv.speaker_a, sb = conv.speaker_b;
    const srcA = uuidFrom(`${sa}_${idx}`), srcB = uuidFrom(`${sb}_${idx}`);
    const qas = data[idx].qa || [];
    out[String(idx)] = [];
    let n = 0;
    for (const qa of qas) {
      const q = qa.question || "";
      const gold = String(qa.answer ?? "");
      const cat = String(qa.category ?? -1);
      if (!q) continue;
      const grab = async (src: string) => {
        try {
          const r: any = await searchService.search({ query: q, sourceIds: [src], strategy: "multi", topK: 30, returnTrace: false });
          return (r.sections || []).map((s: any) => s.content || s.heading || "").filter(Boolean);
        } catch (e) { process.stderr.write(`search err: ${e}\n`); return []; }
      };
      const la = await grab(srcA), lb = await grab(srcB);
      const resp = await answer(ANSWER_PROMPT(sa, la.join("\n"), sb, lb.join("\n"), q));
      out[String(idx)].push({
        question: q, answer: gold, category: cat, response: resp,
        speaker_1_memories: la.map((c) => ({ content: c })),
        speaker_2_memories: lb.map((c) => ({ content: c })),
      });
      if (++n % 20 === 0) process.stderr.write(`conv ${idx}: ${n}/${qas.length}\n`);
    }
  }
  writeFileSync(OUT, JSON.stringify(out, null, 2));
  process.stderr.write(`SEARCH done -> ${OUT}\n`);
}

main().finally(() => closePool());
