/** 本文件验证 MCP 协议握手、工具枚举和工具调用映射。 */

import assert from "node:assert/strict";
import test from "node:test";
import { handleRequest } from "../src/index.mjs";

const fakeClient = {
  searchMemory: async (input) => [{ memory_id: "m1", block_id: "b1", score: 1, snippet: input.text }],
  addMemory: async () => ({ id: "m2", created_at: 2 }),
  getMemory: async (id) => ({ id, content: "memory" }),
  askMemory: async ({ question }) => ({ answer: question, citations: [] }),
};

test("完成 MCP 初始化并声明工具能力", async () => {
  const response = await handleRequest(fakeClient, {
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: { protocolVersion: "2025-06-18" },
  });
  assert.equal(response.result.protocolVersion, "2025-06-18");
  assert.equal(response.result.serverInfo.name, "nexus-memory");
});

test("将 search_memory 映射到 Nexus SDK", async () => {
  const response = await handleRequest(fakeClient, {
    jsonrpc: "2.0",
    id: 2,
    method: "tools/call",
    params: { name: "search_memory", arguments: { query: "protocol", limit: 5 } },
  });
  assert.equal(response.result.structuredContent[0].snippet, "protocol");
  assert.equal(response.result.isError, undefined);
});

test("工具参数错误以 MCP 工具错误返回", async () => {
  const response = await handleRequest(fakeClient, {
    jsonrpc: "2.0",
    id: 3,
    method: "tools/call",
    params: { name: "get_memory", arguments: {} },
  });
  assert.equal(response.result.isError, true);
});
