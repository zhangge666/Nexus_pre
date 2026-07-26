/** 本文件组合 Muse 自定义桌面壳、独立页面与可选 Orbit 同步能力。 */

import React, { useEffect, useMemo, useState } from "react";
import { Sidebar } from "./components/Sidebar";
import { Titlebar } from "./components/Titlebar";
import { isTauriRuntime, connectService, getConnectionStatus, submitIdea, type ConnectionStatus } from "./api";
import type { MuseView } from "./core/types";
import { useMuseWorkspace } from "./core/workspace";
import { ClipboardPage } from "./pages/ClipboardPage";
import { IdeasPage } from "./pages/IdeasPage";
import { MeetingsPage } from "./pages/MeetingsPage";
import { SettingsPage } from "./pages/SettingsPage";
import { TasksPage } from "./pages/TasksPage";
import { TodayPage } from "./pages/TodayPage";
import "./muse.css";

const initialConnection: ConnectionStatus = {
  state: "disconnected",
  endpoint: null,
  message: "Orbit 是可选连接；Muse 当前以本地模式运行。",
};

/** 把未知 IPC 错误转换为可直接展示的消息。 */
function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** 渲染 Muse 可独立运行的多页面桌面应用。 */
export function App(): React.JSX.Element {
  const [activeView, setActiveView] = useState<MuseView>("today");
  const [connection, setConnection] = useState<ConnectionStatus>(initialConnection);
  const [connecting, setConnecting] = useState(false);
  const {
    workspace,
    addIdea,
    updateIdeaSync,
    addTask,
    setTaskStatus,
    addTaskActivity,
    toggleClipboardPin,
    clearUnpinnedClipboard,
  } = useMuseWorkspace();

  useEffect(() => {
    if (!isTauriRuntime()) return;
    void getConnectionStatus()
      .then((status) => {
        setConnection(
          status.state === "connected"
            ? status
            : { ...status, message: "Orbit 未连接；Muse 仍会把内容保存在本机。" },
        );
      })
      .catch((error) => {
        setConnection({ ...initialConnection, message: errorMessage(error) });
      });
  }, []);

  /** 注册应用内快捷键预览；系统级全局热键由后续 Tauri 阶段接入。 */
  useEffect(() => {
    const mappings: Record<string, MuseView> = { i: "ideas", t: "tasks", r: "meetings", v: "clipboard" };
    const listener = (event: KeyboardEvent) => {
      if (!event.ctrlKey || !event.shiftKey) return;
      const view = mappings[event.key.toLowerCase()];
      if (!view) return;
      event.preventDefault();
      setActiveView(view);
    };
    window.addEventListener("keydown", listener);
    return () => window.removeEventListener("keydown", listener);
  }, []);

  /** 连接可选 Orbit 服务，失败时不影响本机数据。 */
  async function handleConnect(): Promise<void> {
    if (!isTauriRuntime()) {
      setConnection({ ...initialConnection, message: "浏览器预览没有 Tauri IPC；桌面壳中才能连接 Orbit。" });
      return;
    }
    setConnecting(true);
    try {
      setConnection(await connectService());
    } catch (error) {
      setConnection({ ...initialConnection, message: `${errorMessage(error)}；Muse 已继续使用本地模式。` });
    } finally {
      setConnecting(false);
    }
  }

  /** 先可靠保存到本机；连接 Orbit 时再执行不阻塞单机使用的可选同步。 */
  async function handleAddIdea(content: string): Promise<void> {
    const idea = addIdea(content);
    if (connection.state !== "connected" || !isTauriRuntime()) return;
    updateIdeaSync(idea.id, "syncing");
    try {
      await submitIdea(content);
      updateIdeaSync(idea.id, "synced");
    } catch (error) {
      updateIdeaSync(idea.id, "error");
      setConnection({ ...initialConnection, message: `${errorMessage(error)}；灵感已经保存在本机。` });
    }
  }

  const activeTaskCount = useMemo(
    () => workspace.tasks.filter((task) => task.status !== "done").length,
    [workspace.tasks],
  );

  /** 根据当前导航返回独立页面组件，避免把所有功能堆在单个页面文件中。 */
  function renderPage(): React.JSX.Element {
    if (activeView === "ideas") return <IdeasPage ideas={workspace.ideas} onAddIdea={handleAddIdea} />;
    if (activeView === "tasks") {
      return (
        <TasksPage
          tasks={workspace.tasks}
          onAddTask={addTask}
          onSetStatus={setTaskStatus}
          onAddActivity={addTaskActivity}
        />
      );
    }
    if (activeView === "meetings") return <MeetingsPage meetings={workspace.meetings} />;
    if (activeView === "clipboard") {
      return (
        <ClipboardPage
          items={workspace.clipboard}
          onTogglePin={toggleClipboardPin}
          onClearUnpinned={clearUnpinnedClipboard}
        />
      );
    }
    if (activeView === "settings") {
      return <SettingsPage connection={connection} connecting={connecting} onConnect={handleConnect} />;
    }
    return (
      <TodayPage
        ideas={workspace.ideas}
        tasks={workspace.tasks}
        onAddIdea={handleAddIdea}
        onNavigate={setActiveView}
      />
    );
  }

  return (
    <div className="muse-app">
      <Titlebar />
      <div className="app-body">
        <Sidebar activeView={activeView} taskCount={activeTaskCount} onNavigate={setActiveView} />
        <main className="app-workspace">{renderPage()}</main>
      </div>
    </div>
  );
}
