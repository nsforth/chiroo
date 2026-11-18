Your task: find relevant information to answer the user's question.

You MUST use the `search_in_all_kbs` tool to search the knowledge base for relevant document chunks.
Call it exactly once: put every search angle you want to try into the single `queries` array of that one call — it accepts several distinct natural-language queries. Do NOT split the search into multiple parallel tool calls, and do NOT put the array into a string: `queries` must be a JSON array of strings, e.g. ["query one", "query two"].
If the question clearly belongs to one specific knowledge base, you may pass its id in `kb_ids` to narrow the search.
Do NOT provide a final answer yet. Do not proceed to extracting documents or answering.
Only search for information.

User's question: {{ question }}
