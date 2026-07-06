import type { AuditEvent } from "@sheet-port/shared";

export class AuditService {
  private readonly events: AuditEvent[] = [];

  record(event: Omit<AuditEvent, "id" | "timestamp">): AuditEvent {
    const auditEvent: AuditEvent = {
      ...event,
      id: `evt_${crypto.randomUUID()}`,
      timestamp: new Date().toISOString()
    };
    this.events.unshift(auditEvent);
    return auditEvent;
  }

  list(limit = 100): AuditEvent[] {
    return this.events.slice(0, limit);
  }
}
