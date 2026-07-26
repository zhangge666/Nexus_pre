# @nexus/sdk-ts

Nexus Memory Protocol 的公开 TypeScript SDK。先在 Orbit「连接与隐私」中创建第三方授权，再使用一次性展示的令牌：

```ts
import { NexusClient } from "@nexus/sdk-ts";

const nexus = new NexusClient({
  endpoint: "http://127.0.0.1:4111",
  token: process.env.NEXUS_TOKEN!,
  source: "external:my-app",
});

await nexus.addMemory({ content: "# 一条记忆", tags: ["example"] });
const hits = await nexus.searchMemory({ text: "记忆", mode: "hybrid" });
```
