import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { displayAdvice, DisplayInputs, positiveNumber } from "../src/lib/displayAdvice";

const mocks = vi.hoisted(() => ({ recoveryTimeline: vi.fn(), recoveryUndo: vi.fn(), networkStart: vi.fn(), networkStatus: vi.fn(), networkCancel: vi.fn() }));
vi.mock("../src/api", () => ({ api: mocks }));
vi.mock("../src/i18n", () => ({ useLang: () => ({ t: (key: string) => key, lang: "de" }) }));
import { RecoveryTimeline } from "../src/components/RecoveryTimeline";
import { NetworkCard } from "../src/components/diag/NetworkCard";
import { DisplayAssistant } from "../src/components/diag/DisplayAssistant";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(yes => { resolve = yes; });
  return { promise, resolve };
}
beforeEach(() => {
  Object.values(mocks).forEach(mock => mock.mockReset());
  mocks.networkCancel.mockResolvedValue(undefined);
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

const base: DisplayInputs = { vrr: "unknown", gameVsync: "unknown", driverVsync: "unknown", driverQueue: "unknown", frameGen: "unknown", limiter: "unknown", latency: "unknown", mode: "unknown", api: "unknown", goal: "smooth", cap: null, hz: 144, belowMax: false };
describe("conditional display advice", () => {
  it("does not infer active VRR or a measured latency from desktop refresh", () => {
    expect(displayAdvice(base)).toEqual(["displayAdvice_unknown", "displayAdvice_missing"]);
    render(<DisplayAssistant monitors={[{ id: "screen", name: "Test", hz: 144, maxHz: 144 }]} />);
    expect(screen.getByLabelText("displayVrr")).toHaveProperty("value", "unknown");
    expect(screen.getByText("displayInterval")).toBeTruthy();
    expect(screen.queryByText("displayAdvice_fg")).toBeNull();
    fireEvent.change(screen.getByLabelText("displayFrameGen"), { target: { value: "on" } });
    expect(screen.getByText("displayAdvice_fg")).toBeTruthy();
  });
  it("recognizes a VRR ceiling conflict without treating an automatic Reflex cap as manual", () => {
    expect(displayAdvice({ ...base, vrr: "on", limiter: "game", cap: 144 })).toContain("displayAdvice_cap");
    const reflex = displayAdvice({ ...base, vrr: "on", limiter: "automatic", cap: 144, latency: "reflex", driverQueue: "on", api: "dx12" });
    expect(reflex).not.toContain("displayAdvice_cap");
    expect(reflex).toEqual(expect.arrayContaining(["displayAdvice_auto", "displayAdvice_reflex", "displayAdvice_modernApi"]));
    expect(displayAdvice({ ...base, limiter: "multiple" })).toContain("displayAdvice_multiple");
    expect(positiveNumber("")).toBeNull();
    expect(positiveNumber("Infinity")).toBeNull();
    expect(positiveNumber("-60")).toBeNull();
  });
});

describe("recovery timeline", () => {
  const session = { id: "session-1", label: "My session", time: "2026-11-13T12:00:00Z", legacy: false, entries: [{ id: "entry", name: "Setting", status: "applied", restored: 0, attempted: 2, items: [{ resource: "HKCU/test", before: { kind: "Registry", value: null }, target: { kind: "Registry", value: { t: "Dword", v: 1 } }, current: { kind: "Registry", value: { t: "Dword", v: 2 } }, status: "conflict" }] }] };
  it("shows drift and reloads persisted partial progress after a rejected undo", async () => {
    mocks.recoveryTimeline.mockResolvedValueOnce({ sessions: [session] }).mockResolvedValueOnce({ sessions: [{ ...session, entries: [{ ...session.entries[0], status: "partial", restored: 1 }] }] });
    mocks.recoveryUndo.mockRejectedValue(new Error("disk write failed"));
    vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<RecoveryTimeline />);
    expect(await screen.findByText("My session")).toBeTruthy();
    expect(screen.getByText("recovery_conflict")).toBeTruthy();
    expect(screen.getByText("Dword: 2")).toBeTruthy();
    fireEvent.click(screen.getByText("recoveryUndo"));
    await screen.findByText("Error: disk write failed");
    expect(mocks.recoveryUndo).toHaveBeenCalledWith("session-1");
    expect(mocks.recoveryTimeline).toHaveBeenCalledTimes(2);
    expect(screen.getByText(/recovery_partial/)).toBeTruthy();
    expect(screen.getByText(/1\/2/)).toBeTruthy();
  });
});

describe("network job ownership", () => {
  it("never runs a probe on mount and requires separate load confirmation", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<NetworkCard />);
    expect(mocks.networkStart).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("networkLoad"));
    expect(window.confirm).toHaveBeenCalledWith("networkConsent");
    expect(mocks.networkStart).not.toHaveBeenCalled();
  });
  it("cancels a late start after unmount without scheduling status polling", async () => {
    const start = deferred<string>(); mocks.networkStart.mockReturnValue(start.promise);
    const view = render(<NetworkCard />);
    fireEvent.click(screen.getByText("networkIdle"));
    view.unmount();
    await act(async () => start.resolve("late-job"));
    expect(mocks.networkCancel).toHaveBeenCalledWith("late-job");
    expect(mocks.networkStatus).not.toHaveBeenCalled();
  });
  it("offers cancellation while the first status request is still pending", async () => {
    mocks.networkStart.mockResolvedValue("running-job");
    mocks.networkStatus.mockReturnValue(new Promise(() => {}));
    render(<NetworkCard />);
    fireEvent.click(screen.getByText("networkIdle"));
    await waitFor(() => expect(mocks.networkStatus).toHaveBeenCalledWith("running-job"));
    const cancel = screen.getByRole("button", { name: "networkCancel" });
    expect(cancel).toHaveProperty("disabled", false);
    fireEvent.click(cancel);
    expect(mocks.networkCancel).toHaveBeenCalledWith("running-job");
  });
  it("does not restart polling after a late status response following unmount", async () => {
    vi.useFakeTimers();
    const status = deferred<{ done: boolean; result: null; error: null }>();
    mocks.networkStart.mockResolvedValue("job"); mocks.networkStatus.mockReturnValue(status.promise);
    const view = render(<NetworkCard />);
    await act(async () => { fireEvent.click(screen.getByText("networkIdle")); });
    view.unmount();
    await act(async () => { status.resolve({ done: false, result: null, error: null }); await vi.advanceTimersByTimeAsync(2000); });
    expect(mocks.networkStatus).toHaveBeenCalledTimes(1);
    expect(mocks.networkCancel).toHaveBeenCalledWith("job");
  });
});
