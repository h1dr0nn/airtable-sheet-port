import type { ReactNode } from "react";
import { ArrowRight, ExternalLink } from "lucide-react";
import { Button, Card, CardContent, CardHeader, CardTitle, cn, FOCUS_RING } from "@sheet-port/ui";
import bridgeCode from "../../../../bridge/Code.gs?raw";
import bridgeManifest from "../../../../bridge/appsscript.json?raw";
import { CopyButton } from "../components/CopyButton.js";
import { ScreenHeader } from "../components/ScreenHeader.js";
import { useTranslation } from "../i18n/useTranslation.js";
import type { TranslationKey } from "../i18n/translations.js";
import type { ScreenId } from "../lib/nav.js";
import { openExternal } from "../lib/openExternal.js";

const APPS_SCRIPT_URL = "https://script.google.com";

/** The two Apps Script files the user pastes into the editor, in step order. */
const BRIDGE_FILES: ReadonlyArray<{ name: string; content: string }> = [
  { name: "Code.gs", content: bridgeCode },
  { name: "appsscript.json", content: bridgeManifest }
];

type Step = { titleKey: TranslationKey; bodyKey: TranslationKey };

const STEPS: readonly Step[] = [
  { titleKey: "guide.step1.title", bodyKey: "guide.step1.body" },
  { titleKey: "guide.step2.title", bodyKey: "guide.step2.body" },
  { titleKey: "guide.step3.title", bodyKey: "guide.step3.body" },
  { titleKey: "guide.step4.title", bodyKey: "guide.step4.body" },
  { titleKey: "guide.step5.title", bodyKey: "guide.step5.body" },
  { titleKey: "guide.step6.title", bodyKey: "guide.step6.body" }
];

const TIPS: readonly Step[] = [
  { titleKey: "guide.tips.update.title", bodyKey: "guide.tips.update.body" },
  { titleKey: "guide.tips.services.title", bodyKey: "guide.tips.services.body" },
  { titleKey: "guide.tips.rotate.title", bodyKey: "guide.tips.rotate.body" },
  { titleKey: "guide.tips.revoke.title", bodyKey: "guide.tips.revoke.body" },
  { titleKey: "guide.tips.secret.title", bodyKey: "guide.tips.secret.body" }
];

/** Line count without the trailing newline most files end with. */
function countLines(content: string): number {
  return content.replace(/\n$/, "").split("\n").length;
}

/** One numbered step: badge, title, body, and an optional action row. */
function StepItem({ index, step, action }: { index: number; step: Step; action?: ReactNode }) {
  const { t } = useTranslation();
  return (
    <li className="flex gap-3.5 py-4 first:pt-0 last:pb-0">
      <span
        aria-hidden
        className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full border border-edge-strong bg-surface font-mono text-[12px] font-semibold text-ink"
      >
        {index + 1}
      </span>
      <div className="min-w-0 flex-1 pt-0.5">
        <p className="text-[13px] font-medium text-ink">{t(step.titleKey)}</p>
        <p className="mt-1 text-[12.5px] leading-5 text-ink-muted">{t(step.bodyKey)}</p>
        {action ? <div className="mt-2.5 flex flex-wrap items-center gap-2">{action}</div> : null}
      </div>
    </li>
  );
}

/** One bridge file: name header with a prominent Copy, full content below. */
function CodeFileCard({ name, content }: { name: string; content: string }) {
  const { t } = useTranslation();
  return (
    <Card>
      <CardHeader className="py-2.5">
        <div className="flex min-w-0 items-baseline gap-2.5">
          <h3 className="truncate font-mono text-[13px] font-medium text-ink">{name}</h3>
          <span className="shrink-0 text-[11.5px] text-ink-faint">
            {t("guide.files.lines", { count: countLines(content) })}
          </span>
        </div>
        <CopyButton
          value={content}
          label={t("guide.files.copyLabel", { name })}
          variant="secondary"
        />
      </CardHeader>
      <pre
        tabIndex={0}
        aria-label={name}
        className={cn(
          "max-h-96 overflow-auto rounded-b-card bg-surface px-5 py-4 font-mono text-[12px] leading-[1.6] text-ink",
          FOCUS_RING
        )}
      >
        <code>{content}</code>
      </pre>
    </Card>
  );
}

/** Guide: how to create an Apps Script bridge, with both files ready to copy. */
export function Guide({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { t } = useTranslation();

  const stepActions: Partial<Record<number, ReactNode>> = {
    0: (
      <>
        <a
          href={APPS_SCRIPT_URL}
          target="_blank"
          rel="noreferrer"
          // The webview would navigate itself; hand the URL to the system browser.
          onClick={(event) => {
            event.preventDefault();
            void openExternal(APPS_SCRIPT_URL);
          }}
          className={cn(
            "inline-flex h-8 items-center gap-2 rounded-md border border-edge-strong bg-surface px-3 text-[12.5px] font-medium text-ink transition-colors hover:bg-raised",
            FOCUS_RING
          )}
        >
          <ExternalLink size={14} aria-hidden />
          {t("guide.step1.link")}
        </a>
        <CopyButton value={APPS_SCRIPT_URL} label={t("guide.step1.copyLabel")} />
      </>
    ),
    5: (
      <Button variant="secondary" size="sm" onClick={() => onNavigate("sources")}>
        {t("guide.openSources")}
        <ArrowRight size={14} aria-hidden />
      </Button>
    )
  };

  return (
    <>
      <ScreenHeader
        title={t("screen.guide.title")}
        description={t("screen.guide.description")}
        actions={
          <Button variant="outline" size="sm" onClick={() => onNavigate("sources")}>
            {t("guide.openSources")}
          </Button>
        }
      />
      <div className="space-y-4">
        <Card>
          <CardHeader>
            <CardTitle>{t("guide.intro.title")}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            <p className="text-[13px] leading-5 text-ink">{t("guide.intro.body1")}</p>
            <p className="text-[12.5px] leading-5 text-ink-muted">{t("guide.intro.body2")}</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>{t("guide.steps.title")}</CardTitle>
          </CardHeader>
          <CardContent>
            <ol className="divide-y divide-edge">
              {STEPS.map((step, index) => (
                <StepItem key={step.titleKey} index={index} step={step} action={stepActions[index]} />
              ))}
            </ol>
          </CardContent>
        </Card>

        <section aria-labelledby="guide-files-title" className="space-y-3 pt-2">
          <div>
            <h3 id="guide-files-title" className="text-[15px] font-semibold text-ink">
              {t("guide.files.title")}
            </h3>
            <p className="mt-0.5 text-[12.5px] leading-5 text-ink-muted">
              {t("guide.files.description")}
            </p>
          </div>
          {BRIDGE_FILES.map((file) => (
            <CodeFileCard key={file.name} name={file.name} content={file.content} />
          ))}
        </section>

        <Card>
          <CardHeader>
            <CardTitle>{t("guide.tips.title")}</CardTitle>
          </CardHeader>
          <CardContent>
            <ul className="divide-y divide-edge">
              {TIPS.map((tip) => (
                <li key={tip.titleKey} className="py-3 first:pt-0 last:pb-0">
                  <p className="text-[13px] font-medium text-ink">{t(tip.titleKey)}</p>
                  <p className="mt-1 text-[12.5px] leading-5 text-ink-muted">{t(tip.bodyKey)}</p>
                </li>
              ))}
            </ul>
          </CardContent>
        </Card>
      </div>
    </>
  );
}
