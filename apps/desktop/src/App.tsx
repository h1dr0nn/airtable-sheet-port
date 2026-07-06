import { Badge, Button, Panel } from "@sheet-port/ui";
import { createColumnHelper, flexRender, getCoreRowModel, useReactTable } from "@tanstack/react-table";
import { Activity, CheckCircle2, Database, FileClock, KeyRound, LayoutDashboard, Plug, ShieldCheck, Table2 } from "lucide-react";
import { useMemo, useState } from "react";
import type { TableRecord } from "@sheet-port/shared";
import { auditEvents, pendingChanges, records, rules, schema, sources } from "./mockData.js";

const navItems = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "sources", label: "Data Sources", icon: Plug },
  { id: "tables", label: "Tables", icon: Table2 },
  { id: "permissions", label: "Permissions", icon: ShieldCheck },
  { id: "changes", label: "Changes", icon: FileClock },
  { id: "audit", label: "Audit Log", icon: Activity }
] as const;

type ViewId = (typeof navItems)[number]["id"];

export function App() {
  const [view, setView] = useState<ViewId>("dashboard");

  return (
    <main className="min-h-screen bg-slate-50 text-slate-950">
      <div className="grid min-h-screen grid-cols-[260px_1fr]">
        <aside className="border-r border-slate-200 bg-white px-4 py-5">
          <div className="mb-8 flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-emerald-600 text-white">
              <Database size={20} />
            </div>
            <div>
              <h1 className="text-lg font-semibold">Airtable - Sheet Port</h1>
              <p className="text-xs text-slate-500">Safe local table access</p>
            </div>
          </div>
          <nav className="space-y-1">
            {navItems.map((item) => {
              const Icon = item.icon;
              const active = view === item.id;
              return (
                <button
                  key={item.id}
                  className={`flex w-full items-center gap-3 rounded-md px-3 py-2 text-left text-sm font-medium ${
                    active ? "bg-emerald-50 text-emerald-900" : "text-slate-600 hover:bg-slate-50 hover:text-slate-950"
                  }`}
                  onClick={() => setView(item.id)}
                >
                  <Icon size={17} />
                  {item.label}
                </button>
              );
            })}
          </nav>
        </aside>
        <section className="overflow-auto px-8 py-7">
          {view === "dashboard" && <Dashboard />}
          {view === "sources" && <DataSources />}
          {view === "tables" && <Tables />}
          {view === "permissions" && <Permissions />}
          {view === "changes" && <Changes />}
          {view === "audit" && <AuditLog />}
        </section>
      </div>
    </main>
  );
}

function PageTitle({ title, subtitle }: { title: string; subtitle: string }) {
  return (
    <header className="mb-6">
      <h2 className="text-2xl font-semibold tracking-normal">{title}</h2>
      <p className="mt-1 text-sm text-slate-600">{subtitle}</p>
    </header>
  );
}

function Dashboard() {
  return (
    <>
      <PageTitle title="Dashboard" subtitle="Local broker status for agent access to spreadsheets and tables." />
      <div className="grid gap-4 md:grid-cols-3">
        <Panel>
          <div className="flex items-center justify-between">
            <p className="text-sm font-medium text-slate-600">MCP server</p>
            <Badge className="border-emerald-200 bg-emerald-50 text-emerald-700">stdio ready</Badge>
          </div>
          <p className="mt-4 text-2xl font-semibold">Local only</p>
        </Panel>
        <Panel>
          <div className="flex items-center justify-between">
            <p className="text-sm font-medium text-slate-600">Connector</p>
            <Badge>mock</Badge>
          </div>
          <p className="mt-4 text-2xl font-semibold">1 source</p>
        </Panel>
        <Panel>
          <div className="flex items-center justify-between">
            <p className="text-sm font-medium text-slate-600">Pending changes</p>
            <Badge className="border-amber-200 bg-amber-50 text-amber-700">review</Badge>
          </div>
          <p className="mt-4 text-2xl font-semibold">{pendingChanges.length}</p>
        </Panel>
      </div>
      <Panel className="mt-4">
        <h3 className="mb-3 text-sm font-semibold">Capability boundary</h3>
        <div className="grid gap-3 md:grid-cols-3">
          {["Tokens stay local", "Preview before commit", "Audit every write"].map((label) => (
            <div key={label} className="flex items-center gap-2 rounded-md border border-slate-200 px-3 py-2 text-sm">
              <CheckCircle2 className="text-emerald-600" size={16} />
              {label}
            </div>
          ))}
        </div>
      </Panel>
    </>
  );
}

function DataSources() {
  return (
    <>
      <PageTitle title="Data Sources" subtitle="Connected sources and provider placeholders." />
      <div className="grid gap-4 md:grid-cols-3">
        {sources.map((sourceItem) => (
          <Panel key={sourceItem.id}>
            <div className="flex items-center justify-between">
              <h3 className="font-semibold">{sourceItem.name}</h3>
              <Badge>{sourceItem.kind}</Badge>
            </div>
            <p className="mt-3 text-sm text-slate-600">{sourceItem.kind === "mock" ? "Available for development." : "Auth flow planned."}</p>
            <Button className="mt-4 w-full" disabled={sourceItem.kind !== "mock"}>
              <KeyRound size={16} className="mr-2" />
              {sourceItem.kind === "mock" ? "Connected" : "Connect"}
            </Button>
          </Panel>
        ))}
      </div>
    </>
  );
}

