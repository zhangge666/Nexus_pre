/** 本文件定义软件族共享的应用标识及其类型。 */

/** Nexus 软件族中可独立发布的应用标识。 */
export type NexusApp = "echo" | "muse" | "quill" | "orbit";

/** 返回当前软件族包含的全部应用。 */
export function listNexusApps(): readonly NexusApp[] {
  return ["echo", "muse", "quill", "orbit"];
}

