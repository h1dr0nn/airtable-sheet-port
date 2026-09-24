import { useState, type FormEvent } from "react";
import { ChevronRight } from "lucide-react";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  cn,
  FOCUS_RING,
  Input,
  Skeleton,
  Tooltip,
  TooltipContent,
  TooltipTrigger
} from "@sheet-port/ui";
import bridgeCode from "../../../../../bridge/Code.gs?raw";
import bridgeManifest from "../../../../../bridge/appsscript.json?raw";
import {
  useAddGoogleBridge,
  useGoogleAccounts,
  useRemoveGoogleBridge,
  useTestGoogleBridge
} from "../../hooks/useGoogleBridges.js";
import { useTranslation } from "../../i18n/useTranslation.js";
import type { TranslationKey } from "../../i18n/translations.js";
import { shortenId } from "../../lib/format.js";
import type { GoogleAccount } from "../../lib/ipc.js";
import { ConfirmDialog } from "../ConfirmDialog.js";
import { CopyButton } from "../CopyButton.js";

/** Setup steps shown in the collapsible guide, in order. */
const GUIDE_STEP_KEYS: readonly TranslationKey[] = [
  "settings.bridges.step1",
  "settings.bridges.step2",
  "settings.bridges.step3",
  "settings.bridges.step4",
  "settings.bridges.step5",
  "settings.bridges.step6"
];

/** The two Apps Script files the user pastes into the editor. */
const BRIDGE_FILES: ReadonlyArray<{ name: string; content: string; copyKey: TranslationKey }> = [
  { name: "Code.gs", content: bridgeCode, copyKey: "settings.bridges.copyCode" },
  { name: "appsscript.json", content: bridgeManifest, copyKey: "settings.bridges.copyManifest" }
];

/** One bridge: email, deployment id and URL, plus Test and a confirmed Remove. */
function BridgeRow({ account }: { account: GoogleAccount }) {
  const test = useTestGoogleBridge();
  const remove = useRemoveGoogleBridge();
  const { t } = useTranslation();
  const [isConfirmOpen, setIsConfirmOpen] = useState(false);
  // An account whose keychain credential is missing (or left over from the
  // removed OAuth sign-in) has no bridge to test; it can only be removed.
  const hasBridge = account.bridgeUrl !== "";
  const isBusy = test.isPending || remove.isPending;

  return (
    <li className="py-3 first:pt-0 last:pb-0">
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13px] font-medium text-ink">{account.email}</p>
          {hasBridge ? (
            <>
              <p className="mt-0.5 text-[12px] text-ink-muted">
                {t("settings.bridges.deployment")}{" "}
                <span className="font-mono text-ink">{shortenId(account.deploymentId)}</span>
              </p>
              <Tooltip>
                <TooltipTrigger asChild>
                  <p className="mt-0.5 truncate font-mono text-[11.5px] text-ink-muted">
                    {account.bridgeUrl}
                  </p>
                </TooltipTrigger>
                <TooltipContent className="max-w-md break-all font-mono">
                  {account.bridgeUrl}
                </TooltipContent>
              </Tooltip>
            </>
          ) : (
            <p className="mt-0.5 text-[12px] leading-4 text-warning">
              {t("settings.bridges.missingCredential")}
            </p>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {hasBridge ? (
            <Button
              variant="outline"
              size="sm"
              disabled={isBusy}
              onClick={() => test.mutate(account.sourceId)}
            >
              {test.isPending ? t("settings.bridges.testing") : t("settings.bridges.test")}
            </Button>
          ) : null}
          <Button
            variant="outline"
            size="sm"
            disabled={isBusy}
            onClick={() => setIsConfirmOpen(true)}
          >
            {t("settings.bridges.remove")}
          </Button>
        </div>
      </div>
      <ConfirmDialog
        open={isConfirmOpen}
        onOpenChange={setIsConfirmOpen}
        title={t("settings.bridges.removeTitle")}
        description={t("settings.bridges.removeDescription", { email: account.email })}
        confirmLabel={t("settings.bridges.remove")}
        isPending={remove.isPending}
        onConfirm={() =>
          remove.mutate(account.sourceId, { onSettled: () => setIsConfirmOpen(false) })
        }
      />
    </li>
  );
}

/** URL + secret form; clears itself once the bridge is added. */
function AddBridgeForm() {
  const add = useAddGoogleBridge();
  const { t } = useTranslation();
  const [url, setUrl] = useState("");
  const [secret, setSecret] = useState("");

  const trimmedUrl = url.trim();
  const trimmedSecret = secret.trim();
  const canAdd = trimmedUrl !== "" && trimmedSecret !== "" && !add.isPending;

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!canAdd) {
      return;
    }
    add.mutate(
      { url: trimmedUrl, secret: trimmedSecret },
      {
        onSuccess: () => {
          setUrl("");
          setSecret("");
        }
      }
    );
  };

  return (
    <form className="space-y-3" onSubmit={submit}>
      <p className="text-[13px] font-medium text-ink">{t("settings.bridges.addTitle")}</p>
      <div className="space-y-1.5">
        <label className="text-[12px] font-medium text-ink-muted" htmlFor="google-bridge-url">
          {t("settings.bridges.url")}
        </label>
        <Input
          id="google-bridge-url"
          className="font-mono text-[12.5px]"
          value={url}
          placeholder="https://script.google.com/macros/s/.../exec"
          spellCheck={false}
          autoComplete="off"
          onChange={(event) => setUrl(event.target.value)}
        />
      </div>
      <div className="space-y-1.5">
        <label className="text-[12px] font-medium text-ink-muted" htmlFor="google-bridge-secret">
          {t("settings.bridges.secret")}
        </label>
        <Input
          id="google-bridge-secret"
          type="password"
          className="font-mono text-[12.5px]"
          value={secret}
          placeholder={t("settings.bridges.secretPlaceholder")}
          spellCheck={false}
          autoComplete="off"
          onChange={(event) => setSecret(event.target.value)}
        />
        <p className="text-[12px] leading-4 text-ink-muted">{t("settings.bridges.secretHint")}</p>
      </div>
      <div className="flex justify-end">
        <Button type="submit" size="sm" disabled={!canAdd}>
          {add.isPending ? t("settings.bridges.adding") : t("settings.bridges.add")}
        </Button>
      </div>
    </form>
  );
}