function Tables() {
  const columnHelper = createColumnHelper<TableRecord>();
  const columns = useMemo(
    () => [
      columnHelper.accessor("id", { header: "Record ID", cell: (info) => info.getValue() }),
      ...schema.fields.map((field) =>
        columnHelper.accessor((row) => row.fields[field.name], {
          id: field.name,
          header: field.name,
          cell: (info) => String(info.getValue() ?? "")
        })
      )
    ],
    [columnHelper]
  );
  const tableModel = useReactTable({ data: records, columns, getCoreRowModel: getCoreRowModel() });

  return (
    <>
      <PageTitle title="Tables" subtitle="Mock table preview through the shared table abstraction." />
      <Panel>
        <div className="mb-4 flex items-center justify-between">
          <div>
            <h3 className="font-semibold">{schema.name}</h3>
            <p className="text-sm text-slate-600">{schema.fields.length} fields, {records.length} records</p>
          </div>
          <Badge>mock-source</Badge>
        </div>
        <div className="overflow-hidden rounded-md border border-slate-200">
          <table className="w-full border-collapse text-sm">
            <thead className="bg-slate-100 text-left">
              {tableModel.getHeaderGroups().map((headerGroup) => (
                <tr key={headerGroup.id}>
                  {headerGroup.headers.map((header) => (
                    <th key={header.id} className="border-b border-slate-200 px-3 py-2 font-semibold">
                      {flexRender(header.column.columnDef.header, header.getContext())}
                    </th>
                  ))}
                </tr>
              ))}
            </thead>
            <tbody>
              {tableModel.getRowModel().rows.map((row) => (
                <tr key={row.id} className="odd:bg-white even:bg-slate-50">
                  {row.getVisibleCells().map((cell) => (
                    <td key={cell.id} className="border-b border-slate-100 px-3 py-2">
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Panel>
    </>
  );
}

function Permissions() {
  return (
    <>
      <PageTitle title="Permissions" subtitle="Base rules for read, write, delete, and confirmation policy." />
      <div className="space-y-4">
        {rules.map((rule) => (
          <Panel key={`${rule.sourceId}:${rule.tableId ?? "*"}`}>
            <div className="flex items-center justify-between">
              <h3 className="font-semibold">{rule.sourceId}/{rule.tableId ?? "*"}</h3>
              <Badge>{rule.write ? "write allowed" : "read only"}</Badge>
            </div>
            <div className="mt-4 grid gap-3 md:grid-cols-4">
              {[
                ["Read", rule.read],
                ["Write", rule.write],
                ["Delete", rule.deleteRecords],
                ["Confirmation", rule.requireConfirmationFor.length > 0]
              ].map(([label, enabled]) => (
                <label key={String(label)} className="flex items-center gap-2 rounded-md border border-slate-200 px-3 py-2 text-sm">
                  <input type="checkbox" checked={Boolean(enabled)} readOnly className="h-4 w-4 accent-emerald-600" />
                  {label}
                </label>
              ))}
            </div>
          </Panel>
        ))}
      </div>
    </>
  );
}

function Changes() {
  return (
    <>
      <PageTitle title="Changes" subtitle="Pending write previews before commit." />
      <div className="space-y-4">
        {pendingChanges.map((change) => (
          <Panel key={change.id}>
            <div className="flex items-center justify-between">
              <div>
                <h3 className="font-semibold">{change.id}</h3>
                <p className="text-sm text-slate-600">{change.type} on {change.sourceId}/{change.tableId}</p>
              </div>
              <Badge className="border-amber-200 bg-amber-50 text-amber-700">{change.status}</Badge>
            </div>
            <pre className="mt-4 overflow-auto rounded-md bg-slate-950 p-3 text-xs text-slate-100">{JSON.stringify(change.diff, null, 2)}</pre>
            <div className="mt-4 flex gap-2">
              <Button>Approve mock</Button>
              <Button className="text-slate-600">Reject mock</Button>
            </div>
          </Panel>
        ))}
      </div>
    </>
  );
}

function AuditLog() {
  return (
    <>
      <PageTitle title="Audit Log" subtitle="Recent tool calls and write-related actions." />
      <Panel>
        <div className="divide-y divide-slate-100">
          {auditEvents.map((event) => (
            <div key={event.id} className="grid grid-cols-[190px_1fr_auto] gap-4 py-3 text-sm">
              <span className="text-slate-500">{new Date(event.timestamp).toLocaleString()}</span>
              <span className="font-medium">{event.action}</span>
              <Badge>{event.actor}</Badge>
            </div>
          ))}
        </div>
      </Panel>
    </>
  );
}
