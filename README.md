# Airtable - Sheet Port

Let AI assistants like Claude, Cursor or Codex read and edit your Google Sheets,
safely and on your own machine.

- **No Google Cloud setup.** Connect your account with a small Apps Script you paste
  into your own Google Drive. No Cloud Console project, no API keys.
- **Works with the AI tools you already use.** One click registers it with Claude
  (Desktop and Code), Cursor, Windsurf, Cline, Antigravity and Codex.
- **Real spreadsheet skills.** The assistant can read, search, add and update rows, write
  any cell or formula, style tables, add dropdowns, checkboxes and color rules, and
  create or remove tabs. Just paste a Sheets link.
- **Your data stays yours.** Your Google access stays on your computer, in the system
  keychain. The assistant never sees your password or token, and every change is
  logged.

## Install

Download the latest version for Windows, macOS or Linux from
[Releases](https://github.com/h1dr0nn/airtable-sheet-port/releases) and run the
installer. The app updates itself when a new version is out.

## Get started

1. **Create a bridge.** Open the **Guide** tab. Follow the steps and copy the two files
   into [script.google.com](https://script.google.com). It takes about two minutes.
2. **Connect it.** Go to **Data Sources**, paste the bridge URL and secret, then click
   **Add Bridge**. Add one bridge per Google account if you use several.
3. **Connect your AI tool.** In **Settings > MCP Clients**, pick your tool and click
   **Configure**. Restart that tool.

Now ask your assistant something like:

> Read this sheet and summarize the totals by month: https://docs.google.com/spreadsheets/d/...

> Add a "Status" column with a dropdown (To do, Doing, Done) and color the Done rows green.

## Good to know

- Anyone who has your bridge URL **and** secret can access your Sheets, so keep the
  secret private. To change it, see the tips in the **Guide** tab.
- Changes are applied right away. Your AI tool shows what it is about to do, and the
  **Activity** log in the app lists everything it did.
- The app does not need to stay open. Your AI tool starts the connection on its own.

## License

[MIT](LICENSE) © 2026 h1dr0n
