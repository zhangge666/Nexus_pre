# nexus-sdk

Python SDK 与 `nexus` CLI 均只依赖标准库。

```bash
python -m pip install nexus-sdk
export NEXUS_ENDPOINT=http://127.0.0.1:4111
export NEXUS_TOKEN='<Orbit 生成的令牌>'
export NEXUS_SOURCE=external:cli
nexus add '# 一条长期记忆' --tag example
nexus search '长期记忆'
```

```python
from nexus_sdk import NexusClient

nexus = NexusClient(
    "http://127.0.0.1:4111",
    token="...",
    source="external:python",
)
nexus.add_memory("需要长期保留的内容", tags=["example"])
print(nexus.search_memory("长期保留"))
```
