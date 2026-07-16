/** 本文件定义 Memory Protocol v1 的类型安全 TypeScript 客户端、错误模型和 SSE 订阅能力。 */

/** 支持注入的 Fetch 实现，便于浏览器、Tauri 与测试环境复用客户端。 */
export type FetchLike = typeof fetch;

/** 表示协议请求失败时返回的稳定错误信息。 */
export class ProtocolClientError extends Error {
  /** 创建包含 HTTP 状态、服务端消息和可选响应正文的错误。 */
  public constructor(
    message: string,
    public readonly status?: number,
    public readonly body?: unknown,
  ) {
    super(message);
    this.name = "ProtocolClientError";
  }
}

/** Memory Protocol 服务端的能力发现响应。 */
export interface Capabilities {
  version: string;
  capabilities: string[];
  scopes: string[];
}

/** 统一记忆来源。 */
export type MemorySource = "echo" | "muse" | "quill" | "orbit" | `external:${string}`;

/** 统一记忆类别。 */
export type MemoryKind = "screen" | "note" | "idea" | "voice" | "card" | "clip" | "file";

/** 正文编码格式。 */
export type ContentFormat = "markdown" | "plain" | "json";

/** 记忆块响应结构。 */
export interface MemoryBlock {
  id: string;
  seq: number;
  type: string;
  text: string;
}

/** 完整记忆协议结构。 */
export interface Memory {
  id: string;
  source: MemorySource;
  kind: MemoryKind;
  title: string | null;
  content: string;
  content_format: ContentFormat;
  blocks: MemoryBlock[];
  tags: string[];
  pinned: boolean;
  archived: boolean;
  created_at: number;
  updated_at: number;
  captured_at: number | null;
  device_id: string;
  meta: Record<string, unknown>;
}

/** 创建记忆请求。 */
export interface CreateMemoryInput {
  source: MemorySource;
  kind: MemoryKind;
  title?: string;
  content: string;
  content_format: ContentFormat;
  tags?: string[];
  captured_at?: number;
  device_id?: string;
  meta?: Record<string, unknown>;
}

/** 更新记忆请求。 */
export interface UpdateMemoryInput {
  title?: string | null;
  content?: string;
  content_format?: ContentFormat;
  tags?: string[];
  pinned?: boolean;
  archived?: boolean;
  captured_at?: number;
  meta?: Record<string, unknown>;
}

/** 记忆列表过滤条件。 */
export interface MemoryFilters {
  source?: MemorySource[];
  kind?: MemoryKind[];
  tags?: string[];
  created_from?: number;
  created_to?: number;
}

/** 记忆分页结果。 */
export interface MemoryPage {
  items: Memory[];
  total: number;
  next_offset: number | null;
}

/** 检索模式。 */
export type SearchMode = "semantic" | "keyword" | "hybrid";

/** 块级检索命中。 */
export interface SearchHit {
  memory_id: string;
  block_id: string;
  score: number;
  snippet: string;
}

/** 检索请求。 */
export interface SearchInput {
  text: string;
  mode?: SearchMode;
  filters?: MemoryFilters;
  limit?: number;
}

/** 记忆关联类型。 */
export type LinkRelation = "references" | "derived_from" | "related" | "duplicate";

/** 关联创建主体。 */
export type LinkCreator = "user" | "ai" | "system";

/** 记忆关联结构。 */
export interface MemoryLink {
  from_id: string;
  to_id: string;
  relation: LinkRelation;
  created_by: LinkCreator;
  created_at: number;
}

/** 创建关联请求。 */
export interface CreateLinkInput {
  from_id: string;
  to_id: string;
  relation: LinkRelation;
  created_by?: LinkCreator;
}

/** 集合结构。 */
export interface Collection {
  id: string;
  name: string;
  icon: string | null;
  parent_id: string | null;
  sort: number;
  created_at: number;
  updated_at: number;
}

