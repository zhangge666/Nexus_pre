/** 本文件实现 Muse 无需 Orbit 即可工作的浏览器/WebView 本地持久化数据层。 */

import { useEffect, useMemo, useState } from "react";
import type {
  MuseClipboardItem,
  MuseIdea,
  MuseMeeting,
  MuseTask,
  MuseWorkspace,
  SyncState,
  TaskActivity,
  TaskStatus,
} from "./types";

const STORAGE_KEY = "nexus.muse.workspace.v1";

/** 生成适合本地对象使用的稳定标识。 */
function createId(prefix: string): string {
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

/** 生成首启演示数据，帮助用户直接理解工作留痕与比较方式。 */
function createInitialWorkspace(): MuseWorkspace {
  const now = Date.now();
  return {
    ideas: [
      {
        id: "idea-welcome",
        content: "Muse 应该始终比当前工作更轻，不要为了记录而打断记录本身。",
        createdAt: now - 38 * 60_000,
        syncState: "local",
      },
    ],
    tasks: [
      {
        id: "task-release",
        title: "确认新版首页发布清单",
        description: "确保桌面端与移动端首屏文案、埋点和回滚方案全部就绪。",
        status: "doing",
        dueLabel: "今天 18:00",
        requester: "林然",
        source: "钉钉消息",
        project: "产品发布",
        activities: [
          {
            id: "activity-source",
            type: "source",
            title: "任务由钉钉消息建立",
            detail: "首页今天必须发出；确认移动端断点、按钮埋点和旧版回滚包。",
            createdAt: now - 5 * 60 * 60_000,
          },
          {
            id: "activity-file",
            type: "file",
            title: "添加 release-checklist.xlsx",
            detail: "12 项中已完成 9 项 · 文件版本 v3",
            createdAt: now - 3 * 60 * 60_000,
          },
          {
            id: "activity-progress",
            type: "note",
            title: "进展更新",
            detail: "移动端 390px 断点已修复，等待埋点在预发布环境回传。",
            createdAt: now - 48 * 60_000,
          },
        ],
      },
      {
        id: "task-feedback",
        title: "整理客户反馈并回复",
        description: "合并邮件与会议中的问题，回复确认处理顺序。",
        status: "todo",
        dueLabel: "明天 10:00",
        requester: "客户成功组",
        source: "Outlook 邮件",
        project: "客户反馈",
        activities: [],
      },
      {
        id: "task-invoice",
        title: "核对七月发票数据",
        description: "等待财务确认最终开票金额。",
        status: "waiting",
        dueLabel: "等待确认",
        requester: "财务",
        source: "企业微信",
        project: "财务",
        activities: [],
      },
    ],
    meetings: [
      {
        id: "meeting-weekly",
        title: "产品周会",
        summary: "新版首页按原计划今日发布；移动端断点已修复，按钮埋点仍需确认。",
        recordedAt: now - 24 * 60 * 60_000,
        durationLabel: "32:18",
        actionItems: ["确认按钮埋点回传", "发送发布检查结果"],
      },
    ],
    clipboard: [
      {
        id: "clip-a",
        title: "新版报价条目",
        content: "基础服务：￥12,800\n交付周期：10 个工作日\n数据迁移：包含 1 次\n驻场支持：2 天\n售后服务：3 个月\n付款方式：5 / 5",
        source: "Excel",
        copiedAt: now - 8 * 60_000,
        pinned: true,
      },
      {
        id: "clip-b",
        title: "客户确认版本",
        content: "基础服务：￥12,800\n交付周期：12 个工作日\n数据迁移：包含 1 次\n驻场支持：3 天\n售后服务：6 个月\n付款方式：5 / 5",
        source: "钉钉",
        copiedAt: now - 5 * 60_000,
        pinned: true,
      },
    ],
  };
}

/** 从本地存储恢复工作区；损坏数据回退为可用的初始状态。 */
function loadWorkspace(): MuseWorkspace {
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    return saved ? (JSON.parse(saved) as MuseWorkspace) : createInitialWorkspace();
  } catch {
    return createInitialWorkspace();
  }
}

