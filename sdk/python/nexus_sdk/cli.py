"""本文件实现用于写入、检索和诊断 Nexus 的命令行入口。"""

from __future__ import annotations

import argparse
import json
import os
import sys
from typing import Sequence

from .client import NexusClient, NexusError


def build_parser() -> argparse.ArgumentParser:
    """构造 nexus CLI 的参数和子命令。"""

    parser = argparse.ArgumentParser(prog="nexus", description="Nexus Memory Protocol CLI")
    parser.add_argument("--endpoint", default=os.getenv("NEXUS_ENDPOINT", "http://127.0.0.1:4111"))
    parser.add_argument("--token", default=os.getenv("NEXUS_TOKEN", ""))
    parser.add_argument("--source", default=os.getenv("NEXUS_SOURCE", "external:cli"))
    commands = parser.add_subparsers(dest="command", required=True)

    add = commands.add_parser("add", help="写入记忆")
    add.add_argument("content", nargs="?", help="正文；省略时从标准输入读取")
    add.add_argument("--title")
    add.add_argument("--tag", action="append", default=[])
    add.add_argument("--plain", action="store_true")

    search = commands.add_parser("search", help="检索记忆")
    search.add_argument("query")
    search.add_argument("--limit", type=int, default=10)
    search.add_argument("--mode", choices=["hybrid", "keyword", "semantic"], default="hybrid")

    get = commands.add_parser("get", help="读取完整记忆")
    get.add_argument("id")

    ask = commands.add_parser("ask", help="基于记忆问答")
    ask.add_argument("question")

    commands.add_parser("capabilities", help="查看服务能力")
    return parser


def execute(args: argparse.Namespace, client: NexusClient) -> object:
    """执行已经解析的 CLI 子命令并返回可序列化结果。"""

    if args.command == "add":
        content = args.content if args.content is not None else sys.stdin.read()
        if not content.strip():
            raise NexusError("记忆正文不能为空")
        return client.add_memory(
            content,
            title=args.title,
            tags=args.tag,
            content_format="plain" if args.plain else "markdown",
        )
    if args.command == "search":
        return client.search_memory(args.query, limit=args.limit, mode=args.mode)
    if args.command == "get":
        return client.get_memory(args.id)
    if args.command == "ask":
        return client.ask_memory(args.question)
    if args.command == "capabilities":
        return client.capabilities()
    raise NexusError(f"未知命令: {args.command}")


def main(argv: Sequence[str] | None = None) -> int:
    """运行 CLI，标准输出只写 JSON，错误写入标准错误。"""

    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        client = NexusClient(args.endpoint, args.token, args.source)
        result = execute(args, client)
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 0
    except NexusError as error:
        print(f"nexus: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
