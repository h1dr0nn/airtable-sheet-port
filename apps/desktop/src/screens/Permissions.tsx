import { useState } from "react";
import { Button, EmptyState, Skeleton } from "@sheet-port/ui";
import { usePermissionRules } from "../hooks/usePermissions.js";
import { RuleFormDialog } from "../components/permissions/RuleFormDialog.js";
import { RuleRow } from "../components/permissions/RuleRow.js";
import { ScreenHeader } from "../components/ScreenHeader.js";

export function Permissions() {
  const { data: rules, isPending } = usePermissionRules();
  const [isFormOpen, setIsFormOpen] = useState(false);
  const list = rules ?? [];

  return (
    <>
      <ScreenHeader
        title="Permissions"
        description="What agents may read or write, and which actions need your confirmation"
        actions={
          <Button size="sm" onClick={() => setIsFormOpen(true)}>
            Add rule
          </Button>
        }
      />

      {isPending ? (
        <div className="space-y-4">
          <Skeleton className="h-32 rounded-card" />
          <Skeleton className="h-32 rounded-card" />
        </div>
      ) : list.length === 0 ? (
        <EmptyState
          title="No rules yet"
          description="Agents are denied by default. Add a rule to grant scoped access"
          action={
            <Button size="sm" onClick={() => setIsFormOpen(true)}>
              Add rule
            </Button>
          }
        />
      ) : (
        <div className="space-y-4">
          {list.map((rule) => (
            <RuleRow key={rule.id} rule={rule} />
          ))}
        </div>
      )}

      <RuleFormDialog open={isFormOpen} onOpenChange={setIsFormOpen} />
    </>
  );
}
