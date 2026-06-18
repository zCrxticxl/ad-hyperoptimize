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
