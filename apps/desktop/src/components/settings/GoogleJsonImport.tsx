import { useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  toast
} from "@sheet-port/ui";
import { getErrorMessage } from "../../lib/errors.js";
import { useTranslation } from "../../i18n/useTranslation.js";
import { ipc } from "../../lib/ipc.js";
import { queryKeys } from "../../lib/queryKeys.js";

// Google Cloud Console exports desktop OAuth clients under an "installed" key
// and web clients under a "web" key. Both carry the same credential fields.
const CREDENTIAL_KEYS = ["installed", "web"] as const;

type ParsedCredentials = {
  clientId: string;
  clientSecret: string;
};

/**
 * Parses a Google OAuth client JSON file. Returns the extracted credentials on
 * success, or a list of human-readable problems describing what is missing.
 * Exported so the parsing rules can be exercised in isolation.
 */
export function parseGoogleCredentials(
  raw: string
): { ok: true; value: ParsedCredentials } | { ok: false; problems: string[] } {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return { ok: false, problems: ["The file is not valid JSON."] };
  }

  if (typeof parsed !== "object" || parsed === null) {
    return { ok: false, problems: ["The file is not valid JSON."] };
  }

  const record = parsed as Record<string, unknown>;
  const containerKey = CREDENTIAL_KEYS.find(
    (key) => typeof record[key] === "object" && record[key] !== null
  );

  if (!containerKey) {
    return {
      ok: false,
      problems: [
        'The file has no "installed" or "web" section. Download the OAuth client JSON from the Google Cloud Console.'
      ]
    };
  }

  const container = record[containerKey] as Record<string, unknown>;
  const rawClientId = container.client_id;
  const rawClientSecret = container.client_secret;

  const problems: string[] = [];
  const clientId = typeof rawClientId === "string" ? rawClientId.trim() : "";
  const clientSecret = typeof rawClientSecret === "string" ? rawClientSecret.trim() : "";

  if (clientId === "") {
    problems.push('The "client_id" field is missing or empty.');
  }
  if (clientSecret === "") {
    problems.push('The "client_secret" field is missing or empty.');
  }

  if (problems.length > 0) {
    return { ok: false, problems };
  }

  return { ok: true, value: { clientId, clientSecret } };
}

/** Reads a File as UTF-8 text via FileReader (no Tauri fs plugin dependency). */
function readFileAsText(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result ?? ""));
    reader.onerror = () => reject(new Error("The file could not be read."));
    reader.readAsText(file);
  });
}

/**
 * "Import JSON" button that reads a Google OAuth client JSON file, extracts the
 * client id and secret, and stores both. Validation failures surface an error
 * modal listing every problem; the file input resets after each attempt.
 */
export function GoogleJsonImport() {
  const queryClient = useQueryClient();
  const { t } = useTranslation();
  const inputRef = useRef<HTMLInputElement>(null);
  const [isImporting, setIsImporting] = useState(false);
  const [errorProblems, setErrorProblems] = useState<string[] | null>(null);

  const openPicker = () => {
    inputRef.current?.click();
  };

  const resetInput = () => {
    if (inputRef.current) {
      inputRef.current.value = "";
    }
  };

  const applyCredentials = async ({ clientId, clientSecret }: ParsedCredentials) => {
    // Sequential: the secret depends on the same keychain-backed config record.
    await ipc.setGoogleClientId(clientId);
    await ipc.setGoogleClientSecret(clientSecret);
    await queryClient.invalidateQueries({ queryKey: queryKeys.googleConfig });
    toast.success(t("settings.import.successTitle"), {
      description: t("settings.import.successDescription")
    });
  };

  const handleFile = async (file: File) => {
    setIsImporting(true);
    try {
      const raw = await readFileAsText(file);
      const result = parseGoogleCredentials(raw);
      if (!result.ok) {
        setErrorProblems(result.problems);
        return;
      }
      await applyCredentials(result.value);
    } catch (error: unknown) {
      toast.error(t("settings.import.failed"), { description: getErrorMessage(error) });
    } finally {
      setIsImporting(false);
      resetInput();
    }
  };

  const onChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (file) {
      void handleFile(file);
    }
  };

  return (
    <>
      <input
        ref={inputRef}
        type="file"
        accept="application/json,.json"
        className="hidden"
        onChange={onChange}
      />
      <Button variant="ghost" size="sm" disabled={isImporting} onClick={openPicker}>
        {isImporting ? t("settings.google.importing") : t("settings.google.importJson")}
      </Button>

      <Dialog
        open={errorProblems !== null}
        onOpenChange={(open) => {
          if (!open) {
            setErrorProblems(null);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("settings.import.invalidTitle")}</DialogTitle>
            <DialogDescription>{t("settings.import.invalidDescription")}</DialogDescription>
          </DialogHeader>
          <ul className="list-disc space-y-1.5 pl-5 text-[12.5px] text-ink-muted">
            {(errorProblems ?? []).map((problem) => (
              <li key={problem}>{problem}</li>
            ))}
          </ul>
          <DialogFooter>
            <Button variant="secondary" onClick={() => setErrorProblems(null)}>
              {t("common.close")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
