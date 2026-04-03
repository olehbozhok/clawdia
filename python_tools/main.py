import asyncio

from dotenv import load_dotenv
import os

from langchain_deepseek import ChatDeepSeek
from langchain_mcp_adapters.client import MultiServerMCPClient
from langgraph.prebuilt import create_react_agent
from pydantic import SecretStr

load_dotenv()

llm = ChatDeepSeek(
    model=os.getenv("DEEPSEEK_MODEL", "deepseek-chat"),
    api_key=SecretStr(str(os.getenv("DEEPSEEK_API_KEY"))),
)

MCP_SERVERS = {
    "filesystem": {
        "transport": "stdio",
        "command": "npx",
        "args": [
            "-y",
            "@modelcontextprotocol/server-filesystem",
            os.path.expanduser("~/work"),
        ],
    },
}


async def main():
    client = MultiServerMCPClient(MCP_SERVERS)
    tools = await client.get_tools()
    print(f"Loaded MCP tools: {[t.name for t in tools]}")

    # Print tools per server (full inputSchema)
    import json

    for server_name in MCP_SERVERS:
        async with client.session(server_name) as session:
            response = await session.list_tools()
            print(f"\n=== {server_name} ({len(response.tools)} tools) ===")
            for tool in response.tools:
                print(f"\n  {tool.name}:")
                print(f"    description: {tool.description}")
                print(f"    inputSchema: {json.dumps(tool.inputSchema, indent=6)}")

    # agent = create_react_agent(
    #     llm.bind_tools(tools, parallel_tool_calls=False),
    #     tools,
    # )

    # print("Chat with DeepSeek + MCP (type 'quit' to exit)")
    # while True:
    #     user_input = input("\nYou: ")
    #     if user_input.strip().lower() in ("quit", "exit"):
    #         break
    #     result = await agent.ainvoke({"messages": [("human", user_input)]})
    #     print(f"\nAssistant: {result['messages'][-1].content}")


if __name__ == "__main__":
    asyncio.run(main())
