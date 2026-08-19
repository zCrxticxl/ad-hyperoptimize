import React from "react";
import { useLang, LANG_NAMES, Lang } from "../i18n";
import type { Mode } from "../App";

/**
 * First-run screen. Shown once (guarded by `ui.onboarded` in localStorage) so a
 * new user picks a language and an experience level before being dropped into a
 * tool with 39 entries. The mode choice is the important one: it decides whether
 * the sidebar shows 13 safe tools or the full catalogue.
 */
export default function Onboarding({ onDone }: { onDone: (mode: Mode) => void }) {
  const { lang, setLang, t } = useLang();

  return (
    <div className="onboarding">
      <div className="onboarding-card">
        <div className="onboarding-brand">
          <div className="brand-mark">AD</div>
          <div className="logo">Hyper<span>Optimize</span></div>
        </div>

        <h1>{t("onbTitle")}</h1>
        <p className="onboarding-sub">{t("onbSub")}</p>

        <label className="onboarding-lang">
          <span>{t("onbLanguage")}</span>
          <select value={lang} onChange={(event) => setLang(event.target.value as Lang)}>
            {Object.entries(LANG_NAMES).map(([code, name]) => <option key={code} value={code}>{name}</option>)}
          </select>
        </label>

        <div className="onboarding-choices">
          <button className="onboarding-choice recommended" onClick={() => onDone("beginner")}>
            <span className="choice-icon" aria-hidden="true">🌱</span>
            <b>{t("onbBeginnerTitle")}</b>
            <span className="choice-desc">{t("onbBeginnerDesc")}</span>
            <span className="choice-tag">{t("onbRecommended")}</span>
          </button>
          <button className="onboarding-choice" onClick={() => onDone("expert")}>
            <span className="choice-icon" aria-hidden="true">🛠️</span>
            <b>{t("onbExpertTitle")}</b>
            <span className="choice-desc">{t("onbExpertDesc")}</span>
          </button>
        </div>

        <div className="onboarding-safety">
          <span aria-hidden="true">🛟</span>
          <span>{t("onbSafety")}</span>
        </div>
        <p className="onboarding-switch-hint">{t("onbSwitchHint")}</p>
      </div>
    </div>
  );
}
