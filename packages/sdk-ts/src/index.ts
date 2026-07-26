/** 本文件提供面向第三方应用的公开 TypeScript SDK，并把写入来源固定在授权来源内。 */

import {
  ProtocolClient,
  ProtocolClientError,
  createProtocolBaseUrl,
  type AskInput,
  type AskResult,
  type ContentFormat,
  type CreateMemoryInput,
  type FetchLike,
  type Memory,
  type MemoryKind,
  type MemorySource,
  type SearchHit,
  type SearchInput,
  type UpdateMemoryInput,
} from "@nexus/protocol-client";

export {
  ProtocolClient,
  ProtocolClientError,
  createProtocolBaseUrl,
};
export type * from "@nexus/protocol-client";

/** 公开 SDK 的连接配置。 */
export interface NexusClientOptions {
  endpoint: string;
  token: string;
  source: `external:${string}`;
  fetch?: FetchLike;
}

/** 固定来源写入的便捷参数，防止调用方意外伪装成其他应用。 */
export interface AddMemoryInput {
  content: string;
  kind?: MemoryKind;
  title?: string;
  content_format?: ContentFormat;
  tags?: string[];
  captured_at?: number;
  device_id?: string;
  meta?: Record<string, unknown>;
}

/** 面向脚本、Node 服务和浏览器集成的 Nexus 高层客户端。 */
export class NexusClient {
  public readonly protocol: ProtocolClient;
  public readonly source: `external:${string}`;

  /** 使用 Orbit 生成的 capability token 构造来源受限客户端。 */
  public constructor(options: NexusClientOptions) {
    if (!/^external:[a-z0-9][a-z0-9._-]{0,79}$/.test(options.source)) {
      throw new ProtocolClientError("source 必须是合法的 external:<app_id>");
    }
    this.source = options.source;
    this.protocol = new ProtocolClient(options);
  }

  /** 写入一条属于当前授权来源的记忆。 */
  public addMemory(input: AddMemoryInput): Promise<{ id: string; created_at: number }> {
    const request: CreateMemoryInput = {
      source: this.source,
      kind: input.kind ?? "note",
      content: input.content,
      content_format: input.content_format ?? "markdown",
      title: input.title,
      tags: input.tags,
      captured_at: input.captured_at,
      device_id: input.device_id,
      meta: input.meta,
    };
    return this.protocol.createMemory(request);
  }

  /** 检索授权可见范围内的记忆。 */
  public searchMemory(input: SearchInput): Promise<SearchHit[]> {
    return this.protocol.search(input);
  }

  /** 读取一条完整记忆。 */
  public getMemory(id: string): Promise<Memory> {
    return this.protocol.getMemory(id);
  }

  /** 更新当前来源拥有的记忆。 */
  public updateMemory(id: string, input: UpdateMemoryInput): Promise<Memory> {
    return this.protocol.updateMemory(id, input);
  }

  /** 删除当前来源拥有的记忆。 */
  public deleteMemory(id: string): Promise<void> {
    return this.protocol.deleteMemory(id);
  }

  /** 对授权可见范围执行带引用问答。 */
  public askMemory(input: AskInput): Promise<AskResult> {
    return this.protocol.ask(input);
  }
}

/** 将任意协议来源收窄为公开 SDK 允许的第三方来源。 */
export function assertExternalSource(source: MemorySource): asserts source is `external:${string}` {
  if (!source.startsWith("external:")) {
    throw new ProtocolClientError("公开 SDK 只能使用 external:* 来源");
  }
}
