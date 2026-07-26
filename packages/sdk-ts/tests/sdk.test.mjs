/** 本文件验证公开 TypeScript SDK 会固定外部来源并透传检索结果。 */

import assert from "node:assert/strict";
import test from "node:test";
import { NexusClient, ProtocolClientError } from "../dist/index.js";

/** 创建记录请求的 Fetch 测试替身。 */
function recordingFetch(payload) {
  const calls = [];
  return {
    calls,
    fetch: async (input, init) => {
      calls.push({ input: String(input), init });
      return Response.json(payload, { status: 201 });
    },
  };
}

test("写入时固定授权来源", async () => {
  const mock = recordingFetch({ id: "memory-1", created_at: 1 });
  const client = new NexusClient({
    endpoint: "http://127.0.0.1:4111",
    token: "secret",
    source: "external:sample",
    fetch: mock.fetch,
  });
  await client.addMemory({ content: "SDK memory", tags: ["sdk"] });
  assert.deepEqual(JSON.parse(mock.calls[0].init.body), {
    source: "external:sample",
    kind: "note",
    content: "SDK memory",
    content_format: "markdown",
    tags: ["sdk"],
  });
  assert.equal(mock.calls[0].init.headers.get("authorization"), "Bearer secret");
});

test("拒绝不合法的公开来源", () => {
  assert.throws(
    () => new NexusClient({
      endpoint: "http://127.0.0.1:4111",
      token: "secret",
      source: "external:Bad App",
    }),
    ProtocolClientError,
  );
});
