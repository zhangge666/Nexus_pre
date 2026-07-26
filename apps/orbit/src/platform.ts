/** 本文件集中判断 Orbit 当前构建目标，避免平台条件散落到共享页面。 */

/** 返回当前是否运行 Android 专用外壳。 */
export function isAndroidPlatform(): boolean {
  return __NEXUS_PLATFORM__ === "android";
}
