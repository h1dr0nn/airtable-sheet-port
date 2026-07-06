# Connectors

## Connector Abstraction

Connectors implement a provider-neutral table contract:

- list sources
- list tables
- describe table
- read records
- find records
- append records
- update records

The contract hides provider-specific tokens, endpoints, ranges, and response shapes from agents.

## Mock Connector

The mock connector provides one source and one customer table. It supports schema discovery, bounded reads, simple text search, append, and update. This lets the desktop and MCP server run without external accounts.

## Google Sheets Connector Plan

The Google Sheets connector will map a spreadsheet to a source and sheets/ranges to tables. Planned work:

- Tauri OAuth flow using Google consent.
- Store refresh tokens in OS keychain.
- Discover spreadsheets through Drive picker or user-provided spreadsheet ids.
- Infer table headers from the first row or configured header row.
- Map rows to records with stable generated row ids.
- Read and update only configured ranges.
- Preserve formulas unless a policy explicitly allows formula changes.

## Airtable Connector Plan

The Airtable connector will map bases to sources, tables to tables, fields to schema fields, and records directly to internal records. Planned work:

- API key or OAuth setup stored in OS keychain.
- Base and table discovery.
- Field type mapping to Sheet Port field types.
- Pagination and rate-limit handling.
- Batch update and append operations.

## Google Sheets Range Mapping

Google Sheets does not have native row ids. Sheet Port should create internal record ids using a table id plus row number or a hidden configured id column. The preferred production path is a stable id column because row-number ids shift when users sort or insert rows.

## Airtable Mapping

Airtable records already have stable ids. Field metadata maps naturally into `FieldSchema`, while linked records, attachments, collaborators, and formulas should initially map to `unknown` until specific support is added.

## Current Limitations

- Google Sheets and Airtable connectors are skeleton packages.
- The mock connector stores all state in memory.
- No rate limit or retry policy exists yet.