/** 创建集合请求。 */
export interface CreateCollectionInput {
  name: string;
  icon?: string;
  parent_id?: string;
  sort?: number;
}

/** 更新集合请求。 */
export interface UpdateCollectionInput {
  name?: string;
  icon?: string;
  clear_icon?: boolean;
  parent_id?: string;
  move_to_root?: boolean;
  sort?: number;
}

/** 第一方本地应用申请 capability token 的登记请求。 */
export interface RegisterConnectionInput {
  app_id: string;
  name: string;
  source: MemorySource;
  scopes: string[];
}

/** 本地应用登记成功后返回的来源受限授权。 */
export interface RegisteredConnection {
  tokenId: string;
  token: string;
  scopes: string[];
  source: MemorySource;
}

/** 管理端展示的已连接应用摘要。 */
export interface ConnectedApp {
  id: string;
  name: string;
  source: MemorySource;
  scopes: string[];
  lastActiveAt: number;
  memoriesCount: number;
  tokenId: string;
}

/** 订阅到的 Memory Protocol 事件。 */
export interface ProtocolEvent {
  type: "memory_created" | "memory_updated" | "memory_deleted";
  id: string;
  source: MemorySource;
}

/** SSE 订阅选项。 */
export interface EventSubscriptionOptions {
  types?: Array<"memory.created" | "memory.updated" | "memory.deleted">;
  signal?: AbortSignal;
  reconnect_delay_ms?: number;
}

/** 客户端配置。 */
export interface ProtocolClientOptions {
  endpoint: string;
  token?: string;
  fetch?: FetchLike;
}

/** 将服务地址规范化为 Memory Protocol v1 根地址。 */
export function createProtocolBaseUrl(endpoint: string): string {
  return `${endpoint.replace(/\/+$/, "")}/v1`;
}

/** 面向本地或远程 Memory Protocol v1 服务的类型安全客户端。 */
export class ProtocolClient {
  private readonly baseUrl: string;
  private readonly fetchImplementation: FetchLike;

  /** 使用服务端点、可选 capability token 与自定义 Fetch 构造客户端。 */
  public constructor(private readonly options: ProtocolClientOptions) {
    this.baseUrl = createProtocolBaseUrl(options.endpoint);
    this.fetchImplementation = options.fetch ?? globalThis.fetch;
    if (!this.fetchImplementation) {
      throw new ProtocolClientError("当前运行环境不提供 Fetch 实现");
    }
  }

  /** 读取服务端版本、能力和支持的 scope。 */
  public capabilities(): Promise<Capabilities> {
    return this.request<Capabilities>("/capabilities", { method: "GET" }, false);
  }

  /** 创建记忆并返回服务端生成的 ID 与时间戳。 */
  public createMemory(input: CreateMemoryInput): Promise<{ id: string; created_at: number }> {
    return this.request("/memories", { method: "POST", body: input });
  }

  /** 读取单条完整记忆。 */
  public getMemory(id: string): Promise<Memory> {
    return this.request(`/memories/${encodeURIComponent(id)}`, { method: "GET" });
  }

  /** 应用记忆字段补丁并返回更新后的数据。 */
  public updateMemory(id: string, input: UpdateMemoryInput): Promise<Memory> {
    return this.request(`/memories/${encodeURIComponent(id)}`, { method: "PATCH", body: input });
  }

  /** 删除记忆及服务端关联的索引数据。 */
  public deleteMemory(id: string): Promise<void> {
    return this.request(`/memories/${encodeURIComponent(id)}`, { method: "DELETE" });
  }

