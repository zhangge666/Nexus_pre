/** 本文件验证共享协议客户端的 URL、认证、错误映射和 SSE 解析行为。 */

import assert from "node:assert/strict";
import test from "node:test";
import { ProtocolClient, ProtocolClientError, createProtocolBaseUrl } from "../dist/index.js";

/** 创建记录请求并返回预设响应的 Fetch 替身。 */
function createFetch(responseFactory) {
  const calls = [];
  return {
    calls,
    fetch: async (input, init) => {
      calls.push({ input: String(input), init });
      return responseFactory();
    },
  };
}

/** 验证客户端规范化端点并附加本地 Bearer 令牌。 */
test("构造请求时使用 v1 路径和认证头", async () => {
  const mock = createFetch(() => Response.json({ id: "memory-1", created_at: 1 }));
  const client = new ProtocolClient({ endpoint: "http://127.0.0.1:4111/", token: "secret", fetch: mock.fetch });
  const result = await client.createMemory({ source: "orbit", kind: "note", content: "测试", content_format: "plain" });
  assert.equal(createProtocolBaseUrl("http://localhost/"), "http://localhost/v1");
  assert.equal(result.id, "memory-1");
  assert.equal(mock.calls[0].input, "http://127.0.0.1:4111/v1/memories");
  assert.equal(mock.calls[0].init.headers.get("authorization"), "Bearer secret");
});

/** 验证服务端 JSON 错误被映射为包含状态码的专用错误。 */
test("映射服务端错误", async () => {
  const mock = createFetch(() => Response.json({ error: "当前令牌无权执行此操作" }, { status: 403 }));
  const client = new ProtocolClient({ endpoint: "http://localhost", fetch: mock.fetch });
  await assert.rejects(client.getMemory("memory-1"), (error) => {
    assert.ok(error instanceof ProtocolClientError);
    assert.equal(error.status, 403);
    return true;
  });
});

/** 验证客户端解析 SSE 事件数据并遵循事件类型过滤查询参数。 */
test("订阅并解析 SSE 事件", async () => {
  const encoder = new TextEncoder();
  const stream = new ReadableStream({
    start(controller) {
      controller.enqueue(encoder.encode("event: memory.created\ndata: {\"type\":\"memory_created\",\"id\":\"memory-1\",\"source\":\"orbit\"}\n\n"));
      controller.close();
    },
  });
  const mock = createFetch(() => new Response(stream, { headers: { "content-type": "text/event-stream" } }));
  const controller = new AbortController();
  const client = new ProtocolClient({ endpoint: "http://localhost", token: "secret", fetch: mock.fetch });
  const iterator = client.subscribeEvents({ types: ["memory.created"], signal: controller.signal, reconnect_delay_ms: 0 });
  const event = await iterator.next();
  assert.deepEqual(event.value, { type: "memory_created", id: "memory-1", source: "orbit" });
  controller.abort();
  assert.equal((await iterator.next()).done, true);
  assert.equal(mock.calls[0].input, "http://localhost/v1/events?types=memory.created");
});
