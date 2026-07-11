/** 本文件定义前端访问 Memory Protocol v1 的基础端点规则。 */

/** 将服务地址规范化为 Memory Protocol v1 根地址。 */
export function createProtocolBaseUrl(endpoint: string): string {
  return `${endpoint.replace(/\/$/, "")}/v1`;
}

