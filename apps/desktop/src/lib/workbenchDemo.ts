import type {
  AddSpreadsheetInput,
  GridData,
  SheetTab,
  WorkbenchFolder,
  WorkbenchItem
} from "./ipc.js";

// In-memory Workbench backend for the browser preview. Mirrors the future Rust
// commands so every Workbench interaction (folder tree, sheet tabs, editable
// grid) is fully clickable without a real Google connection. All mutations
// follow the app's immutable style: replace arrays/objects instead of mutating.

/** The subset of IpcApi the Workbench owns; spread into the demo IPC object. */
export type WorkbenchDemoApi = {
  workbenchTree(): Promise<{ folders: WorkbenchFolder[]; items: WorkbenchItem[] }>;
  createFolder(name: string): Promise<WorkbenchFolder>;
  renameFolder(id: string, name: string): Promise<void>;
  deleteFolder(id: string): Promise<void>;
  addSpreadsheet(input: AddSpreadsheetInput): Promise<WorkbenchItem>;
  removeWorkbenchItem(id: string): Promise<void>;
  moveWorkbenchItem(id: string, folderId: string | null): Promise<void>;
  listSheetTabs(itemId: string): Promise<SheetTab[]>;
  readSheet(
    itemId: string,
    gid: string,
    limit: number | null,
    offset: number | null
  ): Promise<GridData>;
  updateCell(
    itemId: string,
    gid: string,
    rowIndex: number,
    columnId: string,
    value: string
  ): Promise<void>;
  appendSheetRow(
    itemId: string,
    gid: string,
    values: Record<string, string>
  ): Promise<{ rowIndex: number }>;
};

// Mirrors demoData's GOOGLE_SOURCE_ID; the demo spreadsheets all belong to it.
const WORKBENCH_SOURCE_ID = "google-sheets";
const READ_SHEET_MAX_LIMIT = 500;

/** Per-item sheet store: ordered tabs plus one grid per gid. */
type ItemSheets = {
  tabs: SheetTab[];
  grids: Record<string, GridData>;
};

/** The bijective base-26 A1 column letter for a zero-based index (0 -> "A"). */
function columnLetter(index: number): string {
  let n = index + 1;
  let label = "";
  while (n > 0) {
    const remainder = (n - 1) % 26;
    label = String.fromCharCode(65 + remainder) + label;
    n = Math.floor((n - 1) / 26);
  }
  return label;
}

/**
 * Builds a RAW GridData that mirrors Google Sheets literally: columns are the
 * A1 letters (id AND title), and the header titles become the FIRST data row
 * (row 1 is real data, not a consumed header). Every following row is a literal
 * string cell keyed by column letter.
 */
function makeGrid(headerTitles: string[], dataRows: readonly (readonly string[])[]): GridData {
  const width = Math.max(headerTitles.length, ...dataRows.map((row) => row.length), 0);
  const columns = Array.from({ length: width }, (_, index) => {
    const id = columnLetter(index);
    return { id, title: id };
  });
  const rawRows = [headerTitles, ...dataRows];
  const rows = rawRows.map((cells) => {
    const record: Record<string, string> = {};
    columns.forEach((column, index) => {
      record[column.id] = cells[index] ?? "";
    });
    return record;
  });
  return { columns, rows, totalRows: rows.length };
}

/** Data-driven level progression so the Progression tab stays plausible. */
function progressionRows(count: number): string[][] {
  const unlocks: Record<number, string> = {
    1: "Basic Attack",
    3: "Sprint",
    5: "Skill Slot II",
    8: "Mount",
    10: "Guild Access",
    12: "Skill Slot III",
    15: "Prestige"
  };
  const rows: string[][] = [];
  let total = 0;
  for (let level = 1; level <= count; level += 1) {
    const required = Math.round(100 * Math.pow(level, 1.35));
    total += required;
    rows.push([
      String(level),
      String(required),
      String(total),
      unlocks[level] ?? "-",
      String(level * 25)
    ]);
  }
  return rows;
}

