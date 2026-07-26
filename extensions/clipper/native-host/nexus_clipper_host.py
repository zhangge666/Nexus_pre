"""本文件实现 Chrome/Edge Native Messaging 与本地 Memory Protocol 之间的安全桥接。"""

from __future__ import annotations

import json
import os
import struct
import sys
from pathlib import Path
from typing import Any, BinaryIO
from urllib.error import HTTPError, URLError
from urllib.parse import urlparse
from urllib.request import Request, urlopen

MAX_MESSAGE_BYTES = 1_048_576


def default_config_path() -> Path:
    """返回当前平台的剪藏宿主配置路径。"""

    override = os.getenv("NEXUS_CLIPPER_CONFIG")
    if override:
        return Path(override)
    if os.name == "nt":
        return Path(os.getenv("APPDATA", Path.home())) / "Nexus" / "clipper.json"
    return Path(os.getenv("XDG_CONFIG_HOME", Path.home() / ".config")) / "nexus" / "clipper.json"


def load_config(path: Path | None = None) -> dict[str, str]:
    """读取不随浏览器页面暴露的服务地址和 capability token。"""

    config_path = path or default_config_path()
    data = json.loads(config_path.read_text(encoding="utf-8"))
    endpoint = str(data.get("endpoint", "")).rstrip("/")
    token = str(data.get("token", "")).strip()
    source = str(data.get("source", "external:clipper"))
    if not endpoint or not token or source != "external:clipper":
        raise ValueError("剪藏宿主配置缺少 endpoint/token 或来源不正确")
    return {"endpoint": endpoint, "token": token, "source": source}


def read_message(stream: BinaryIO) -> dict[str, Any] | None:
    """读取 Native Messaging 的小端长度前缀 JSON 消息。"""

    header = stream.read(4)
    if not header:
        return None
    if len(header) != 4:
        raise ValueError("Native Messaging 消息头不完整")
    length = struct.unpack("<I", header)[0]
    if length > MAX_MESSAGE_BYTES:
        raise ValueError("Native Messaging 消息超过 1 MiB 上限")
    payload = stream.read(length)
    if len(payload) != length:
        raise ValueError("Native Messaging 消息正文不完整")
    value = json.loads(payload.decode("utf-8"))
    if not isinstance(value, dict):
        raise ValueError("Native Messaging 消息必须是对象")
    return value


def write_message(stream: BinaryIO, message: dict[str, Any]) -> None:
    """写出 Native Messaging 的小端长度前缀 JSON 消息。"""

    payload = json.dumps(message, ensure_ascii=False).encode("utf-8")
    stream.write(struct.pack("<I", len(payload)))
    stream.write(payload)
    stream.flush()


def build_content(message: dict[str, Any]) -> tuple[str, str, list[str], dict[str, Any]]:
    """从浏览器页面快照构建有界 Markdown 正文和审计元数据。"""

    title = str(message.get("title", "")).strip()[:200] or "未命名网页"
    url = str(message.get("url", "")).strip()
    parsed = urlparse(url)
    if parsed.scheme not in {"http", "https"} or not parsed.netloc:
        raise ValueError("只允许剪藏 HTTP/HTTPS 页面")
    selection = str(message.get("selection", "")).strip()
    page_text = str(message.get("pageText", "")).strip()
    excerpt = (selection or page_text)[:80_000]
    tags = [str(tag).strip()[:60] for tag in message.get("tags", []) if str(tag).strip()][:30]
    content = f"# {title}\n\n{excerpt}\n\n[查看原网页]({url})"
    meta = {
        "capture_method": "browser_native_messaging",
        "url": url,
        "selection_only": bool(selection and not page_text),
    }
    return title, content, tags, meta


def handle_message(message: dict[str, Any], config: dict[str, str]) -> dict[str, Any]:
    """校验剪藏消息并通过来源受限令牌写入 Memory Protocol。"""

    if message.get("action") != "clip":
        return {"ok": False, "error": "不支持的宿主操作"}
    try:
        title, content, tags, meta = build_content(message)
        payload = json.dumps(
            {
                "source": config["source"],
                "kind": "clip",
                "title": title,
                "content": content,
                "content_format": "markdown",
                "tags": tags,
                "device_id": "browser-clipper",
                "meta": meta,
            },
            ensure_ascii=False,
        ).encode("utf-8")
        request = Request(
            f"{config['endpoint']}/v1/memories",
            data=payload,
            method="POST",
            headers={
                "Authorization": f"Bearer {config['token']}",
                "Content-Type": "application/json",
                "Accept": "application/json",
            },
        )
        with urlopen(request, timeout=15) as response:
            created = json.loads(response.read())
        return {"ok": True, "id": created["id"], "createdAt": created["created_at"]}
    except HTTPError as error:
        body = error.read().decode("utf-8", errors="replace")
        return {"ok": False, "error": f"Memory Protocol {error.code}: {body}"}
    except (URLError, ValueError, KeyError, json.JSONDecodeError) as error:
        return {"ok": False, "error": str(error)}


def main() -> int:
    """持续处理浏览器消息，单条业务错误不会终止宿主进程。"""

    try:
        config = load_config()
        while True:
            message = read_message(sys.stdin.buffer)
            if message is None:
                return 0
            write_message(sys.stdout.buffer, handle_message(message, config))
    except Exception as error:  # 宿主边界必须把启动/协议错误返回浏览器，而不是写入 stdout 日志。
        try:
            write_message(sys.stdout.buffer, {"ok": False, "error": str(error)})
        except Exception:
            pass
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
