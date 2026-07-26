"""本文件实现仅依赖 Python 标准库的 Memory Protocol v1 客户端。"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from typing import Any, Mapping
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode
from urllib.request import Request, urlopen


@dataclass(slots=True)
class NexusError(Exception):
    """表示包含 HTTP 状态和服务端响应的稳定 SDK 错误。"""

    message: str
    status: int | None = None
    body: Any = None

    def __str__(self) -> str:
        """返回适合 CLI 展示的错误文本。"""

        return self.message


class NexusClient:
    """面向 Python 脚本和数据管线的来源受限 Nexus 客户端。"""

    def __init__(
        self,
        endpoint: str,
        token: str,
        source: str,
        *,
        timeout: float = 15.0,
    ) -> None:
        """保存服务地址、capability token 和固定外部来源。"""

        if not re.fullmatch(r"external:[a-z0-9][a-z0-9._-]{0,79}", source):
            raise NexusError("source 必须是合法的 external:<app_id>")
        if not token.strip():
            raise NexusError("token 不能为空")
        self.base_url = f"{endpoint.rstrip('/')}/v1"
        self.token = token.strip()
        self.source = source
        self.timeout = timeout

    def capabilities(self) -> dict[str, Any]:
        """读取服务端版本、能力和可用 scope。"""

        return self._request("GET", "/capabilities", authenticated=False)

    def add_memory(
        self,
        content: str,
        *,
        title: str | None = None,
        kind: str = "note",
        content_format: str = "markdown",
        tags: list[str] | None = None,
        captured_at: int | None = None,
        device_id: str | None = None,
        meta: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """写入一条固定属于当前授权来源的记忆。"""

        payload: dict[str, Any] = {
            "source": self.source,
            "kind": kind,
            "content": content,
            "content_format": content_format,
        }
        optional = {
            "title": title,
            "tags": tags,
            "captured_at": captured_at,
            "device_id": device_id,
            "meta": dict(meta) if meta is not None else None,
        }
        payload.update({key: value for key, value in optional.items() if value is not None})
        return self._request("POST", "/memories", payload)

    def get_memory(self, memory_id: str) -> dict[str, Any]:
        """按 ID 读取完整记忆。"""

        return self._request("GET", f"/memories/{memory_id}")

    def update_memory(self, memory_id: str, **patch: Any) -> dict[str, Any]:
        """更新当前来源拥有的记忆字段。"""

        return self._request("PATCH", f"/memories/{memory_id}", patch)

    def delete_memory(self, memory_id: str) -> None:
        """删除当前来源拥有的记忆。"""

        self._request("DELETE", f"/memories/{memory_id}")

    def list_memories(
        self,
        *,
        limit: int = 20,
        offset: int = 0,
        source: list[str] | None = None,
        kind: list[str] | None = None,
        tags: list[str] | None = None,
    ) -> dict[str, Any]:
        """按来源、类别和标签分页读取记忆。"""

        query: dict[str, str | int] = {"limit": limit, "offset": offset}
        if source:
            query["source"] = ",".join(source)
        if kind:
            query["kind"] = ",".join(kind)
        if tags:
            query["tags"] = ",".join(tags)
        return self._request("GET", f"/memories?{urlencode(query)}")

    def search_memory(
        self,
        query: str,
        *,
        mode: str = "hybrid",
        limit: int = 10,
        filters: Mapping[str, Any] | None = None,
    ) -> list[dict[str, Any]]:
        """执行关键词、语义或混合检索并返回块级命中。"""

        response = self._request(
            "POST",
            "/search",
            {
                "text": query,
                "mode": mode,
                "limit": limit,
                "filters": dict(filters or {}),
            },
        )
        return list(response["hits"])

    def ask_memory(
        self,
        question: str,
        *,
        collection: str | None = None,
        source: str | None = None,
    ) -> dict[str, Any]:
        """执行带引用问答，并返回 Provider 数据流向元数据。"""

        scope = {
            key: value
            for key, value in {"collection": collection, "source": source}.items()
            if value is not None
        }
        payload: dict[str, Any] = {"question": question}
        if scope:
            payload["scope"] = scope
        return self._request("POST", "/ask", payload)

    def _request(
        self,
        method: str,
        path: str,
        payload: Mapping[str, Any] | None = None,
        *,
        authenticated: bool = True,
    ) -> Any:
        """发送 JSON 请求并统一映射网络、HTTP 和协议错误。"""

        data = None if payload is None else json.dumps(payload).encode("utf-8")
        headers = {"Accept": "application/json"}
        if data is not None:
            headers["Content-Type"] = "application/json"
        if authenticated:
            headers["Authorization"] = f"Bearer {self.token}"
        request = Request(
            f"{self.base_url}{path}",
            data=data,
            headers=headers,
            method=method,
        )
        try:
            with urlopen(request, timeout=self.timeout) as response:
                body = response.read()
                if not body:
                    return None
                return json.loads(body)
        except HTTPError as error:
            body_bytes = error.read()
            try:
                body = json.loads(body_bytes) if body_bytes else None
            except json.JSONDecodeError:
                body = body_bytes.decode("utf-8", errors="replace")
            message = body.get("error") if isinstance(body, dict) else None
            raise NexusError(message or f"Memory Protocol 请求失败: {error.code}", error.code, body) from error
        except URLError as error:
            raise NexusError(f"无法连接 Nexus 服务: {error.reason}") from error