const CONSTANTS_GRID = makeGrid(
  ["Key", "Value", "Type", "Category", "Description"],
  [
    ["MAX_LEVEL", "60", "int", "progression", "Highest attainable player level"],
    ["BASE_HP", "100", "int", "combat", "Starting hit points at level 1"],
    ["HP_PER_LEVEL", "12", "int", "combat", "Hit points gained each level"],
    ["BASE_MANA", "50", "int", "combat", "Starting mana pool"],
    ["CRIT_MULTIPLIER", "1.75", "float", "combat", "Damage multiplier on critical hits"],
    ["CRIT_CHANCE", "0.15", "float", "combat", "Base critical hit probability"],
    ["MOVE_SPEED", "5.5", "float", "movement", "Default units per second"],
    ["SPRINT_MULTIPLIER", "1.6", "float", "movement", "Speed bonus while sprinting"],
    ["INVENTORY_SLOTS", "30", "int", "economy", "Default backpack capacity"],
    ["STARTING_GOLD", "250", "int", "economy", "Gold granted to new players"],
    ["RESPAWN_SECONDS", "8", "int", "combat", "Delay before respawn"],
    ["XP_CURVE_EXP", "1.35", "float", "progression", "Exponent for the XP curve"]
  ]
);

const FORMULAS_GRID = makeGrid(
  ["Name", "Expression", "Inputs", "Output", "Notes"],
  [
    ["LevelXp", "floor(BASE_XP * level ^ XP_CURVE_EXP)", "level", "xp", "Total XP for a level"],
    ["MaxHp", "BASE_HP + HP_PER_LEVEL * (level - 1)", "level", "hp", "Hit points at a level"],
    ["CritDamage", "damage * CRIT_MULTIPLIER", "damage", "damage", "Applied on a crit"],
    ["GoldReward", "base * (1 + streak * 0.1)", "base, streak", "gold", "Kill-streak bonus"],
    ["SellPrice", "round(buyPrice * 0.4)", "buyPrice", "gold", "Vendor sellback value"],
    ["DodgeChance", "clamp(agility * 0.005, 0, 0.5)", "agility", "chance", "Evasion from agility"],
    ["ArmorMitigation", "armor / (armor + 100)", "armor", "ratio", "Damage reduction curve"],
    ["ManaRegen", "BASE_MANA * 0.02 + spirit * 0.5", "spirit", "mana/s", "Mana per second"]
  ]
);

const PROGRESSION_GRID = makeGrid(
  ["Level", "Xp Required", "Total Xp", "Unlock", "Gold Bonus"],
  progressionRows(15)
);

const UNITS_GRID = makeGrid(
  ["Unit", "Role", "Hp", "Damage", "Range", "Cost", "Tier"],
  [
    ["Footman", "Melee", "220", "18", "1", "90", "T1"],
    ["Archer", "Ranged", "140", "26", "6", "120", "T1"],
    ["Knight", "Tank", "380", "22", "1", "210", "T2"],
    ["Mage", "Caster", "120", "42", "8", "260", "T2"],
    ["Cleric", "Support", "160", "10", "5", "180", "T2"],
    ["Rogue", "Melee", "150", "34", "1", "150", "T2"],
    ["Ballista", "Siege", "200", "70", "12", "340", "T3"],
    ["Griffon", "Flyer", "260", "30", "2", "300", "T3"],
    ["Paladin", "Tank", "460", "28", "1", "380", "T3"],
    ["Archmage", "Caster", "180", "60", "9", "500", "T4"]
  ]
);

const ABILITIES_GRID = makeGrid(
  ["Ability", "Unit", "Cooldown", "Mana Cost", "Effect", "Power"],
  [
    ["Shield Wall", "Knight", "18", "0", "Block 40% damage", "40"],
    ["Power Shot", "Archer", "8", "15", "Pierce armor", "55"],
    ["Fireball", "Mage", "6", "30", "AoE burn", "80"],
    ["Heal", "Cleric", "4", "20", "Restore HP", "60"],
    ["Backstab", "Rogue", "10", "10", "Bonus from behind", "120"],
    ["Divine Aura", "Paladin", "30", "45", "Party shield", "35"],
    ["Meteor", "Archmage", "45", "90", "Massive AoE", "200"],
    ["Dive", "Griffon", "12", "0", "Charge target", "45"],
    ["Siege Mode", "Ballista", "20", "0", "Double range", "0"]
  ]
);