  /** 按服务端支持的过滤条件分页读取记忆。 */
  public listMemories(filters: MemoryFilters = {}, limit = 20, offset = 0): Promise<MemoryPage> {
    const query = new URLSearchParams({ limit: String(limit), offset: String(offset) });
    if (filters.source?.length) query.set("source", filters.source.join(","));
    if (filters.kind?.length) query.set("kind", filters.kind.join(","));
    if (filters.tags?.length) query.set("tags", filters.tags.join(","));
    if (filters.created_from !== undefined) query.set("created_from", String(filters.created_from));
    if (filters.created_to !== undefined) query.set("created_to", String(filters.created_to));
    return this.request(`/memories?${query.toString()}`, { method: "GET" });
  }

  /** 执行关键词、语义或混合检索。 */
  public async search(input: SearchInput): Promise<SearchHit[]> {
    const response = await this.request<{ hits: SearchHit[] }>("/search", {
      method: "POST",
      body: input,
    });
    return response.hits;
  }

  /** 创建一条有向记忆关联。 */
  public createLink(input: CreateLinkInput): Promise<MemoryLink> {
    return this.request("/links", { method: "POST", body: input });
  }

  /** 返回指定记忆作为任一端参与的关联。 */
  public listLinks(memoryId: string): Promise<MemoryLink[]> {
    return this.request(`/links?memory_id=${encodeURIComponent(memoryId)}`, { method: "GET" });
  }

  /** 删除由源、目标和关系唯一确定的关联。 */
  public deleteLink(fromId: string, toId: string, relation: LinkRelation): Promise<void> {
    return this.request(
      `/links/${encodeURIComponent(fromId)}/${encodeURIComponent(toId)}/${encodeURIComponent(relation)}`,
      { method: "DELETE" },
    );
  }

  /** 创建集合。 */
  public createCollection(input: CreateCollectionInput): Promise<Collection> {
    return this.request("/collections", { method: "POST", body: input });
  }

  /** 返回全部集合。 */
  public listCollections(): Promise<Collection[]> {
    return this.request("/collections", { method: "GET" });
  }

  /** 读取单个集合。 */
  public getCollection(id: string): Promise<Collection> {
    return this.request(`/collections/${encodeURIComponent(id)}`, { method: "GET" });
  }

  /** 更新集合名称、图标、层级或排序。 */
  public updateCollection(id: string, input: UpdateCollectionInput): Promise<Collection> {
    return this.request(`/collections/${encodeURIComponent(id)}`, { method: "PATCH", body: input });
  }

  /** 删除集合及其成员关系。 */
  public deleteCollection(id: string): Promise<void> {
    return this.request(`/collections/${encodeURIComponent(id)}`, { method: "DELETE" });
  }

  /** 幂等地将记忆加入集合。 */
  public addMemoryToCollection(collectionId: string, memoryId: string): Promise<void> {
    return this.request(
      `/collections/${encodeURIComponent(collectionId)}/memories/${encodeURIComponent(memoryId)}`,
      { method: "PUT" },
    );
  }

  /** 从集合移除记忆。 */
  public removeMemoryFromCollection(collectionId: string, memoryId: string): Promise<void> {
    return this.request(
      `/collections/${encodeURIComponent(collectionId)}/memories/${encodeURIComponent(memoryId)}`,
      { method: "DELETE" },
    );
  }

  /** 返回集合成员记忆 ID。 */
  public listCollectionMemoryIds(collectionId: string): Promise<string[]> {
    return this.request(`/collections/${encodeURIComponent(collectionId)}/memories`, { method: "GET" });
  }

  /** 使用本地持有者凭据登记第一方应用的最小授权。 */
  public registerConnection(input: RegisterConnectionInput): Promise<RegisteredConnection> {
    return this.request("/connections", { method: "POST", body: input });
  }

  /** 返回管理令牌可见的全部已授权本地应用。 */
  public listConnections(): Promise<ConnectedApp[]> {
    return this.request("/connections", { method: "GET" });
  }

  /** 撤销指定 capability token。 */
  public revokeConnection(tokenId: string): Promise<void> {
    return this.request(`/connections/${encodeURIComponent(tokenId)}`, { method: "DELETE" });
  }

