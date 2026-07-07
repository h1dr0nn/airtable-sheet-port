import { EmptyState } from "@sheet-port/ui";
import { useTranslation } from "../i18n/useTranslation.js";
import { ScreenHeader } from "../components/ScreenHeader.js";

/**
 * Placeholder Tables screen. The editable spreadsheet-style workbench (source
 * selector + table list + records grid) will replace this later; for now the
 * screen keeps its nav item and route but shows a centered "Coming Soon" panel.
 */
export function Tables() {
  const { t } = useTranslation();

  return (
    <>
      <ScreenHeader
        title={t("screen.tables.title")}
        description={t("screen.tables.description")}
      />
      <div className="flex min-h-[50vh] items-center justify-center">
        <EmptyState
          title={t("tables.comingSoonTitle")}
          description={t("tables.comingSoonSubtitle")}
        />
      </div>
    </>
  );
}
