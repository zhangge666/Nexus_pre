"""本文件验证 Python SDK 的固定来源、检索映射和 CLI 参数。"""

from __future__ import annotations

import unittest
from unittest.mock import patch
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from nexus_sdk import NexusClient, NexusError
from nexus_sdk.cli import build_parser


class _Response:
    """提供 urllib 上下文管理协议的响应替身。"""

    def __init__(self, body: bytes) -> None:
        self.body = body

    def __enter__(self) -> "_Response":
        """进入响应上下文。"""

        return self

    def __exit__(self, *_args: object) -> None:
        """退出响应上下文。"""

    def read(self) -> bytes:
        """返回预置响应正文。"""

        return self.body


class NexusClientTests(unittest.TestCase):
    """验证客户端的公开行为。"""

    @patch("nexus_sdk.client.urlopen")
    def test_add_memory_fixes_source(self, mocked_open: unittest.mock.Mock) -> None:
        """写入请求必须使用构造客户端时声明的来源。"""

        mocked_open.return_value = _Response(b'{"id":"m1","created_at":1}')
        client = NexusClient("http://127.0.0.1:4111", "secret", "external:python")
        result = client.add_memory("python memory", tags=["sdk"])
        request = mocked_open.call_args.args[0]
        self.assertEqual(result["id"], "m1")
        self.assertIn(b'"source": "external:python"', request.data)
        self.assertEqual(request.headers["Authorization"], "Bearer secret")

    def test_rejects_invalid_source(self) -> None:
        """公开 SDK 不能使用第一方来源。"""

        with self.assertRaises(NexusError):
            NexusClient("http://127.0.0.1:4111", "secret", "orbit")

    def test_cli_parses_search(self) -> None:
        """CLI 应稳定解析检索参数。"""

        args = build_parser().parse_args(["--token", "secret", "search", "memory", "--limit", "5"])
        self.assertEqual(args.command, "search")
        self.assertEqual(args.limit, 5)


if __name__ == "__main__":
    unittest.main()
