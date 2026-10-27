import { describe, expect, it } from "vitest";
import { DEMO, handle } from "./mock";
import type { Label, ThreadRow } from "./types";

describe("demo mailbox", () => {
  it("seeds an inbox with unread mail, labels, and an OTP thread", async () => {
    const labels = (await handle("list_labels")) as Label[];
    const inbox = (await handle("list_threads", { labelId: "INBOX" })) as ThreadRow[];
    const clients = (await handle("list_threads", { labelId: "Label_1" })) as ThreadRow[];
    const otp = inbox.find((t) => t.threadId === DEMO.otpThreadId);

    expect(otp?.otpCode).toBe(DEMO.otpCode);
    expect(otp?.fromName).toBe(DEMO.otpFrom);
    expect(otp?.fromEmail).toMatch(/\.example$/);
    expect(inbox.some((t) => t.unread)).toBe(true);
    expect(labels.find((l) => l.id === "INBOX")?.unread).toBeGreaterThan(0);
    expect(labels.find((l) => l.name === "Clients")).toBeTruthy();
    expect(labels.find((l) => l.name === "Receipts")).toBeTruthy();
    expect(clients.length).toBeGreaterThan(0);
    expect(inbox.every((t) => !/@gmail\.com|github\.com|instagram\.com|amazon\./i.test(t.fromEmail))).toBe(true);
  });

  it("finds the OTP mail with local search", async () => {
    const rows = (await handle("search", { query: "northwind" })) as ThreadRow[];
    expect(rows.some((t) => t.otpCode === DEMO.otpCode)).toBe(true);
  });
});
