import React from "react";
import { useLang } from "../i18n";
import { ThrottleCard } from "../components/diag/ThrottleCard";
import { PregameCard } from "../components/diag/PregameCard";
import { StutterCard } from "../components/diag/StutterCard";
import { DisplayCard } from "../components/diag/DisplayCard";
import { InputCard } from "../components/diag/InputCard";
import { AudioCard } from "../components/diag/AudioCard";
import { StorageCard } from "../components/diag/StorageCard";

/** Gaming diagnostics: measurements that find real causes of low FPS and
 * stutter. Read-only; nothing here changes the system. */
export default function GameDiag({ admin }: { admin: boolean }) {
  const { t } = useLang();
  return (
    <>
      <h1 className="page-title">{t("navGameDiag")}</h1>
      <div className="page-sub">{t("toolDescGameDiag")}</div>
      <PregameCard admin={admin} />
      <div className="mt"><ThrottleCard /></div>
      <div className="mt"><StutterCard /></div>
      <div className="mt"><DisplayCard /></div>
      <div className="mt"><InputCard /></div>
      <div className="mt"><AudioCard /></div>
      <div className="mt"><StorageCard /></div>
    </>
  );
}
