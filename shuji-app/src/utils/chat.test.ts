import { describe, it, expect } from "vitest";
import { chatMessageKey, mergeMessages, initialCabinetMessage } from "./chat";
import type { ChatMessage } from "../types";

function msg(overrides: Partial<ChatMessage> & { content: string; timestamp: string }): ChatMessage {
  return {
    role: "内阁",
    options: [],
    documents: [],
    ...overrides,
  };
}

describe("chatMessageKey", () => {
  it("includes timestamp, role, content length, and content prefix", () => {
    const m = msg({ timestamp: "t1", role: "皇帝", content: "hello" });
    const key = chatMessageKey(m);
    expect(key).toContain("t1");
    expect(key).toContain("皇帝");
    expect(key).toContain("5"); // content length
    expect(key).toContain("hello");
  });

  it("differs for messages with same prefix but different lengths", () => {
    const a = msg({ timestamp: "t1", role: "皇帝", content: "a".repeat(40) });
    const b = msg({ timestamp: "t1", role: "皇帝", content: "a".repeat(41) });
    expect(chatMessageKey(a)).not.toBe(chatMessageKey(b));
  });
});

describe("mergeMessages", () => {
  it("returns prev unchanged when hist is empty", () => {
    const prev = [msg({ content: "a", timestamp: "t1" })];
    const result = mergeMessages(prev, []);
    expect(result).toBe(prev); // reference equality when no change
  });

  it("returns prev unchanged when no new messages", () => {
    const prev = [msg({ content: "hi", timestamp: "t1" })];
    const hist = [msg({ content: "hi", timestamp: "t1" })];
    const result = mergeMessages(prev, hist);
    expect(result).toBe(prev);
  });

  it("appends new messages from hist", () => {
    const prev = [msg({ content: "a", timestamp: "t1" })];
    const hist = [msg({ content: "b", timestamp: "t2" })];
    const result = mergeMessages(prev, hist);
    expect(result).toHaveLength(2);
    expect(result[0].content).toBe("a");
    expect(result[1].content).toBe("b");
  });

  it("deduplicates by key (same timestamp + role + content length + content prefix)", () => {
    const prev = [msg({ content: "x".repeat(100), timestamp: "t1" })];
    // Same content, but the key uses slice(0,80) of the content
    const hist = [msg({ content: "x".repeat(100), timestamp: "t1" })];
    const result = mergeMessages(prev, hist);
    expect(result).toHaveLength(1);
  });

  it("does not dedup same prefix but different length", () => {
    const prev = [msg({ content: "x".repeat(80), timestamp: "t1" })];
    const hist = [msg({ content: "x".repeat(81), timestamp: "t1" })];
    const result = mergeMessages(prev, hist);
    expect(result).toHaveLength(2);
  });

  it("handles mixed: some old, some new", () => {
    const prev = [msg({ content: "old1", timestamp: "t1" }), msg({ content: "old2", timestamp: "t2" })];
    const hist = [
      msg({ content: "old1", timestamp: "t1" }),
      msg({ content: "new1", timestamp: "t3" }),
    ];
    const result = mergeMessages(prev, hist);
    expect(result).toHaveLength(3);
    expect(result.map((m) => m.content)).toEqual(["old1", "old2", "new1"]);
  });
});

describe("initialCabinetMessage", () => {
  it("creates a message with 内阁 role", () => {
    const m = initialCabinetMessage("测试");
    expect(m.role).toBe("内阁");
    expect(m.content).toBe("测试");
  });

  it("sets empty options and documents", () => {
    const m = initialCabinetMessage("hello");
    expect(m.options).toEqual([]);
    expect(m.documents).toEqual([]);
  });

  it("sets a valid ISO timestamp", () => {
    const m = initialCabinetMessage("x");
    expect(() => new Date(m.timestamp)).not.toThrow();
    expect(new Date(m.timestamp).toISOString()).toBe(m.timestamp);
  });
});
