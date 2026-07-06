import { Plus, ShieldCheck } from "lucide-react";
import { useState } from "react";
import { Button, EmptyState, Skeleton } from "@sheet-port/ui";
import { usePermissionRules } from "../hooks/usePermissions.js";
import { RuleFormDialog } from "../components/permissions/RuleFormDialog.js";
import { RuleRow } from "../components/permissions/RuleRow.js";
import { ScreenHeader } from "../components/ScreenHeader.js";

const EMPTY_STATE_ICON_SIZE = 22;

export function Permissions() {
  const { data: rules, isPending } = usePermissionRules();
  const [isFormOpen, setIsFormOpen] = useState(false);

  return (
    <>
      <ScreenHeader
        title="Permissions"
        description="What agents may read or write, and which actions need your confirmation."
        actions={
          <Button size="sm" onClick={() => setIsFormOpen(true)}>
            <Plus size={13} aria-hidden />
            Add rule
          </Button>
        }
      />

      {isPending ? (
        <div className="space-y-3">
          <Skeleton className="h-32" />
          <Skeleton className="h-32" />
        </div>
      ) : (rules ?? []).length === 0 ? (
        <EmptyState
          icon={<ShieldCheck size={EMPTY_STATE_ICON_SIZE} aria-hidden />}
          title="No permission rules"
          description="Agents are denied by default. Add a rule to grant scoped access."
          action={
            <Button size="sm" onClick={() => setIsFormOpen(true)}>
              <Plus size={13} aria-hidden />
              Add rule
            </Button>
          }
        />
      ) : (
        <div className="space-y-3">
          {(rules ?? []).map((rule) => (
            <RuleRow key={rule.id} rule={rule} />
          ))}
        </div>
      )}

      <RuleFormDialog open={isFormOpen} onOpenChange={setIsFormOpen} />
    </>
  );
}