const EVENTS_GRID = makeGrid(
  ["Event", "Start", "End", "Type", "Status", "Reward Track"],
  [
    ["Summer Festival", "2026-06-01", "2026-06-14", "seasonal", "active", "Sunlit Path"],
    ["Double XP Weekend", "2026-06-20", "2026-06-22", "boost", "scheduled", "-"],
    ["Raid: Frost Titan", "2026-07-01", "2026-07-08", "raid", "scheduled", "Glacier Cache"],
    ["Login Streak", "2026-07-01", "2026-07-31", "retention", "scheduled", "Daily Chest"],
    ["Arena Season IV", "2026-07-05", "2026-08-05", "pvp", "draft", "Champion Ladder"],
    ["Flash Sale", "2026-07-10", "2026-07-11", "store", "draft", "-"],
    ["Anniversary", "2026-08-01", "2026-08-15", "seasonal", "draft", "Cake Collection"],
    ["Guild War", "2026-08-10", "2026-08-17", "pvp", "draft", "War Banner"],
    ["Boss Rush", "2026-08-20", "2026-08-24", "event", "draft", "Trophy Vault"],
    ["Harvest Moon", "2026-09-01", "2026-09-14", "seasonal", "draft", "Autumn Path"]
  ]
);

const REWARDS_GRID = makeGrid(
  ["Tier", "Track", "Reward", "Quantity", "Currency", "Premium"],
  [
    ["1", "Sunlit Path", "Gold", "500", "gold", "no"],
    ["2", "Sunlit Path", "XP Boost", "1", "item", "no"],
    ["3", "Sunlit Path", "Summer Skin", "1", "cosmetic", "yes"],
    ["4", "Sunlit Path", "Gems", "150", "gems", "yes"],
    ["1", "Glacier Cache", "Frost Shard", "10", "material", "no"],
    ["2", "Glacier Cache", "Titan Core", "1", "material", "yes"],
    ["1", "Daily Chest", "Gold", "100", "gold", "no"],
    ["2", "Daily Chest", "Energy", "5", "item", "no"],
    ["3", "Daily Chest", "Rare Ticket", "1", "item", "yes"],
    ["1", "Champion Ladder", "Ranked Points", "50", "points", "no"]
  ]
);

const SHOP_GRID = makeGrid(
  ["Item", "Category", "Buy", "Sell", "Currency", "Stock"],
  [
    ["Health Potion", "consumable", "25", "10", "gold", "99"],
    ["Mana Potion", "consumable", "30", "12", "gold", "99"],
    ["Iron Sword", "weapon", "180", "72", "gold", "5"],
    ["Steel Shield", "armor", "220", "88", "gold", "4"],
    ["Phoenix Feather", "rare", "40", "0", "gems", "3"],
    ["Speed Elixir", "consumable", "60", "24", "gold", "20"],
    ["Mystery Box", "bundle", "100", "0", "gems", "10"],
    ["Guild Banner", "cosmetic", "500", "0", "gems", "1"],
    ["Repair Kit", "utility", "45", "18", "gold", "50"],
    ["Teleport Scroll", "utility", "75", "30", "gold", "25"]
  ]
);

const CURRENCIES_GRID = makeGrid(
  ["Currency", "Symbol", "Type", "Cap", "Exchange", "Notes"],
  [
    ["Gold", "G", "soft", "9999999", "-", "Earned in-game"],
    ["Gems", "GEM", "hard", "0", "100:1", "Premium purchase"],
    ["Energy", "EN", "stamina", "120", "-", "Regenerates over time"],
    ["Honor", "HON", "pvp", "50000", "-", "Arena currency"],
    ["Guild Marks", "GM", "guild", "0", "-", "Guild activities"],
    ["Event Tokens", "ET", "event", "0", "-", "Limited-time events"]
  ]
);

