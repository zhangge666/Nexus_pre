#!/usr/bin/env node
/** 本文件实现基于标准输入输出 JSON-RPC 的 Nexus MCP Server。 */

import { createInterface } from "node:readline";
import { pathToFileURL } from "node:url";
import { NexusClient } from "@nexus/sdk-ts";

const SERVER_INFO = { name: "nexus-memory", version: "0.1.0" };

/** MCP 暴露的工具定义；每个工具只声明完成任务所需的最小参数。 */
export const TOOLS = [
  {
    name: "search_memory",
    description: "在 Nexus 长期记忆中执行混合检索，返回相关块及记忆标识。",
    inputSchema: {
      type: "object",
      properties: {
        query: { type: "string", description: "自然语言检索词" },
        limit: { type: "integer", minimum: 1, maximum: 50, default: 10 },
        source: { type: "string", description: "可选的精确来源过滤器" },
      },
      required: ["query"],
      additionalProperties: false,
    },
  },
  {
    name: "add_memory",
    description: "把值得长期保留的内容写入 Nexus，并标记为当前 MCP 来源。",
    inputSchema: {
      type: "object",
      properties: {
        content: { type: "string", description: "Markdown 或纯文本正文" },
        title: { type: "string" },
        tags: { type: "array", items: { type: "string" }, maxItems: 30 },
      },
      required: ["content"],
      additionalProperties: false,
    },
  },
  {
    name: "get_memory",
    description: "按记忆 ID 读取完整正文和来源元数据。",
    inputSchema: {
      type: "object",
      properties: { id: { type: "string" } },
      required: ["id"],
      additionalProperties: false,
    },
  },
  {
    name: "ask_memory",
    description: "基于 Nexus 本地检索执行带块级引用的问答。",
    inputSchema: {
      type: "object",
      properties: { question: { type: "string" } },
      required: ["question"],
      additionalProperties: false,
    },
  },
];

/** 将工具结果包装成 MCP 文本内容，保留结构化 JSON 供客户端解析。 */
function toolResult(value) {
  return {
    content: [{ type: "text", text: JSON.stringify(value, null, 2) }],
    structuredContent: value,
  };
}

/** 校验字符串参数并给出可直接展示给 MCP 客户端的错误。 */
function requiredString(args, key) {
  const value = args?.[key];
  if (typeof value !== "string" || value.trim() === "") {
    throw new Error(`${key} 必须是非空字符串`);
  }
  return value.trim();
}

/** 执行单个 MCP 工具调用。 */
export async function callTool(client, name, args = {}) {
  switch (name) {
    case "search_memory": {
      const source = typeof args.source === "string" && args.source.trim()
        ? [args.source.trim()]
        : undefined;
      const limit = Number.isInteger(args.limit) ? Math.min(50, Math.max(1, args.limit)) : 10;
      return toolResult(await client.searchMemory({
        text: requiredString(args, "query"),
        mode: "hybrid",
        filters: source ? { source } : undefined,
        limit,
      }));
    }
    case "add_memory":
      return toolResult(await client.addMemory({
        content: requiredString(args, "content"),
        title: typeof args.title === "string" ? args.title.trim() : undefined,
        tags: Array.isArray(args.tags) ? args.tags.filter((tag) => typeof tag === "string") : undefined,
        meta: { integration: "mcp" },
      }));
    case "get_memory":
      return toolResult(await client.getMemory(requiredString(args, "id")));
    case "ask_memory":
      return toolResult(await client.askMemory({ question: requiredString(args, "question") }));
    default:
      throw new Error(`未知工具: ${name}`);
  }
}

/** 处理一条 MCP JSON-RPC 消息；通知消息返回 null。 */
export async function handleRequest(client, message) {
  if (message.method === "notifications/initialized") return null;
  if (message.method === "ping") {
    return { jsonrpc: "2.0", id: message.id, result: {} };
  }
  if (message.method === "initialize") {
    return {
      jsonrpc: "2.0",
      id: message.id,
      result: {
        protocolVersion: message.params?.protocolVersion ?? "2024-11-05",
        capabilities: { tools: { listChanged: false } },
        serverInfo: SERVER_INFO,
      },
    };
  }
  if (message.method === "tools/list") {
    return { jsonrpc: "2.0", id: message.id, result: { tools: TOOLS } };
  }
  if (message.method === "tools/call") {
    try {
      const result = await callTool(client, message.params?.name, message.params?.arguments);
      return { jsonrpc: "2.0", id: message.id, result };
    } catch (error) {
      return {
        jsonrpc: "2.0",
        id: message.id,
        result: {
          isError: true,
          content: [{ type: "text", text: error instanceof Error ? error.message : String(error) }],
        },
      };
    }
  }
  return {
    jsonrpc: "2.0",
    id: message.id ?? null,
    error: { code: -32601, message: `不支持的方法: ${message.method}` },
  };
}

/** 从环境变量创建来源受限的 Nexus SDK 客户端。 */
function createClientFromEnvironment() {
  const token = process.env.NEXUS_TOKEN?.trim();
  if (!token) throw new Error("缺少 NEXUS_TOKEN");
  return new NexusClient({
    endpoint: process.env.NEXUS_ENDPOINT?.trim() || "http://127.0.0.1:4111",
    token,
    source: process.env.NEXUS_SOURCE?.trim() || "external:mcp",
  });
}

/** 启动逐行 JSON-RPC stdio 循环，绝不向标准输出写入协议之外的日志。 */
export async function main() {
  const client = createClientFromEnvironment();
  const input = createInterface({ input: process.stdin, crlfDelay: Infinity });
  for await (const line of input) {
    if (!line.trim()) continue;
    let message;
    try {
      message = JSON.parse(line);
    } catch {
      process.stdout.write(`${JSON.stringify({
        jsonrpc: "2.0",
        id: null,
        error: { code: -32700, message: "JSON 解析失败" },
      })}\n`);
      continue;
    }
    const response = await handleRequest(client, message);
    if (response) process.stdout.write(`${JSON.stringify(response)}\n`);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`Nexus MCP 启动失败：${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
