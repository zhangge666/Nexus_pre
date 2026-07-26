/** 本文件读取当前页面的最小必要内容，并通过 Native Messaging 交给本地剪藏宿主。 */

const titleInput = document.querySelector("#title");
const tagsInput = document.querySelector("#tags");
const selectionOnlyInput = document.querySelector("#selection-only");
const statusNode = document.querySelector("#status");
const clipButton = document.querySelector("#clip");
let page = null;

/** 在当前标签页中提取标题、地址、选中文本和有界正文。 */
async function readCurrentPage() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab?.id) throw new Error("没有可剪藏的活动页面");
  const [result] = await chrome.scripting.executeScript({
    target: { tabId: tab.id },
    func: () => ({
      title: document.title,
      url: location.href,
      selection: getSelection()?.toString().trim() ?? "",
      pageText: document.body?.innerText?.replace(/\s+\n/g, "\n").trim().slice(0, 50_000) ?? "",
    }),
  });
  if (!result?.result) throw new Error("无法读取当前页面");
  return result.result;
}

/** 更新可访问状态文本和错误色。 */
function setStatus(message, isError = false) {
  statusNode.textContent = message;
  statusNode.classList.toggle("error", isError);
}

/** 将页面快照发送到本机宿主，令牌始终留在浏览器扩展之外。 */
async function clipPage() {
  if (!page) return;
  clipButton.disabled = true;
  setStatus("正在写入本地记忆库…");
  const tags = tagsInput.value.split(",").map((tag) => tag.trim()).filter(Boolean);
  try {
    const response = await chrome.runtime.sendNativeMessage("com.nexus.clipper", {
      action: "clip",
      title: titleInput.value.trim() || page.title,
      url: page.url,
      selection: page.selection,
      pageText: selectionOnlyInput.checked && page.selection ? "" : page.pageText,
      tags,
    });
    if (!response?.ok) throw new Error(response?.error || "本机宿主未返回成功状态");
    setStatus(`已保存 · ${response.id}`);
    clipButton.textContent = "已剪藏";
  } catch (error) {
    setStatus(`剪藏失败：${error instanceof Error ? error.message : String(error)}`, true);
    clipButton.disabled = false;
  }
}

/** 初始化弹窗并只在页面内容准备完成后开放提交。 */
async function initialize() {
  try {
    page = await readCurrentPage();
    titleInput.value = page.title;
    selectionOnlyInput.checked = Boolean(page.selection);
    setStatus(page.selection ? `已选中 ${page.selection.length} 个字符` : "将保存当前页面正文与链接");
    clipButton.disabled = false;
  } catch (error) {
    setStatus(error instanceof Error ? error.message : String(error), true);
  }
}

clipButton.addEventListener("click", () => void clipPage());
void initialize();
