# Security

## Threat Model

Airtable - Sheet Port assumes AI agents may be over-permissioned, prompt-injected, or confused by untrusted spreadsheet content. The app reduces blast radius by exposing typed local tools instead of provider credentials or broad execution primitives.

## Token Handling

Provider tokens and API keys must be owned by the desktop app and stored in OS secure storage. Connectors receive credentials through internal app services, not through MCP tool inputs.

The MVP includes connector TODOs but does not implement real token storage yet.

## Why Agents Never Receive Tokens

If agents receive provider credentials, they can bypass Airtable - Sheet Port policy, audit, preview, and confirmation flows. Airtable - Sheet Port keeps tokens behind a local capability broker so every action can be checked, logged, and shaped through narrow schemas.

## Local MCP Attack Surface

The MCP server should run locally with stdio transport or, if HTTP/SSE is later added, bind only to `127.0.0.1`. Tool input schemas should remain strict, bounded, and provider-neutral.

## Tool Allowlist

Allowed tool categories:

- list data sources
- list tables
- describe schema
- bounded reads
- text search
- preview updates
- append through pending change
- commit a pending change
- read audit log

Dangerous tools to avoid:

- shell command execution
- arbitrary JavaScript execution
- raw provider API calls
- token export
- SQL execution
- destructive delete without explicit confirmation semantics

## Permission Checks

Read tools require `read`. Write previews and commits require `write`. Delete operations require `deleteRecords` and are not implemented in the MVP. `requireConfirmationFor` marks changes that must pass a user or policy confirmation layer before commit.

## Audit Log

The audit log records tool calls, previews, commits, policy denials, and system events. The MVP stores events in memory; production should persist them in SQLite with timestamps, actor, action, source/table scope, and metadata.

## Confirmation Flow

All write operations create a pending change first. `commit_change` is the only tool that applies a pending change. The desktop app should surface pending changes to users before commit in human-driven workflows.

## Current Limitations

- In-memory audit logs are lost when the MCP server exits.
- Confirmation is represented in policy metadata, but desktop approval is not yet wired to MCP state.
- Secure storage and OAuth are not implemented yet.
