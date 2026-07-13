import type { Lang } from "./i18n";
import * as es from "./locales/es";
import * as fr from "./locales/fr";
import * as pt from "./locales/pt";
import * as pl from "./locales/pl";
import * as ru from "./locales/ru";
import * as tr from "./locales/tr";
import * as sv from "./locales/sv";

type TweakOverlay = { name?: string; description?: string; rationale?: string; impact?: string };
type TextOverlay = { title?: string; message?: string };

const TWEAKS: Partial<Record<Lang, Record<string, TweakOverlay>>> = {
  es: es.tweaks, fr: fr.tweaks, pt: pt.tweaks, pl: pl.tweaks, ru: ru.tweaks, tr: tr.tweaks, sv: sv.tweaks,
};
const WARNINGS: Partial<Record<Lang, Record<string, TextOverlay>>> = {
  es: es.warnings, fr: fr.warnings, pt: pt.warnings, pl: pl.warnings, ru: ru.warnings, tr: tr.warnings, sv: sv.warnings,
};
const RISKS: Partial<Record<Lang, Record<string, TextOverlay>>> = {
  es: es.risks, fr: fr.risks, pt: pt.risks, pl: pl.risks, ru: ru.risks, tr: tr.risks, sv: sv.risks,
};

// Tweak catalog entries (tweaks.rs + gputweaks.rs) — keyed by tweak id.
// `de`/`en` pass through untouched (no overlay exists for those langs).
export function localizeTweak<T extends { id: string; name: string; description: string; rationale?: string; impact?: string }>(
  t: T,
  lang: Lang
): T {
  const overlay = TWEAKS[lang]?.[t.id];
  if (!overlay) return t;
  return {
    ...t,
    name: overlay.name ?? t.name,
    description: overlay.description ?? t.description,
    rationale: overlay.rationale ?? t.rationale,
    impact: overlay.impact ?? t.impact,
  };
}

// Hardware warnings (hwprofile.rs) — composite key `${page}::${id}` because the
// same id is reused with different text across pages (e.g. "low_ram" on
// dashboard vs game_booster).
export function localizeWarning<T extends { page: string; id: string; title: string; message: string }>(
  w: T,
  lang: Lang
): T {
  const overlay = WARNINGS[lang]?.[`${w.page}::${w.id}`];
  if (!overlay) return w;
  return { ...w, title: overlay.title ?? w.title, message: overlay.message ?? w.message };
}

// Per-tweak hardware risk verdict (hwprofile.rs `tweakRisks`) — composite key
// `${tweakId}::${englishTitle}` because "nv_power_max_perf" has two mutually
// exclusive English text variants (laptop vs. desktop) chosen at runtime.
export function localizeRisk<T extends { title: string; message: string }>(
  risk: T,
  tweakId: string,
  lang: Lang
): T {
  const overlay = RISKS[lang]?.[`${tweakId}::${risk.title}`];
  if (!overlay) return risk;
  return { ...risk, title: overlay.title ?? risk.title, message: overlay.message ?? risk.message };
}

// Dashboard analysis findings (analysis.rs) — backend ships a stable `code` +
// raw `params` per finding (English title/detail/recommendation too, used
// only by the Rust-side HTML report which has no i18n). The live UI instead
// renders fully localized text built from `find<Code>Title/Detail/Rec` i18n
// keys + simple {placeholder} substitution, so it follows the selected
// language like everything else in the app instead of being English-only.
function interpolate(template: string, params: Record<string, unknown>): string {
  return template.replace(/\{(\w+)\}/g, (m, key) => (key in params ? String(params[key]) : m));
}

function toPascalCase(code: string): string {
  return code.split("_").map(s => s.charAt(0).toUpperCase() + s.slice(1)).join("");
}

export type Finding = {
  severity: number;
  code: string;
  title: string;
  detail: string;
  recommendation: string;
  tweakIds: string[];
  params: Record<string, unknown>;
};

export function localizeFinding(finding: Finding, t: (key: any) => string): { title: string; detail: string; recommendation: string } {
  // Defensive: stale pre-v2 cached findings (or any future shape drift) may
  // lack `code`/`params` — fall back to whatever English text is present
  // instead of throwing and taking down the whole Dashboard render.
  if (!finding || typeof finding.code !== "string" || !finding.code) {
    return {
      title: finding?.title ?? "",
      detail: finding?.detail ?? "",
      recommendation: finding?.recommendation ?? "",
    };
  }
  const base = toPascalCase(finding.code);
  const titleKey = `find${base}Title`;
  const detailKey = `find${base}Detail`;
  const recKey = `find${base}Rec`;
  const title = t(titleKey);
  const detailTpl = t(detailKey);
  const rec = t(recKey);
  // t() returns the raw key string if missing (see i18n.tsx's t() fallback
  // chain) — fall back to the backend's English text for any finding code
  // that doesn't (yet) have a translation, rather than showing a raw key.
  return {
    title: title === titleKey ? finding.title : title,
    detail: detailTpl === detailKey ? finding.detail : interpolate(detailTpl, finding.params ?? {}),
    recommendation: rec === recKey ? finding.recommendation : rec,
  };
}

// Builds the Dashboard's localized one-line summary from the already-sorted
// findings list (severity descending), mirroring analysis.rs's English
// summary construction but driven by `dashSummaryTemplate`/`dashSummaryNone`.
export function localizeSummary(findings: Finding[], t: (key: any) => string): string {
  if (findings.length === 0) return t("dashSummaryNone");
  const top = localizeFinding(findings[0], t);
  return interpolate(t("dashSummaryTemplate"), {
    count: findings.length,
    title: top.title,
    recommendation: top.recommendation,
  });
}