/** 对外暴露 Muse 本机数据与最小编辑动作。 */
export function useMuseWorkspace() {
  const [workspace, setWorkspace] = useState<MuseWorkspace>(loadWorkspace);

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(workspace));
  }, [workspace]);

  /** 新增一条本地灵感，并返回新对象供可选同步使用。 */
  function addIdea(content: string): MuseIdea {
    const idea: MuseIdea = {
      id: createId("idea"),
      content: content.trim(),
      createdAt: Date.now(),
      syncState: "local",
    };
    setWorkspace((current) => ({ ...current, ideas: [idea, ...current.ideas] }));
    return idea;
  }

  /** 更新灵感的可选 Orbit 同步状态。 */
  function updateIdeaSync(id: string, syncState: SyncState): void {
    setWorkspace((current) => ({
      ...current,
      ideas: current.ideas.map((idea) => (idea.id === id ? { ...idea, syncState } : idea)),
    }));
  }

  /** 创建一个带首条来源活动的本地任务。 */
  function addTask(title: string, sourceText?: string): MuseTask {
    const activities: TaskActivity[] = sourceText?.trim()
      ? [
          {
            id: createId("activity"),
            type: "source",
            title: "创建任务时保存的来源",
            detail: sourceText.trim(),
            createdAt: Date.now(),
          },
        ]
      : [];
    const task: MuseTask = {
      id: createId("task"),
      title: title.trim(),
      description: "",
      status: "todo",
      dueLabel: "未设置",
      requester: "未填写",
      source: sourceText ? "手动粘贴" : "Muse",
      project: "未分类",
      activities,
    };
    setWorkspace((current) => ({ ...current, tasks: [task, ...current.tasks] }));
    return task;
  }

  /** 修改任务状态，同时追加不可覆盖的状态活动。 */
  function setTaskStatus(id: string, status: TaskStatus): void {
    setWorkspace((current) => ({
      ...current,
      tasks: current.tasks.map((task) => {
        if (task.id !== id) return task;
        const activity: TaskActivity = {
          id: createId("activity"),
          type: "status",
          title: status === "done" ? "任务已完成" : "任务状态已更新",
          detail: status === "done" ? "保留全部历史，可随时复开。" : `当前状态：${status}`,
          createdAt: Date.now(),
        };
        return { ...task, status, activities: [...task.activities, activity] };
      }),
    }));
  }

  /** 为任务追加工作进展，不覆盖既有留痕。 */
  function addTaskActivity(taskId: string, detail: string): void {
    const value = detail.trim();
    if (!value) return;
    setWorkspace((current) => ({
      ...current,
      tasks: current.tasks.map((task) =>
        task.id === taskId
          ? {
              ...task,
              activities: [
                ...task.activities,
                {
                  id: createId("activity"),
                  type: "note",
                  title: "进展更新",
                  detail: value,
                  createdAt: Date.now(),
                },
              ],
            }
          : task,
      ),
    }));
  }

  /** 钉住或取消钉住本地剪贴板条目。 */
  function toggleClipboardPin(id: string): void {
    setWorkspace((current) => ({
      ...current,
      clipboard: current.clipboard.map((item) => (item.id === id ? { ...item, pinned: !item.pinned } : item)),
    }));
  }

  /** 清理所有未钉住的临时剪贴板内容。 */
  function clearUnpinnedClipboard(): void {
    setWorkspace((current) => ({ ...current, clipboard: current.clipboard.filter((item) => item.pinned) }));
  }

  return useMemo(
    () => ({
      workspace,
      addIdea,
      updateIdeaSync,
      addTask,
      setTaskStatus,
      addTaskActivity,
      toggleClipboardPin,
      clearUnpinnedClipboard,
    }),
    [workspace],
  );
}
