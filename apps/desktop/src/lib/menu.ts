import type { TFunction } from "../i18n/useTranslation.js";
import { NAV, type ScreenId } from "./nav.js";
import type { ThemeSetting } from "./theme.js";

// Data-driven titlebar menu: the Titlebar renders these entries recursively,
// so adding a menu item never touches component code.

export type MenuActionEntry = {
  kind: "action";
  id: string;
  label: string;
  run: () => void;
  /** Radio-style marker, e.g. the active theme. */
  checked?: boolean;
};

export type MenuSubmenuEntry = {
  kind: "submenu";
  id: string;
  label: string;
  items: readonly MenuEntry[];
};

export type MenuSeparatorEntry = {
  kind: "separator";
  id: string;
};

export type MenuEntry = MenuActionEntry | MenuSubmenuEntry | MenuSeparatorEntry;

export type AppMenuDeps = {
  navigate: (screen: ScreenId) => void;
  reloadData: () => void;
  /** Only offered when running inside Tauri; closing has no browser analogue. */
  quit: (() => void) | null;
  themeSetting: ThemeSetting;
  setTheme: (setting: ThemeSetting) => void;
  copyVersion: () => void;
  /** Active-language translator, so menu labels track the language setting. */
  t: TFunction;
};

const THEME_LABELS: ReadonlyArray<{ value: ThemeSetting; labelKey: "theme.light" | "theme.dark" | "theme.system" }> = [
  { value: "light", labelKey: "theme.light" },
  { value: "dark", labelKey: "theme.dark" },
  { value: "system", labelKey: "theme.system" }
];

export function buildAppMenu(deps: AppMenuDeps): readonly MenuEntry[] {
  const { t } = deps;
  const fileItems: MenuEntry[] = [
    { kind: "action", id: "reload-data", label: t("menu.reloadData"), run: deps.reloadData }
  ];
  if (deps.quit !== null) {
    const quit = deps.quit;
    fileItems.push(
      { kind: "separator", id: "file-sep" },
      { kind: "action", id: "quit", label: t("menu.quit"), run: quit }
    );
  }

  return [
    { kind: "submenu", id: "file", label: t("menu.file"), items: fileItems },
    {
      kind: "submenu",
      id: "view",
      label: t("menu.view"),
      items: [
        {
          kind: "submenu",
          id: "theme",
          label: t("menu.theme"),
          items: THEME_LABELS.map(
            ({ value, labelKey }): MenuActionEntry => ({
              kind: "action",
              id: `theme-${value}`,
              label: t(labelKey),
              checked: deps.themeSetting === value,
              run: () => deps.setTheme(value)
            })
          )
        },
        { kind: "separator", id: "view-sep" },
        ...NAV.map(
          (item): MenuActionEntry => ({
            kind: "action",
            id: `nav-${item.id}`,
            label: t(item.labelKey),
            run: () => deps.navigate(item.screen)
          })
        )
      ]
    },
    {
      kind: "submenu",
      id: "help",
      label: t("menu.help"),
      items: [
        {
          kind: "action",
          id: "about",
          label: t("menu.about"),
          run: () => deps.navigate("settings")
        },
        { kind: "action", id: "copy-version", label: t("menu.copyVersion"), run: deps.copyVersion }
      ]
    }
  ];
}