  /** 持续解析 SSE 事件；连接中断时按固定延迟重连，取消信号到达后结束迭代。 */
  public async *subscribeEvents(options: EventSubscriptionOptions = {}): AsyncGenerator<ProtocolEvent> {
    const query = new URLSearchParams();
    if (options.types?.length) query.set("types", options.types.join(","));
    const path = `/events${query.size ? `?${query}` : ""}`;
    const reconnectDelay = options.reconnect_delay_ms ?? 1_000;
    while (!options.signal?.aborted) {
      try {
        const response = await this.fetchImplementation(`${this.baseUrl}${path}`, {
          method: "GET",
          headers: this.headers({ Accept: "text/event-stream" }),
          signal: options.signal,
        });
        if (!response.ok || !response.body) {
          throw await this.toError(response);
        }
        for await (const event of parseSseEvents(response.body, options.signal)) {
          yield event as ProtocolEvent;
        }
      } catch (error) {
        if (options.signal?.aborted) return;
        if (error instanceof ProtocolClientError && error.status && error.status < 500) throw error;
      }
      await wait(reconnectDelay, options.signal);
    }
  }

  /** 发送协议请求、统一附加认证头并将失败映射为客户端错误。 */
  private async request<T>(
    path: string,
    init: { method: string; body?: unknown },
    authenticated = true,
  ): Promise<T> {
    const response = await this.fetchImplementation(`${this.baseUrl}${path}`, {
      method: init.method,
      headers: this.headers(init.body === undefined ? undefined : { "Content-Type": "application/json" }, authenticated),
      body: init.body === undefined ? undefined : JSON.stringify(init.body),
    });
    if (!response.ok) throw await this.toError(response);
    if (response.status === 204) return undefined as T;
    return (await response.json()) as T;
  }

  /** 构造协议公共头，并仅在需要时发送 Bearer 令牌。 */
  private headers(extra: HeadersInit = {}, authenticated = true): Headers {
    const headers = new Headers(extra);
    if (authenticated && this.options.token) headers.set("Authorization", `Bearer ${this.options.token}`);
    return headers;
  }

  /** 尽可能解析服务端 JSON 错误，保留状态码供调用方做恢复决策。 */
  private async toError(response: Response): Promise<ProtocolClientError> {
    const contentType = response.headers.get("content-type") ?? "";
    const body: unknown = contentType.includes("application/json")
      ? await response.json().catch(() => undefined)
      : await response.text().catch(() => undefined);
    const message = typeof body === "object" && body !== null && "error" in body
      ? String((body as { error: unknown }).error)
      : `Memory Protocol 请求失败: ${response.status}`;
    return new ProtocolClientError(message, response.status, body);
  }
}

/** 逐帧解析 SSE 数据，仅产出具有 JSON data 字段的服务端事件。 */
async function* parseSseEvents(
  stream: ReadableStream<Uint8Array>,
  signal?: AbortSignal,
): AsyncGenerator<unknown> {
  const reader = stream.pipeThrough(new TextDecoderStream()).getReader();
  let buffer = "";
  try {
    while (!signal?.aborted) {
      const { value, done } = await reader.read();
      if (done) return;
      buffer += value;
      const frames = buffer.split(/\r?\n\r?\n/);
      buffer = frames.pop() ?? "";
      for (const frame of frames) {
        const data = frame
          .split(/\r?\n/)
          .filter((line) => line.startsWith("data:"))
          .map((line) => line.slice(5).trimStart())
          .join("\n");
        if (data) yield JSON.parse(data) as unknown;
      }
    }
  } finally {
    reader.releaseLock();
  }
}

/** 在重连间隔中响应取消信号，避免订阅关闭后继续等待。 */
function wait(milliseconds: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    const timeout = setTimeout(resolve, milliseconds);
    signal?.addEventListener("abort", () => {
      clearTimeout(timeout);
      resolve();
    }, { once: true });
  });
}
