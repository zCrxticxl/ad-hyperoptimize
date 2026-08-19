# Release kit, AD HyperOptimize v1.4.0

## 1. Ship it on GitHub (exact steps)

In `E:\CLAUDE\git_ad_opt\ad-hyperoptimize`:

```
git add -A
git commit -F CHANGELOG-v1.4.0.md
git push
git tag v1.4.0
git push origin v1.4.0
```

- The tag push triggers `.github/workflows/release.yml`.
- The tag **must** be `v1.4.0` (matches `tauri.conf.json` version `1.4.0`), the workflow aborts on a mismatch.
- The commit message (= the changelog file) becomes the GitHub release notes **and** the in-app "update available" text, so keep that commit as the tagged one.
- Build takes ~10-20 min (Rust). When done you get: signed installer, `latest.json` (auto-update), `SHA256SUMS.txt`, all attached to the release.
- Watch progress under the repo's **Actions** tab.

If a run fails and you want to re-cut the same version:
```
git push --delete origin v1.4.0
git tag -d v1.4.0
```
fix, then tag + push again.

---

## 2. Thought process for the announcement

1. **Lead with the user benefit, not the internals.** "New dashboard, restorable cleaner" beats "refactored nav state."
2. **Use the honesty angle on purpose.** A reviewer publicly audited your claims and was right. Owning that in the open builds more trust than any feature, it's a rare, credible signal. Make it a feature of the release, not a footnote.
3. **One visual.** A short GIF/screenshot of the new dashboard does more than three bullet points. Attach it to the tweet and pin the Discord post.
4. **Cross-post, don't copy-paste identically.** Tweet = punchy hook. Discord = friendly detail for people who already care. GitHub release = full changelog.
5. **Give a clear next step:** download link + "update is automatic if you're on 1.3.0."

---

## 3. Tweet, pick one (attach a dashboard GIF/screenshot)

**A, honesty-forward (recommended, given the public audit):**
> AD HyperOptimize v1.4.0 ⚡
>
> Someone audited my claims, and was right on several. So I fixed the app *and* the wording:
> • registry cleaner is now genuinely restorable (real .reg backups)
> • killed the QoS "20% bandwidth" myth
> • honest about admin + what the security page does
>
> Free · open · zero telemetry 👇
> github.com/zCrxticxl/ad-hyperoptimize

**B, feature-forward:**
> AD HyperOptimize v1.4.0 is out ⚡
> • New dashboard, pick a category, not a wall of 39 tools
> • Registry cleaner is now fully restorable
> • Fixed the stuck-Apply + missing-service bugs
>
> Free, open, no telemetry:
> github.com/zCrxticxl/ad-hyperoptimize

**C, short/punchy:**
> AD HyperOptimize v1.4.0 ⚡ new category dashboard, a registry cleaner you can actually undo, and an honesty pass on every claim.
> free · open · zero telemetry
> github.com/zCrxticxl/ad-hyperoptimize

**German variant (of A):**
> AD HyperOptimize v1.4.0 ⚡
> Jemand hat meine Claims geprüft, und lag mehrfach richtig. Also hab ich die App *und* die Aussagen gefixt:
> • Registry-Cleaner jetzt wirklich wiederherstellbar
> • QoS-"20%-Bandbreite"-Mythos rausgeworfen
> • ehrlich zu Adminrechten + Security-Seite
> Kostenlos · offen · keine Telemetrie 👇

---

## 4. Discord announcement (#announcements, @everyone)

```
@everyone  ⚡ AD HyperOptimize v1.4.0 is live

**Redesigned dashboard**, no more wall of 39 tools. Pick a category (Performance, Cleanup, Protection…), each tool explains itself, and the search up top jumps straight to what you need. Beginner mode now really hides the risky stuff; Expert unlocks everything.

**Registry cleaner you can undo**, every deletion now exports a real .reg backup first, and there's a new "Restore…" button to bring any past cleanup back.

**Fixes**
• "Apply" no longer gets stuck on tweaks that need a reboot to verify
• Tweaks targeting a service you don't have no longer throw an error

**Honesty pass**, someone reviewed the project and was right on a few things, so I corrected both the app and the claims: admin is required (and stated), the Security page has explicit opt-in controls (not just read-only), and the QoS "20% bandwidth" tweak now says outright that it's a myth.

Update is automatic if you're on 1.3.0, or grab the installer in #releases.
No bloat, no telemetry, undo built in. 🚀
```

**German variant:** available on request, say the word.

---

## 5. After it's live
- Pin the Discord post.
- Drop the installer link in `#releases` (the workflow already attaches it to the GitHub release).
- Optional: reply to the reviewer publicly with "fixed in v1.4.0", closes the loop and looks great.
