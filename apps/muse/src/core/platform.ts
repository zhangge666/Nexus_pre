/** 本文件集中判断 Muse WebView 所在的平台，以便按系统约定呈现窗口控件。 */

/** 判断当前 WebView 是否运行在 macOS 上。 */
export function isMacOS(): boolean {
  return /Macintosh|Mac OS X/.test(navigator.userAgent);
}
