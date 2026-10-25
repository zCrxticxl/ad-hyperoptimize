import React from "react";
import { useLang } from "../i18n";
import type { Mode } from "../App";
import { HardwareCard } from "../components/deep/HardwareCard";
import { LeftoversCard } from "../components/deep/LeftoversCard";
import { IrqCard } from "../components/deep/IrqCard";
import { RebarCard } from "../components/deep/RebarCard";

/** Deep check: hardware misconfiguration and harmful leftovers that cost
 * real performance; interrupt affinity for experts. */
export default function DeepCheck({ admin, mode }: { admin: boolean; mode: Mode }) {
  const { t } = useLang();
  return (
    <>
      <h1 className="page-title">{t("navDeepCheck")}</h1>
      <div className="page-sub">{t("toolDescDeepCheck")}</div>
      <HardwareCard admin={admin} />
      <div className="mt"><RebarCard admin={admin} /></div>
      <div className="mt"><LeftoversCard admin={admin} /></div>
      {mode === "expert" && <div className="mt"><IrqCard admin={admin} /></div>}
    </>
  );
}