/** A tab definition paired with its grid, used to seed each spreadsheet. */
type TabSeed = { title: string; grid: GridData };

function buildItemSheets(tabs: TabSeed[]): ItemSheets {
  const sheetTabs: SheetTab[] = tabs.map((tab, index) => ({
    gid: String(index),
    title: tab.title,
    index
  }));
  const grids: Record<string, GridData> = {};
  tabs.forEach((tab, index) => {
    grids[String(index)] = tab.grid;
  });
  return { tabs: sheetTabs, grids };
}

/** A seed spreadsheet: its item metadata plus the tabs it owns. */
type ItemSeed = { item: WorkbenchItem; tabs: TabSeed[] };

const SEED_FOLDERS: WorkbenchFolder[] = [
  { id: "fld_game_config", name: "Game Config", position: 0 },
  { id: "fld_live_ops", name: "Live Ops", position: 1 }
];

const SEED_ITEMS: ItemSeed[] = [
  {
    item: {
      id: "wbi_core",
      folderId: "fld_game_config",
      sourceId: WORKBENCH_SOURCE_ID,
      spreadsheetId: "1CoreConstantsDemoSheetId",
      name: "Core Constants",
      position: 0
    },
    tabs: [
      { title: "Constants", grid: CONSTANTS_GRID },
      { title: "Formulas", grid: FORMULAS_GRID },
      { title: "Progression", grid: PROGRESSION_GRID }
    ]
  },
  {
    item: {
      id: "wbi_units",
      folderId: "fld_game_config",
      sourceId: WORKBENCH_SOURCE_ID,
      spreadsheetId: "1UnitBalanceDemoSheetId",
      name: "Unit Balance",
      position: 1
    },
    tabs: [
      { title: "Units", grid: UNITS_GRID },
      { title: "Abilities", grid: ABILITIES_GRID }
    ]
  },
  {
    item: {
      id: "wbi_events",
      folderId: "fld_live_ops",
      sourceId: WORKBENCH_SOURCE_ID,
      spreadsheetId: "1EventsCalendarDemoSheetId",
      name: "Events Calendar",
      position: 0
    },
    tabs: [
      { title: "Events", grid: EVENTS_GRID },
      { title: "Rewards", grid: REWARDS_GRID }
    ]
  },
  {
    item: {
      id: "wbi_economy",
      folderId: null,
      sourceId: WORKBENCH_SOURCE_ID,
      spreadsheetId: "1EconomySandboxDemoSheetId",
      name: "Economy Sandbox",
      position: 0
    },
    tabs: [
      { title: "Shop", grid: SHOP_GRID },
      { title: "Currencies", grid: CURRENCIES_GRID }
    ]
  }
];

/** Two small starter tabs for a freshly added spreadsheet. */
function newSpreadsheetTabs(): TabSeed[] {
  return [
    {
      title: "Sheet1",
      grid: makeGrid(
        ["Name", "Value", "Notes"],
        [
          ["example_key", "0", "Edit me or add a row"],
          ["another_key", "1", "Cells are editable"]
        ]
      )
    },
    {
      title: "Sheet2",
      grid: makeGrid(["Column A", "Column B"], [["", ""]])
    }
  ];
}

