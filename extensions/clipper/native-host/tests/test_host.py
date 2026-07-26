"""本文件验证剪藏正文构建、Native Messaging 帧与协议写入映射。"""

from __future__ import annotations

import io
import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import nexus_clipper_host as host


class _Response:
    """提供 urllib 响应上下文替身。"""

    def __enter__(self) -> "_Response":
        """进入响应上下文。"""

        return self

    def __exit__(self, *_args: object) -> None:
        """退出响应上下文。"""

    def read(self) -> bytes:
        """返回成功创建响应。"""

        return b'{"id":"clip-1","created_at":1}'


class NativeHostTests(unittest.TestCase):
    """验证浏览器宿主的安全边界。"""

    def test_round_trips_native_message(self) -> None:
        """长度前缀消息应可无损读写。"""

        stream = io.BytesIO()
        host.write_message(stream, {"action": "clip", "title": "标题"})
        stream.seek(0)
        self.assertEqual(host.read_message(stream)["title"], "标题")

    @patch("nexus_clipper_host.urlopen")
    def test_maps_clip_to_external_source(self, mocked_open: unittest.mock.Mock) -> None:
        """剪藏请求必须固定使用 external:clipper。"""

        mocked_open.return_value = _Response()
        result = host.handle_message(
            {
                "action": "clip",
                "title": "Nexus",
                "url": "https://example.com/article",
                "selection": "selected text",
                "pageText": "",
                "tags": ["web"],
            },
            {"endpoint": "http://127.0.0.1:4111", "token": "secret", "source": "external:clipper"},
        )
        request = mocked_open.call_args.args[0]
        payload = json.loads(request.data)
        self.assertTrue(result["ok"])
        self.assertEqual(payload["source"], "external:clipper")
        self.assertEqual(payload["kind"], "clip")
        self.assertEqual(request.headers["Authorization"], "Bearer secret")

    def test_rejects_non_http_page(self) -> None:
        """宿主不能把浏览器内部页面伪装为普通网页剪藏。"""

        with self.assertRaises(ValueError):
            host.build_content({"title": "设置", "url": "chrome://settings"})


if __name__ == "__main__":
    unittest.main()
