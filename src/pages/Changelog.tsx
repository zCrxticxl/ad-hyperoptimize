import React from "react";
import { Card } from "../components/ui";
import { useLang } from "../i18n";
import changelogMd from "../../CHANGELOG.md?raw";

type Section = { title: string; items: string[] };
type Release = { version: string; title: string; sections: Section[] };

// Parse the repository CHANGELOG.md (bundled at build time) so the in-app
// changelog is always the single source of truth, never a hand-copied list.
// Format: `## vX.Y.Z — Title`, `### Section`, `- item`.
function parseChangelog(md: string): Release[] {
  const releases: Release[] = [];
  let release: Release | null = null;
  let section: Section | null = null;
  for (const raw of md.split("\n")) {
    const line = raw.trimEnd();
    const rel = line.match(/^##\s+v?([0-9][^\s—-]*)\s*[—-]\s*(.+)$/);
    if (rel) {
      release = { version: rel[1], title: rel[2].trim(), sections: [] };
      releases.push(release);
      section = null;
      continue;
    }
    if (line.startsWith("### ") && release) {
      section = { title: line.slice(4).trim(), items: [] };
      release.sections.push(section);
      continue;
    }
    const item = line.match(/^[-*]\s+(.+)$/);
    if (item && section) section.items.push(item[1].trim());
  }
  return releases;
}

const RELEASES: Release[] = parseChangelog(changelogMd);

export const CURRENT_VERSION = RELEASES[0]?.version ?? "";

export default function Changelog() {
  const { t } = useLang();

  return (
    <>
      <h1 className="page-title">{t("changelogTitle")}</h1>
      <div className="page-sub">{t("changelogSub")}</div>

      {RELEASES.map((release, index) => (
        <Card
          key={release.version}
          title={`v${release.version}${index === 0 ? ` · ${t("changelogCurrent")}` : ""}`}
          style={{ marginTop: index === 0 ? 14 : 18 }}
        >
          <div lang="en">
            <div style={{ fontWeight: 700, marginBottom: 12 }}>{release.title}</div>
            {release.sections.map((section) => (
              <section key={section.title} style={{ marginTop: 12 }}>
                <h3 style={{ fontSize: 13, margin: "0 0 6px" }}>{section.title}</h3>
                <ul style={{ margin: 0, paddingLeft: 20, lineHeight: 1.6 }}>
                  {section.items.map((item) => <li key={item}>{item}</li>)}
                </ul>
              </section>
            ))}
          </div>
        </Card>
      ))}
    </>
  );
}
