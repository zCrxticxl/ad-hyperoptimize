import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

const { invoke, unlisten, checkUpdate } = vi.hoisted(() => ({ invoke: vi.fn(), unlisten: vi.fn(), checkUpdate: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => unlisten) }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: checkUpdate }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn(async () => "1.7.0") }));
vi.mock("../src/i18n", async importOriginal => ({
  ...await importOriginal<typeof import("../src/i18n")>(),
  useLang: () => ({ lang: "de", setLang: vi.fn(), t: (key: string) => key }),
}));
vi.mock("../src/pages/Dashboard", () => ({ default: () => <div>dashboard-body</div> }));
vi.mock("../src/components/FpsMeasure", () => ({ FpsMeasure: () => null }));
vi.mock("../src/components/HwWarnings", () => ({ HwWarnings: () => null }));
vi.mock("recharts", async importOriginal => ({
  ...await importOriginal<typeof import("recharts")>(),
  ResponsiveContainer: () => null,
}));
import App from "../src/App";
import HwMonitor from "../src/pages/HwMonitor";
import Monitor from "../src/pages/Monitor";
import { api } from "../src/api";
import { pendingOperations } from "../src/lib/pendingOperations";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

beforeEach(() => {
  localStorage.setItem("ui.onboarded", "1");
  localStorage.setItem("ui.mode", "expert");
  Element.prototype.scrollIntoView = vi.fn();
  vi.stubGlobal("CSS", { escape: (s: string) => s.replace(/["\\]/g, "\\$&") });
  checkUpdate.mockReset().mockResolvedValue(null);
  invoke.mockReset();
  unlisten.mockReset();
  invoke.mockImplementation(async (command: string) => {
    if (command === "cmd_is_admin") return true;
    if (command === "cmd_broker_status") return { available: false, connected: false };
    if (command === "cmd_wpr_start") return "recording";
    if (command === "cmd_proc_list") return { processes: [{ pid: 42, name: "game.exe", cpu: 1, memMb: 20 }] };
    if (command === "cmd_startup_list") return { items: [
      { scope: "hkcu_run", name: "Discord", enabled: true },
      { scope: "hklm_run", name: "Discord", enabled: true },
    ] };
    if (command === "cmd_schedtasks_list") return { tasks: [
      { path: "\\One\\", name: "Backup", enabled: true, isBloat: false, origin: "thirdparty" },
      { path: "\\Two\\", name: "Backup", enabled: true, isBloat: false, origin: "thirdparty" },
    ] };
    if (["cmd_gpu_scan", "cmd_privacy_scan", "cmd_debloater_tweaks_list"].includes(command)) return { tweaks: [] };
    if (command === "cmd_services_list") return { services: [] };
    return [];
  });
});
afterEach(() => { cleanup(); vi.useRealTimers(); expect(pendingOperations.getSnapshot()).toBe(0); });

async function openBenchmark() {
  render(<App />);
  fireEvent.change(screen.getByRole("textbox", { name: "navSearchTools" }), { target: { value: "benchmark" } });
  fireEvent.click(await screen.findByRole("button", { name: /navBenchmark/ }));
  await screen.findByText("benchTitle");
}

describe("navigation with the real API tracker and mocked Tauri", () => {
  it.each(["resolve", "reject"])("keeps benchmark feedback mounted until %s", async outcome => {
    const work = deferred<any>();
    const fallback = invoke.getMockImplementation()!;
    invoke.mockImplementation((command: string, ...args: any[]) => command === "cmd_run_benchmark" ? work.promise : fallback(command, ...args));
    await openBenchmark();
    fireEvent.click(screen.getAllByRole("button", { name: "benchRunButton" })[0]);
    fireEvent.click(screen.getByTitle("navDashboard"));
    expect(screen.getByText(/Aktion läuft/)).toBeTruthy();
    expect(screen.getByText("benchTitle")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "beginner", exact: true }));
    expect(localStorage.getItem("ui.mode")).toBe("expert");
    await act(async () => {
      if (outcome === "resolve") work.resolve({ kind: "cpu", singleMBs: 123, multiMBs: 456, threads: 2, scaling: 3 });
      else work.reject(new Error("benchmark failed"));
    });
    expect(screen.getByText(outcome === "resolve" ? "123 MB/s" : /benchmark failed/)).toBeTruthy();
    fireEvent.click(screen.getByTitle("navDashboard"));
    expect(screen.getByText("dashboard-body")).toBeTruthy();
  });

  it("keeps the page mounted while searching and targets distinct duplicate names", async () => {
    render(<App />);
    const errors = vi.spyOn(console, "error");
    const search = screen.getByRole("textbox", { name: "navSearchTools" });
    fireEvent.change(search, { target: { value: "Discord" } });
    const results = await screen.findAllByText("Discord", { selector: ".tool-card b" });
    expect(results).toHaveLength(2);
    fireEvent.click(results[1]);
    await waitFor(() => expect(document.querySelector('.feature-focus')?.getAttribute('data-focus-id')).toBe(JSON.stringify(["hklm_run", "Discord"])));
    fireEvent.change(search, { target: { value: "Discord" } });
    fireEvent.click((await screen.findAllByText("Discord", { selector: ".tool-card b" }))[0]);
    await waitFor(() => expect(document.querySelector('.feature-focus')?.getAttribute('data-focus-id')).toBe(JSON.stringify(["hkcu_run", "Discord"])));
    fireEvent.change(search, { target: { value: "Backup" } });
    fireEvent.click((await screen.findAllByRole("button", { name: /Backup/ }))[1]);
    await waitFor(() => expect(document.querySelector('.feature-focus')?.getAttribute('data-focus-id')).toBe(JSON.stringify(["\\Two\\", "Backup"])));
    expect(errors.mock.calls.filter(args => String(args[0]).includes("same key"))).toEqual([]);
  });

  it("tracks installation through the authoritative command promise", async () => {
    const work = deferred<void>();
    invoke.mockReturnValue(work.promise);
    const result = api.swInstall(["Example.App"]);
    expect(pendingOperations.getSnapshot()).toBe(1);
    work.resolve();
    await result;
    expect(pendingOperations.getSnapshot()).toBe(0);
  });

  it("retains WPR controls after navigation and a failed cancellation", async () => {
    render(<App />);
    const search = screen.getByRole("textbox", { name: "navSearchTools" });
    fireEvent.change(search, { target: { value: "latency" } });
    fireEvent.click(await screen.findByRole("button", { name: /navLatency/ }));
    fireEvent.click(screen.getByRole("button", { name: "latStartRecording" }));
    await screen.findByRole("button", { name: "latStopSave" });
    fireEvent.click(screen.getByTitle("navDashboard"));
    fireEvent.change(search, { target: { value: "latency" } });
    fireEvent.click(await screen.findByRole("button", { name: /navLatency/ }));
    expect(screen.getByRole("button", { name: "latStopSave" })).toBeTruthy();
    invoke.mockRejectedValueOnce(new Error("cancel failed"));
    fireEvent.click(screen.getByRole("button", { name: "cancel", exact: true }));
    await screen.findByText(/cancel failed/);
    expect(screen.getByRole("button", { name: "latStopSave" })).toBeTruthy();
  });

  it("drops a process-kill confirmation on navigation and reloads process state", async () => {
    render(<App />);
    const search = screen.getByRole("textbox", { name: "navSearchTools" });
    const open = async () => {
      fireEvent.change(search, { target: { value: "processes" } });
      fireEvent.click(await screen.findByRole("button", { name: /navProcesses/ }));
      await screen.findByText("game.exe");
    };
    await open();
    fireEvent.click(screen.getByRole("button", { name: "procKill" }));
    expect(screen.getByRole("button", { name: "confirm", exact: true })).toBeTruthy();
    fireEvent.click(screen.getByTitle("navDashboard"));
    await open();
    expect(screen.queryByRole("button", { name: "confirm", exact: true })).toBeNull();
    expect(invoke.mock.calls.filter(([command]) => command === "cmd_proc_list")).toHaveLength(2);
    expect(invoke.mock.calls.some(([command]) => command === "cmd_proc_kill")).toBe(false);
  });

  it("holds self-update navigation after download until installation resolves", async () => {
    const work = deferred<void>();
    checkUpdate.mockResolvedValue({ version: "2.0.0", downloadAndInstall: (progress: (event: any) => void) => {
      progress({ event: "Finished" });
      return work.promise;
    } });
    render(<App />);
    fireEvent.change(screen.getByRole("textbox", { name: "navSearchTools" }), { target: { value: "updates" } });
    fireEvent.click(await screen.findByRole("button", { name: /navUpdates/ }));
    fireEvent.click(await screen.findByRole("button", { name: "updDownloadBtn" }));
    expect(screen.queryByRole("button", { name: "updRestartBtn" })).toBeNull();
    fireEvent.click(screen.getByTitle("navDashboard"));
    expect(screen.getByText(/Aktion läuft/)).toBeTruthy();
    await act(async () => work.resolve());
    expect(screen.getByRole("button", { name: "updRestartBtn" })).toBeTruthy();
  });

  it("does not restart hardware polling after a late response to an unmounted page", async () => {
    vi.useFakeTimers();
    const work = deferred<any>();
    invoke.mockReturnValue(work.promise);
    const page = render(<HwMonitor />);
    page.unmount();
    await act(async () => work.resolve({}));
    await vi.advanceTimersByTimeAsync(30_000);
    expect(invoke.mock.calls.filter(([command]) => command === "cmd_hw_full")).toHaveLength(1);
  });

  it("unsubscribes and stops the live monitor when leaving", async () => {
    const page = render(<Monitor />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("cmd_start_monitor", undefined));
    page.unmount();
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("cmd_stop_monitor", undefined));
    expect(unlisten).toHaveBeenCalled();
    expect(pendingOperations.getSnapshot()).toBe(0);
  });
});
