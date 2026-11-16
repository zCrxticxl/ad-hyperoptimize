export type Toggle = "unknown" | "on" | "off";
export type DisplayInputs = {
  vrr: Toggle; gameVsync: Toggle; driverVsync: Toggle; driverQueue: Toggle; frameGen: Toggle;
  limiter: "unknown" | "none" | "game" | "driver" | "external" | "automatic" | "multiple";
  latency: "unknown" | "none" | "reflex" | "antilag";
  mode: "unknown" | "exclusive" | "borderless" | "windowed";
  api: "unknown" | "dx11" | "dx12" | "vulkan";
  goal: "smooth" | "fast";
  cap: number | null; hz: number | null; belowMax: boolean;
};

export function displayAdvice(v: DisplayInputs) {
  const out: Array<`displayAdvice_${"unknown" | "refresh" | "cap" | "multiple" | "auto" | "vrrSmooth" | "tearing" | "fixed" | "vsync" | "reflex" | "antilag" | "modernApi" | "mode" | "fg" | "missing"}`> = [];
  if (v.belowMax) out.push("displayAdvice_refresh");
  if (v.vrr === "unknown") out.push("displayAdvice_unknown");
  if (v.vrr === "on" && v.hz && v.cap && v.cap >= v.hz && ["game", "driver", "external", "multiple"].includes(v.limiter)) out.push("displayAdvice_cap");
  if (v.limiter === "multiple") out.push("displayAdvice_multiple");
  if (v.latency === "reflex" || v.limiter === "automatic") out.push("displayAdvice_auto");
  if (v.goal === "smooth" && v.vrr === "on") out.push("displayAdvice_vrrSmooth");
  if (v.goal === "fast") out.push("displayAdvice_tearing");
  if (v.vrr === "off") out.push("displayAdvice_fixed");
  if (v.gameVsync === "on" && v.driverVsync === "on") out.push("displayAdvice_vsync");
  if (v.latency === "reflex" && v.driverQueue === "on") out.push("displayAdvice_reflex");
  if (v.latency === "antilag") out.push("displayAdvice_antilag");
  if (v.driverQueue === "on" && ["dx12", "vulkan"].includes(v.api)) out.push("displayAdvice_modernApi");
  if (v.mode !== "unknown") out.push("displayAdvice_mode");
  if (v.frameGen === "on") out.push("displayAdvice_fg");
  if (Object.values(v).includes("unknown")) out.push("displayAdvice_missing");
  return out;
}

export function positiveNumber(value: string): number | null {
  const n = Number(value);
  return Number.isFinite(n) && n > 0 && n <= 10000 ? n : null;
}