/** Extracts a spreadsheet id from a Google Sheets URL or a bare id/handle. */
function parseSpreadsheetId(urlOrId: string): string {
  const trimmed = urlOrId.trim();
  const match = trimmed.match(/\/d\/([a-zA-Z0-9-_]+)/);
  if (match && match[1]) {
    return match[1];
  }
  // Bare value: drop any trailing path, query, or fragment.
  return trimmed.split(/[/?#]/)[0] || trimmed;
}

/** Derives a friendly display name from a resolved spreadsheet id. */
function deriveSpreadsheetName(spreadsheetId: string): string {
  const shortId = spreadsheetId.slice(0, 8) || "Sheet";
  return `Spreadsheet ${shortId}`;
}

/** Deep-clones a grid so seeds are never mutated by later cell edits. */
function cloneGrid(grid: GridData): GridData {
  return {
    columns: grid.columns.map((column) => ({ ...column })),
    rows: grid.rows.map((row) => ({ ...row })),
    totalRows: grid.totalRows
  };
}

function cloneItemSheets(sheets: ItemSheets): ItemSheets {
  const grids: Record<string, GridData> = {};
  for (const [gid, grid] of Object.entries(sheets.grids)) {
    grids[gid] = cloneGrid(grid);
  }
  return { tabs: sheets.tabs.map((tab) => ({ ...tab })), grids };
}

/** Builds an isolated Workbench demo backend. `delay` reuses demoData latency. */
export function createWorkbenchDemo(deps: { delay: () => Promise<void> }): WorkbenchDemoApi {
  const { delay } = deps;

  let folders: WorkbenchFolder[] = SEED_FOLDERS.map((folder) => ({ ...folder }));
  let items: WorkbenchItem[] = SEED_ITEMS.map((seed) => ({ ...seed.item }));
  // itemId -> sheets; seeded from the item seeds, cloned so edits stay isolated.
  const sheetsByItem = new Map<string, ItemSheets>();
  for (const seed of SEED_ITEMS) {
    sheetsByItem.set(seed.item.id, buildItemSheets(seed.tabs));
  }

  let folderCounter = folders.length;
  let itemCounter = items.length;

  const byPosition = (a: { position: number }, b: { position: number }) => a.position - b.position;

  const requireItemSheets = (itemId: string): ItemSheets => {
    const sheets = sheetsByItem.get(itemId);
    if (!sheets) {
      throw new Error(`Unknown spreadsheet ${itemId}`);
    }
    return sheets;
  };

  const requireGrid = (itemId: string, gid: string): GridData => {
    const grid = requireItemSheets(itemId).grids[gid];
    if (!grid) {
      throw new Error(`Unknown sheet ${gid} in spreadsheet ${itemId}`);
    }
    return grid;
  };

  const nextPositionInFolder = (folderId: string | null): number => {
    const siblings = items.filter((item) => item.folderId === folderId);
    return siblings.reduce((max, item) => Math.max(max, item.position + 1), 0);
  };

  return {
    async workbenchTree() {
      await delay();
      return {
        folders: [...folders].sort(byPosition),
        items: [...items].sort(byPosition).map((item) => ({ ...item }))
      };
    },

    async createFolder(name: string) {
      await delay();
      const trimmed = name.trim();
      if (trimmed === "") {
        throw new Error("Folder name must not be empty");
      }
      folderCounter += 1;
      const folder: WorkbenchFolder = {
        id: `fld_new_${folderCounter}`,
        name: trimmed,
        position: folders.reduce((max, existing) => Math.max(max, existing.position + 1), 0)
      };
      folders = [...folders, folder];
      return { ...folder };
    },

    async renameFolder(id: string, name: string) {
      await delay();
      const trimmed = name.trim();
      if (trimmed === "") {
        throw new Error("Folder name must not be empty");
      }
      if (!folders.some((folder) => folder.id === id)) {
        throw new Error(`Unknown folder ${id}`);
      }
      folders = folders.map((folder) =>
        folder.id === id ? { ...folder, name: trimmed } : folder
      );
    },

    async deleteFolder(id: string) {
      await delay();
      if (!folders.some((folder) => folder.id === id)) {
        throw new Error(`Unknown folder ${id}`);
      }
      folders = folders.filter((folder) => folder.id !== id);
      // Orphaned spreadsheets fall back to Ungrouped rather than being removed.
      items = items.map((item) =>
        item.folderId === id ? { ...item, folderId: null } : item
      );
    },

    async addSpreadsheet(input: AddSpreadsheetInput) {
      await delay();
      if (input.urlOrId.trim() === "") {
        throw new Error("Paste a Google Sheets URL or spreadsheet id");
      }
      if (input.folderId !== null && !folders.some((folder) => folder.id === input.folderId)) {
        throw new Error(`Unknown folder ${input.folderId}`);
      }
      const spreadsheetId = parseSpreadsheetId(input.urlOrId);
      itemCounter += 1;
      const item: WorkbenchItem = {
        id: `wbi_new_${itemCounter}`,
        folderId: input.folderId,
        sourceId: WORKBENCH_SOURCE_ID,
        spreadsheetId,
        name: deriveSpreadsheetName(spreadsheetId),
        position: nextPositionInFolder(input.folderId)
      };
      items = [...items, item];
      sheetsByItem.set(item.id, buildItemSheets(newSpreadsheetTabs()));
      return { ...item };
    },

    async removeWorkbenchItem(id: string) {
      await delay();
      if (!items.some((item) => item.id === id)) {
        throw new Error(`Unknown spreadsheet ${id}`);
      }
      items = items.filter((item) => item.id !== id);
      sheetsByItem.delete(id);
    },

    async moveWorkbenchItem(id: string, folderId: string | null) {
      await delay();
      if (!items.some((item) => item.id === id)) {
        throw new Error(`Unknown spreadsheet ${id}`);
      }
      if (folderId !== null && !folders.some((folder) => folder.id === folderId)) {
        throw new Error(`Unknown folder ${folderId}`);
      }
      const position = nextPositionInFolder(folderId);
      items = items.map((item) =>
        item.id === id ? { ...item, folderId, position } : item
      );
    },

    async listSheetTabs(itemId: string) {
      await delay();
      return [...requireItemSheets(itemId).tabs]
        .sort((a, b) => a.index - b.index)
        .map((tab) => ({ ...tab }));
    },

    async readSheet(itemId: string, gid: string, limit: number | null, offset: number | null) {
      await delay();
      const grid = requireGrid(itemId, gid);
      const effectiveOffset = offset ?? 0;
      const effectiveLimit = Math.min(limit ?? grid.rows.length, READ_SHEET_MAX_LIMIT);
      const page = grid.rows
        .slice(effectiveOffset, effectiveOffset + effectiveLimit)
        .map((row) => ({ ...row }));
      return {
        columns: grid.columns.map((column) => ({ ...column })),
        rows: page,
        totalRows: grid.rows.length
      };
    },

    async updateCell(
      itemId: string,
      gid: string,
      rowIndex: number,
      columnId: string,
      value: string
    ) {
      await delay();
      const sheets = requireItemSheets(itemId);
      const grid = requireGrid(itemId, gid);
      if (rowIndex < 0 || rowIndex >= grid.rows.length) {
        throw new Error(`Row ${rowIndex} is out of range`);
      }
      if (!grid.columns.some((column) => column.id === columnId)) {
        throw new Error(`Unknown column ${columnId}`);
      }
      const rows = grid.rows.map((row, index) =>
        index === rowIndex ? { ...row, [columnId]: value } : row
      );
      const nextGrid: GridData = { ...grid, rows };
      sheetsByItem.set(itemId, { ...sheets, grids: { ...sheets.grids, [gid]: nextGrid } });
    },

    async appendSheetRow(itemId: string, gid: string, values: Record<string, string>) {
      await delay();
      const sheets = requireItemSheets(itemId);
      const grid = requireGrid(itemId, gid);
      const newRow: Record<string, string> = {};
      for (const column of grid.columns) {
        newRow[column.id] = values[column.id] ?? "";
      }
      const rows = [...grid.rows, newRow];
      const nextGrid: GridData = { ...grid, rows, totalRows: rows.length };
      sheetsByItem.set(itemId, { ...sheets, grids: { ...sheets.grids, [gid]: nextGrid } });
      return { rowIndex: rows.length - 1 };
    }
  };
}

/** Exported for tests and isolated demo instances. */
export const _internal = {
  parseSpreadsheetId,
  deriveSpreadsheetName,
  makeGrid,
  columnLetter,
  cloneItemSheets
};