/** Collapsible "How to create a bridge" steps plus copy buttons for both files. */
function BridgeGuide() {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);

  return (
    <div>
      <button
        type="button"
        aria-expanded={isOpen}
        aria-controls="google-bridge-guide"
        onClick={() => setIsOpen((current) => !current)}
        className={cn(
          "flex items-center gap-1.5 rounded text-[13px] font-medium text-accent transition-colors hover:text-accent-hover",
          FOCUS_RING
        )}
      >
        <ChevronRight
          size={14}
          aria-hidden
          className={cn("transition-transform", isOpen && "rotate-90")}
        />
        {t("settings.bridges.guideToggle")}
      </button>
      {isOpen ? (
        <div id="google-bridge-guide" className="mt-3 space-y-3">
          <ol className="list-decimal space-y-1.5 pl-5 text-[12.5px] leading-5 text-ink-muted">
            {GUIDE_STEP_KEYS.map((key) => (
              <li key={key}>{t(key)}</li>
            ))}
          </ol>
          <ul className="divide-y divide-edge rounded-md border border-edge bg-surface">
            {BRIDGE_FILES.map((file) => (
              <li key={file.name} className="flex items-center justify-between gap-3 px-3 py-1.5">
                <span className="font-mono text-[12.5px] text-ink">{file.name}</span>
                <CopyButton value={file.content} label={t(file.copyKey)} />
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </div>
  );
}

/** Google Sheets access through Apps Script bridges, one per Google account. */
export function GoogleBridgesCard() {
  const { data: accounts, isPending } = useGoogleAccounts();
  const { t } = useTranslation();
  const accountList = accounts ?? [];

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.bridges.title")}</CardTitle>
        <Badge variant={accountList.length > 0 ? "success" : "muted"}>
          {accountList.length > 0 ? t("common.connected") : t("common.notConnected")}
        </Badge>
      </CardHeader>
      <CardContent>
        <div className="space-y-4">
          <p className="text-[12.5px] leading-5 text-ink-muted">
            {t("settings.bridges.description")}
          </p>
          {isPending ? (
            <Skeleton className="h-16" />
          ) : accountList.length === 0 ? (
            <p className="rounded-md border border-edge bg-surface px-3 py-6 text-center text-[12.5px] text-ink-muted">
              {t("settings.bridges.empty")}
            </p>
          ) : (
            <ul className="divide-y divide-edge">
              {accountList.map((account) => (
                <BridgeRow key={account.sourceId} account={account} />
              ))}
            </ul>
          )}
          <div className="border-t border-edge pt-4">
            <AddBridgeForm />
          </div>
          <div className="border-t border-edge pt-4">
            <BridgeGuide />
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
