# Product Features TODO

| Feature | Priority | Description |
|---|---|---|
| Document ingestion pipeline | P0 | Accept PDF, DOCX, HTML, Markdown, and plain text in addition to the current `text/plain`. Extract clean text, preserve structure where possible, store the original, and index the result into a knowledge base. Used by both the KB upload UI and the CLI sync path. |
| OCR adapter | P0 | Optional integration with an external OCR/extraction engine for scanned PDFs and images. Keeps the native pipeline fast and dependency-light while allowing high-quality OCR when needed. Configured per deployment, not per document. |
| KB management UI | P0 | In-app workspace to create and inspect knowledge bases and documents. Supports upload, browsing, preview of original and extracted text, metadata editing, reindex, and delete. Replaces the current CLI/internal-API-only workflow. |
| RAG eval harness | P0 | Offline benchmark runner with golden question/answer/document sets. Reports retrieval recall@k, ranking quality, citation faithfulness, and regressions between runs. Designed for CLI/CI use, not an in-chat leaderboard. |
| Chat-attached RAG and scoping | P1 | Let a chat attach specific knowledge bases or files so retrieval is scoped to them. Support per-chat selection instead of always searching every KB. Reduces noise and improves answer precision. |
| Focused vs full-document retrieval | P1 | Choose per attachment whether to retrieve relevant chunks or inject the entire document. Mirrors the two useful retrieval modes without prompt-injection hacks. Default remains focused retrieval. |
| Citation validation | P1 | Every link and title cited in the final answer must correspond to a source returned by the retrieval tools. Flag or strip links that were never retrieved, and optionally verify that the cited chunk contains the referenced claim. Format validation is already handled by the renderer; this is about membership and fabricated sources. |
| Per-KB ACL | P1 | Grant read/manage access to knowledge bases per user, and optionally per group. Enforce it in search, document access, and internal APIs. Replaces the current public/private flag only. |
| Full-text chat history search | P1 | Search across message content, not only chat titles. Return matching snippets with navigation to the message. Works within the user's own chats. |
| Temporary chats | P1 | Start a chat that is never persisted or indexed. Cleared when the session ends. Useful for sensitive one-off questions. |
| Import/export chats | P1 | Export a user's chats to a portable JSON format and import them back. Supports backup and migration. Preserves messages, sources, and metadata. |
| Light usage analytics | P1 | Aggregate token usage and message/chat counts per user and model. Show a compact personal view and an admin summary. No cost dashboards or heatmaps required. |
| Banners | P1 | Admin-managed announcements shown on the home screen. Support an active window, severity, and dismissal. Intended for maintenance notices and policy updates, not a CMS. |
| STT dictation | P1 | Record speech in the composer and transcribe it into the message input. Configurable provider with a browser fallback. Primarily an ergonomics and accessibility feature. |
| TTS read-aloud | P1 | Read assistant responses aloud, sentence by sentence as they stream. Configurable provider and voice, with browser speech synthesis as fallback. Primarily an accessibility feature. |
| Mermaid rendering | P1 | Render Mermaid code fences as diagrams in chat. Keep the source visible and copyable, and render it in a safe, non-scripting context. Useful for flows and architecture explanations. |
| Sandboxed code execution | P1 | Let the model run code for calculations and data checks in an isolated environment. No network by default, strict CPU/memory/time limits, and explicit user approval. Output returns to the chat; no arbitrary HTML/JS rendering. |
| Tool approvals and guardrails | P1 | Pause sensitive tool calls such as code execution and external tools for explicit user approval. Show the tool, arguments, and scope before execution. Provides an audit trail for risky actions. |
| Prompt caching and cost telemetry | P1 | Keep prompt prefixes stable and report cache hits and token costs where the provider exposes them. Helps control spend on long chats and research runs. Provider-dependent; never promises savings. |
| Accessibility | P1 | Keyboard navigation, focus management, screen-reader labels, and contrast support across chat and settings. Complements STT/TTS. Treated as a quality bar, not a separate mode. |
| Folders and tags | P2 | Organize chats into lightweight folders and tags. Keep the flat date-grouped list as the default. Navigation aid only, not project workspaces. |
| Archived chats | P2 | Archive chats out of the main list without deleting them. Restore or delete individually. No separate dashboard required. |
| Conversation branching | P2 | Fork a chat from an assistant message into an independent conversation. Useful for exploring alternatives during research. The original chat stays untouched. |
| Job-done notification | P2 | Notify the user when a long-running scenario such as deep research finishes or fails. Minimal delivery via the existing UI or a configured webhook. Not a full inbox. |
| Stream resume | P2 | Resume an interrupted stream without losing generated output. Requires a persistent event log and a resumable transport. Solves long research runs over flaky networks. |
| Image generation | P2 | Generate images from a prompt via a configured image model. Store and show them inline in chat. Optional capability, separate from retrieval and reasoning. |
| Memory | P2 | Store user-approved facts and preferences that persist across chats. User-visible, editable, and deletable; no autonomous background writes initially. Injected only when relevant. |
| Skills | P2 | Reusable markdown instruction sets that can be attached to a scenario or invoked explicitly. Stored as versioned content, not executable code. Teach the model a workflow or house style. |
| MCP | P2 | Connect to admin-approved MCP servers and expose their tools to the model. Allowlisted servers, TLS, encrypted credentials, and per-tool timeouts. No user-installed arbitrary servers initially. |
| Config-driven scenario presets | P2 | Define scenario presets (prompt, tools, KB scope, sampling) in configuration instead of hardcoding them. Keeps the curated `simple`/`deep_research` models while allowing deployment-specific variants. Avoids a full model CMS. |
| Optional web search | P2 | Add a single optional web-search tool for fresh information. One provider per deployment, cited like KB sources. Disabled by default to keep retrieval predictable. |
| KB export/backup | P2 | Export a knowledge base and its documents for backup or migration. Import restores the corpus and metadata. Complements chat import/export. |
