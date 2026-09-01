Your task: find relevant information to answer the user's question.

You MUST use the `search_knowledge_base` tool to search the knowledge base for relevant document chunks.
If the question clearly belongs to one specific knowledge base, pass its id in `kb_ids`; otherwise search all of them.
Do NOT provide a final answer yet. Do not proceed to extracting documents or answering.
Only search for information.

User's question: {{ question }}
