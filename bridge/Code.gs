/**
 * Airtable - Sheet Port: Google token bridge.
 *
 * A tiny Apps Script web app that hands the desktop app / MCP sidecar a
 * short-lived OAuth access token for the account that deployed it, so no
 * Cloud Console project or OAuth client is needed.
 *
 * Flow: POST {"secret": "..."} to the /exec URL. When the secret matches the
 * one stored by setup(), the response is
 *   {ok: true, accessToken, email, expiresInSec}
 * otherwise {ok: false, error: 'unauthorized'}.
 *
 * Anyone holding the /exec URL AND the secret can mint a token for this
 * account. Keep the secret private. See bridge/README.md.
 */

var SECRET_KEY = 'SECRET';

/**
 * Run once from the editor. Generates the shared secret (only if none exists
 * yet), stores it in Script Properties, and logs it so it can be copied into
 * the desktop app. To rotate, delete the SECRET property and run it again.
 */
function setup() {
  var props = PropertiesService.getScriptProperties();
  var secret = props.getProperty(SECRET_KEY);
  if (!secret) {
    secret = Utilities.getUuid() + Utilities.getUuid().replace(/-/g, '');
    props.setProperty(SECRET_KEY, secret);
  }
  Logger.log('SECRET = ' + secret);
}

/** Web app entry point: exchanges the secret for an access token. */
function doPost(e) {
  var body = {};
  try {
    body = JSON.parse((e && e.postData && e.postData.contents) || '{}');
  } catch (err) {
    body = {};
  }

  var expected = PropertiesService.getScriptProperties().getProperty(SECRET_KEY);
  if (!expected || body.secret !== expected) {
    return json_({ ok: false, error: 'unauthorized' });
  }

  return json_({
    ok: true,
    accessToken: ScriptApp.getOAuthToken(),
    email: Session.getEffectiveUser().getEmail(),
    // Apps Script tokens live about an hour; report a safe margin under that.
    expiresInSec: 3000
  });
}

/** Serializes a value as a JSON ContentService response. */
function json_(value) {
  return ContentService.createTextOutput(JSON.stringify(value)).setMimeType(
    ContentService.MimeType.JSON
  );
}
