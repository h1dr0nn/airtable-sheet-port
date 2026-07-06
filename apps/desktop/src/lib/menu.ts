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
};

const THEME_LABELS: ReadonlyArray<{ value: ThemeSetting; label: string }> = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "system", label: "System" }
];

export function buildAppMenu(deps: AppMenuDeps): readonly MenuEntry[] {
  const fileItems: MenuEntry[] = [
    { kind: "action", id: "reload-data", label: "Reload data", run: deps.reloadData }
  ];
  if (deps.quit !== null) {
    const quit = deps.quit;
    fileItems.push(
      { kind: "separator", id: "file-sep" },
      { kind: "action", id: "quit", label: "Quit", run: quit }
    );
  }

  return [
    { kind: "submenu", id: "file", label: "File", items: fileItems },
    {
      kind: "submenu",
      id: "view",
      label: "View",
      items: [
        {
          kind: "submenu",
          id: "theme",
          label: "Theme",
          items: THEME_LABELS.map(
            ({ value, label }): MenuActionEntry => ({
              kind: "action",
              id: `theme-${value}`,
              label,
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
            label: item.label,
            run: () => deps.navigate(item.screen)
          })
        )
      ]
    },
    {
      kind: "submenu",
      id: "help",
      label: "Help",
      items: [
        {
          kind: "action",
          id: "about",
          label: "About",
          run: () => deps.navigate("settings")
        },
        { kind: "action", id: "copy-version", label: "Copy version", run: deps.copyVersion }
      ]
    }
  ];
}
