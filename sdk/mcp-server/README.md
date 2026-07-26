# Nexus MCP Server

先在 Orbit「连接与隐私」创建应用标识 `mcp`，授予 `memory:read`、`memory:write` 和 `search`，再把下列配置加入支持 MCP 的客户端：

```json
{
  "mcpServers": {
    "nexus": {
      "command": "npx",
      "args": ["-y", "@nexus/mcp-server"],
      "env": {
        "NEXUS_ENDPOINT": "http://127.0.0.1:4111",
        "NEXUS_TOKEN": "<Orbit 只展示一次的令牌>",
        "NEXUS_SOURCE": "external:mcp"
      }
    }
  }
}
```

提供 `search_memory`、`add_memory`、`get_memory`、`ask_memory` 四个工具。服务使用 stdio，不会把日志混入 JSON-RPC 输出。
