/** Muse 原型交互：负责界面切换、会议模式切换与基础键盘预览。 */

const availableViews = new Set(["launcher", "idea", "tasks", "meeting", "clipboard", "settings"]);
const shortcutViews = new Map([
  ["i", "idea"],
  ["t", "tasks"],
  ["r", "meeting"],
  ["v", "clipboard"],
]);

/** 从地址参数读取初始界面，非法值回退到快捷启动条。 */
function initialView() {
  const requested = new URLSearchParams(window.location.search).get("view");
  return requested && availableViews.has(requested) ? requested : "launcher";
}

/** 显示指定界面，并同步顶部预览导航和地址参数。 */
function showView(view, updateUrl = true) {
  if (!availableViews.has(view)) return;

  document.querySelectorAll("[data-view]").forEach((panel) => {
    panel.classList.toggle("is-visible", panel.dataset.view === view);
  });

  document.querySelectorAll(".preview-nav [data-switch]").forEach((button) => {
    button.classList.toggle("is-active", button.dataset.switch === view);
  });

  if (updateUrl) {
    const url = new URL(window.location.href);
    url.searchParams.set("view", view);
    window.history.replaceState({}, "", url);
  }
}

/** 在会议原型内切换实时转写与会后摘要。 */
function showMeetingMode(mode) {
  document.querySelectorAll("[data-meeting-mode]").forEach((button) => {
    button.classList.toggle("is-active", button.dataset.meetingMode === mode);
  });
  document.querySelectorAll("[data-meeting-panel]").forEach((panel) => {
    panel.classList.toggle("is-visible", panel.dataset.meetingPanel === mode);
  });
}

/** 绑定所有带 data-switch 的入口，使原型内的功能按钮也可直接跳转。 */
function bindViewSwitches() {
  document.querySelectorAll("[data-switch]").forEach((button) => {
    button.addEventListener("click", () => showView(button.dataset.switch));
  });
}

/** 绑定会议内部页签。 */
function bindMeetingModes() {
  document.querySelectorAll("[data-meeting-mode]").forEach((button) => {
    button.addEventListener("click", () => showMeetingMode(button.dataset.meetingMode));
  });
}

/** 提供原型层键盘切换，避免与输入框的真实输入冲突。 */
function bindKeyboardPreview() {
  document.addEventListener("keydown", (event) => {
    const target = event.target;
    const editing = target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;
    if (editing) return;

    if (event.key === "Escape") {
      showView("launcher");
      return;
    }

    const view = shortcutViews.get(event.key.toLowerCase());
    if (view && !event.ctrlKey && !event.metaKey && !event.altKey) {
      showView(view);
    }
  });
}

bindViewSwitches();
bindMeetingModes();
bindKeyboardPreview();
showView(initialView(), false);
