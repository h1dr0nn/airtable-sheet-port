import type { AuditEvent } from "@sheet-port/shared";

const DEFAULT_AUDIT_LIMIT = 100;

/** Persistence abstraction implemented by @sheet-port/storage. */
export interface AuditStorePort {
  insert(event: AuditEvent): void;
  /** Newest first. */
  list(limit: number, offset: number): AuditEvent[];
}

export class AuditService {
  constructor(private readonly store: AuditStorePort) {}

  record(event: Omit<AuditEvent, "id" | "timestamp">): AuditEvent {
    const auditEvent: AuditEvent = {
      ...event,
      id: `evt_${crypto.randomUUID()}`,
      timestamp: new Date().toISOString()
    };
    this.store.insert(auditEvent);
    return auditEvent;
  }

  list(limit = DEFAULT_AUDIT_LIMIT, offset = 0): AuditEvent[] {
    return this.store.list(limit, offset);
  }
}
