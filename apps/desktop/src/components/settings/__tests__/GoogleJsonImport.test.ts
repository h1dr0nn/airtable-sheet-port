import { describe, expect, it } from "vitest";
import { parseGoogleCredentials } from "../GoogleJsonImport.js";

describe("parseGoogleCredentials", () => {
  it("extracts credentials from an installed (desktop) client", () => {
    const raw = JSON.stringify({
      installed: {
        client_id: "123-abc.apps.googleusercontent.com",
        client_secret: "GOCSPX-secret"
      }
    });

    const result = parseGoogleCredentials(raw);

    expect(result).toEqual({
      ok: true,
      value: { clientId: "123-abc.apps.googleusercontent.com", clientSecret: "GOCSPX-secret" }
    });
  });

  it("extracts credentials from a web client and trims whitespace", () => {
    const raw = JSON.stringify({
      web: { client_id: "  web-id.apps.googleusercontent.com  ", client_secret: "  GOCSPX-web  " }
    });

    const result = parseGoogleCredentials(raw);

    expect(result).toEqual({
      ok: true,
      value: { clientId: "web-id.apps.googleusercontent.com", clientSecret: "GOCSPX-web" }
    });
  });

  it("reports a single problem when the file is not JSON", () => {
    const result = parseGoogleCredentials("not-json{");

    expect(result).toEqual({ ok: false, problems: ["The file is not valid JSON."] });
  });

  it("reports a problem when neither installed nor web section exists", () => {
    const result = parseGoogleCredentials(JSON.stringify({ other: {} }));

    expect(result.ok).toBe(false);
    if (result.ok) {
      throw new Error("expected failure");
    }
    expect(result.problems).toHaveLength(1);
    expect(result.problems[0]).toContain("installed");
    expect(result.problems[0]).toContain("web");
  });

  it("lists both fields when client_id and client_secret are missing", () => {
    const result = parseGoogleCredentials(JSON.stringify({ installed: {} }));

    expect(result.ok).toBe(false);
    if (result.ok) {
      throw new Error("expected failure");
    }
    expect(result.problems).toHaveLength(2);
    expect(result.problems.some((problem) => problem.includes("client_id"))).toBe(true);
    expect(result.problems.some((problem) => problem.includes("client_secret"))).toBe(true);
  });

  it("reports only the missing secret when the id is present", () => {
    const raw = JSON.stringify({ installed: { client_id: "id-only", client_secret: "  " } });

    const result = parseGoogleCredentials(raw);

    expect(result.ok).toBe(false);
    if (result.ok) {
      throw new Error("expected failure");
    }
    expect(result.problems).toEqual(['The "client_secret" field is missing or empty.']);
  });

  it("rejects a JSON array as not having a credentials section", () => {
    const result = parseGoogleCredentials(JSON.stringify([1, 2, 3]));

    expect(result.ok).toBe(false);
  });
});
