/** 本文件集中定义任务界面使用的稳定编号与展示映射。 */

/** 生成只用于界面识别的紧凑 Muse 任务编号。 */
export function taskCode(index: number): string {
  return `MUS-${24 - index}`;
}
