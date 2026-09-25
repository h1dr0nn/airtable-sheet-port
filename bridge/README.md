# Google Bridge (Apps Script)

Airtable - Sheet Port reaches Google Sheets through a small Apps Script web app that you
deploy on your own Google account. The bridge trades a shared secret for a short-lived
OAuth access token (`ScriptApp.getOAuthToken()`), so there is no Cloud Console project,
no OAuth client id, and no client secret to manage.

One bridge serves one Google account. Deploy one per account you want to connect.

## Files

| File | Purpose |
|---|---|
| `Code.gs` | `setup()` creates the secret; `doPost(e)` returns a token when the secret matches |
| `appsscript.json` | Manifest: V8 runtime, OAuth scopes, advanced services, web app settings |

Scopes requested: `spreadsheets`, `drive.metadata.readonly` (list spreadsheets),
`userinfo.email` (report which account the bridge belongs to).

## Setup

1. Open [script.google.com](https://script.google.com) and click **New project**. Use a
   standalone project, not one bound to a sheet, so the bridge does not depend on any
   particular spreadsheet.
2. Open **Project Settings** and tick **Show "appsscript.json" manifest file in editor**.
3. Back in the editor, replace the contents of `Code.gs` and `appsscript.json` with the
   files in this folder.
4. Select the `setup` function and click **Run**. Allow the permissions Google asks for,
   then copy the `SECRET = ...` value from the execution log.
5. Click **Deploy > New deployment**, choose type **Web app**, set **Execute as: Me** and
   **Who has access: Anyone**, then deploy and copy the web app URL (ends in `/exec`).
6. In the desktop app open **Data Sources**, paste the URL and the secret,
   and add the bridge. The app calls the bridge once, reads the account email, and adds
   the source `google-sheets:{accountKey}`.

   Headless alternative: `SHEET_PORT_BRIDGE_SECRET=<secret> sheet-port-mcp bridge add <url>`
   (also `bridge list` and `bridge remove <sourceId>`).

## Request and response

```http
POST https://script.google.com/macros/s/{deploymentId}/exec
Content-Type: application/json

{ "secret": "..." }
```

```json
{ "ok": true, "accessToken": "ya29...", "email": "me@example.com", "expiresInSec": 3000 }
```

A missing or wrong secret returns `{ "ok": false, "error": "unauthorized" }`.

## Updating the code

Use **Deploy > Manage deployments**, pick the existing deployment, click **Edit**, choose
**New version**, and deploy. This keeps the same `/exec` URL, so nothing changes in the
desktop app. **Deploy > New deployment** creates a new URL, which you would then have to
add again.

## Why the advanced services are required

The manifest enables the Sheets v4 and Drive v3 advanced services even though `Code.gs`
never calls them. Enabling them turns those APIs on in the hidden default GCP project
behind the script. Without them, the token works but every Sheets or Drive request fails
with 403 "API has not been used in project ... before or it is disabled".

## Rotating the secret

In **Project Settings > Script Properties**, delete the `SECRET` property, then run
`setup` again and copy the new value. The old secret stops working immediately. Add the
bridge again in the desktop app with the same URL and the new secret; a bridge for the
same email replaces the old entry.

## Revoking access

To cut the bridge off completely, remove the script's access at
[myaccount.google.com](https://myaccount.google.com) > **Security** > **Third-party
access** (listed under the project name). You can also archive the deployment under
**Manage deployments**.

## Security

Anyone who holds both the `/exec` URL and the secret can obtain a one-hour access token
with the scopes above for your account: read and write every spreadsheet you can edit,
and list your Drive files. Treat the secret like a password:

- Do not commit it, paste it into chats, or share the URL and secret together.
- The desktop app stores both in the OS keychain (service `sheet-port`), never in the
  SQLite database.
- Rotate the secret if you think it leaked, and revoke access if you stop using the
  bridge.
