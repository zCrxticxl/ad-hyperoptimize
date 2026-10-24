import type { Lang } from "../i18n";

/** German texts for the English strings the health check engine returns
 * (optregistry.rs, optimizer.rs, tweaks.rs revert errors). Exact matches
 * first, then patterns for formatted messages; anything unknown stays
 * English, so a missing entry never hides information. */
const DE: Record<string, string> = {
  // ── optregistry: benefit / when ───────────────────────────────────────────
  "Restores Windows Game Mode: Windows prioritises the game and Windows Update does not install drivers or show restart notifications while you play.":
    "Stellt den Windows-Spielmodus wieder her: Windows bevorzugt das Spiel, und Windows Update installiert während des Spielens keine Treiber und zeigt keine Neustart-Hinweise.",
  "Only relevant when Game Mode was turned off; it is on by default.":
    "Nur relevant, wenn der Spielmodus ausgeschaltet wurde; standardmäßig ist er an.",
  "Stops Game Bar capture from recording in the background, which uses the GPU video encoder and disk while you play.":
    "Beendet die Hintergrundaufnahme der Game Bar, die während des Spielens den Video-Encoder der GPU und die Festplatte nutzt.",
  "Helps when background recording/capture was enabled. Turn it back on if you use Game Bar clips.":
    "Hilft, wenn die Hintergrundaufnahme eingeschaltet war. Wieder einschalten, wenn du Game-Bar-Clips nutzt.",
  "Makes cursor movement independent of mouse speed, so aim is consistent. Does not change FPS.":
    "Macht die Zeigerbewegung unabhängig von der Mausgeschwindigkeit, damit das Zielen gleichmäßig ist. Ändert die FPS nicht.",
  "Useful for mouse-aimed games. Changes how the desktop cursor feels, so it is never applied automatically.":
    "Sinnvoll für Spiele, in denen mit der Maus gezielt wird. Verändert das Gefühl des Mauszeigers am Desktop, deshalb nie automatisch.",
  "Lets the GPU schedule its own work. Required for DLSS Frame Generation; otherwise effects range from small gains to none or regressions depending on game and driver.":
    "Lässt die GPU ihre Arbeit selbst planen. Pflicht für DLSS Frame Generation; sonst reicht die Wirkung je nach Spiel und Treiber von kleinen Gewinnen über keine bis zu Verschlechterungen.",
  "Needs a GPU and driver that support it (WDDM 2.7+). Support cannot be confirmed before restart, so it is never applied automatically.":
    "Braucht eine GPU und einen Treiber, die es unterstützen (WDDM 2.7+). Die Unterstützung lässt sich erst nach dem Neustart bestätigen, deshalb nie automatisch.",
  "Keeps CPU clocks higher at idle, which can reduce ramp-up latency. On modern CPUs Balanced already boosts quickly; gains are usually small and power use rises.":
    "Hält die CPU-Takte im Leerlauf höher, was die Hochlaufzeit verkürzen kann. Moderne CPUs takten auch mit „Ausbalanciert“ schnell hoch; der Gewinn ist meist klein, der Stromverbrauch steigt.",
  "Desktops only. Not offered on laptops or systems without the High performance plan (Modern Standby).":
    "Nur für Desktop-PCs. Wird auf Laptops und Systemen ohne Höchstleistungsplan (Modern Standby) nicht angeboten.",
  "Restores TCP receive-window auto-tuning to the Windows default. A disabled or restricted level limits download throughput on fast connections.":
    "Stellt das automatische Anpassen des TCP-Empfangsfensters auf den Windows-Standard zurück. Eine abgeschaltete oder eingeschränkte Stufe bremst Downloads auf schnellen Leitungen.",
  "Only relevant when auto-tuning was changed from Normal.":
    "Nur relevant, wenn das Autotuning von „Normal“ geändert wurde.",
  "Restores Receive Side Scaling, which spreads network processing across CPU cores. It is on by default.":
    "Stellt Receive Side Scaling wieder her, das die Netzwerkverarbeitung auf mehrere CPU-Kerne verteilt. Standardmäßig an.",
  "Only relevant when RSS was turned off.": "Nur relevant, wenn RSS ausgeschaltet wurde.",
  "Deletes hiberfil.sys and frees disk space roughly the size of a large part of your RAM. Also disables Fast Startup.":
    "Löscht hiberfil.sys und gibt Speicherplatz in der Größenordnung eines großen Teils deines RAMs frei. Schaltet auch den Schnellstart ab.",
  "Offered only on desktops whose system drive is low on space. Not a performance change.":
    "Wird nur Desktop-PCs mit knappem Platz auf dem Systemlaufwerk angeboten. Keine Leistungsänderung.",
  "Can fix flickering, black screens or stutter caused by Multiplane Overlay on some driver/monitor combinations.":
    "Kann Flackern, schwarze Bildschirme oder Ruckler beheben, die Multiplane Overlay bei manchen Treiber-/Monitor-Kombinationen verursacht.",
  "Only if you actually see those symptoms. Reportedly ignored on newer Windows builds.":
    "Nur wenn du diese Symptome wirklich hast. Neuere Windows-Builds ignorieren es Berichten zufolge.",
  "Can fix USB devices that disconnect or lag after idle.":
    "Kann USB-Geräte reparieren, die nach Leerlauf die Verbindung verlieren oder verzögert reagieren.",
  "Only for that problem; desktops only. Applies to the current power plan.":
    "Nur bei diesem Problem und nur auf Desktop-PCs. Gilt für den aktuellen Energieplan.",
  "Makes shutdown a full shutdown. Helps dual-boot setups and drivers that misbehave after Fast Startup.":
    "Macht Herunterfahren zu einem vollständigen Herunterfahren. Hilft bei Dual-Boot und Treibern, die nach dem Schnellstart Probleme machen.",
  "Only for those cases. Already implied by disabling hibernation.":
    "Nur in diesen Fällen. Ist bei abgeschaltetem Ruhezustand bereits enthalten.",
  "No reliable evidence of a performance gain.": "Kein verlässlicher Beleg für einen Leistungsgewinn.",
  "Only affects threads registered with the multimedia scheduler; most games do not register.":
    "Betrifft nur Threads, die beim Multimedia-Scheduler registriert sind; die meisten Spiele tun das nicht.",
  "Only affects threads that register with the MMCSS \"Games\" task.":
    "Betrifft nur Threads, die sich bei der MMCSS-Aufgabe „Games“ registrieren.",
  "0x26 is the same scheduling as the Windows client default.":
    "0x26 entspricht der Standard-Planung von Windows-Client-Systemen.",
  "Power throttling targets background apps; a foreground game is not throttled. Harmful on laptops and hybrid CPUs.":
    "Die Energiedrosselung trifft Hintergrund-Apps; ein Spiel im Vordergrund wird nicht gedrosselt. Schädlich auf Laptops und CPUs mit P- und E-Kernen.",
  "Effect not established; global flags are largely ignored for modern DX12/flip-model games.":
    "Wirkung nicht belegt; moderne DX12-/Flip-Model-Spiele ignorieren die globalen Werte weitgehend.",
  "Per-game compatibility settings are the reliable control.":
    "Zuverlässig steuern lässt es sich über die Kompatibilitätseinstellungen des einzelnen Spiels.",
  "Startup apps launch sooner after sign-in. No effect on games.":
    "Autostart-Programme starten nach der Anmeldung früher. Keine Wirkung auf Spiele.",
  "Changes how Windows looks or behaves; not a performance change.":
    "Ändert Aussehen oder Verhalten von Windows; keine Leistungsänderung.",
  "Turns off shell animations and shadows. No measurable gain on GPUs from the last decade.":
    "Schaltet Animationen und Schatten der Oberfläche ab. Auf GPUs der letzten zehn Jahre kein messbarer Gewinn.",
  "Menus open faster.": "Menüs öffnen sich schneller.",
  "Fewer Game Bar pop-ups.": "Weniger Game-Bar-Einblendungen.",
  "Prefetching costs little on SSDs and still helps on hard disks; not offered when a hard disk is present.":
    "Prefetching kostet auf SSDs wenig und hilft Festplatten weiterhin; wird nicht angeboten, wenn eine Festplatte verbaut ist.",
  "Slows or breaks Start menu, Outlook and Settings search.":
    "Verlangsamt oder stört die Suche in Startmenü, Outlook und Einstellungen.",
  "Windows already disables short names on new non-system volumes; old installers may need them.":
    "Windows schaltet Kurznamen auf neuen Nicht-System-Laufwerken bereits ab; alte Installer brauchen sie eventuell.",
  "Stops Edge from preloading at sign-in.": "Edge lädt nach der Anmeldung nicht mehr vor.",
  "Edge will show \"managed by your organization\".": "Edge zeigt danach „Von Ihrer Organisation verwaltet“.",
  "Fax is manual-start and idle when unused.": "Fax startet manuell und ist ungenutzt im Leerlauf.",
  "Often absent on Windows 11.": "Fehlt unter Windows 11 oft.",
  "Stops offline map updates.": "Beendet Updates der Offlinekarten.",
  "Breaks repair of moved-file shortcuts on network shares.":
    "Verknüpfungen zu verschobenen Dateien auf Netzwerkfreigaben werden nicht mehr repariert.",
  "Breaks Windows Insider builds.": "Windows-Insider-Builds funktionieren nicht mehr.",
  "Diagnostics Hub collector used by Visual Studio; idle otherwise.":
    "Diagnosehub-Sammeldienst für Visual Studio; sonst im Leerlauf.",
  "Reduces data collection or ads; not a performance change.":
    "Weniger Datensammlung oder Werbung; keine Leistungsänderung.",
  "Feedback Hub diagnostics stop working.": "Diagnosedaten im Feedback-Hub funktionieren nicht mehr.",
  "Also removes File Explorer search suggestions.": "Entfernt auch die Suchvorschläge im Datei-Explorer.",
  "Stops sharing update data with other PCs.": "Teilt keine Update-Daten mehr mit anderen PCs.",
  "Updates for several PCs on one network may download more data.":
    "Bei mehreren PCs im selben Netz laden Updates eventuell mehr Daten herunter.",
  "Crash reports for troubleshooting are no longer created.":
    "Absturzberichte zur Fehlersuche werden nicht mehr erstellt.",
  "Removes the Widgets board and its news feed.": "Entfernt das Widget-Board und seinen Newsfeed.",
  "Removes search highlights.": "Entfernt die Suchhighlights.",
  "Stops media and USB drives from starting programs on insert (security hygiene).":
    "Medien und USB-Laufwerke starten beim Einstecken keine Programme mehr (Sicherheitsgewohnheit).",
  "Keeps all cores unparked.": "Hält alle Kerne aktiv.",
  "Defeats the core parking AMD dual-CCD X3D and Intel hybrid CPUs rely on to keep games on the right cores; High performance plans already unpark cores.":
    "Hebelt das Core Parking aus, mit dem AMD-Dual-CCD-X3D- und Intel-Hybrid-CPUs Spiele auf den richtigen Kernen halten; Höchstleistungspläne aktivieren die Kerne bereits.",
  "The services are already manual-start; forcing them off can break Game Pass and Xbox sign-in.":
    "Die Dienste starten bereits manuell; sie zwangsweise abzuschalten kann Game Pass und die Xbox-Anmeldung stören.",
  "TCPNoDelay is not read by Windows; games set Nagle per socket, and most shooters (incl. Rainbow Six Siege) use UDP.":
    "Windows liest TCPNoDelay nicht; Spiele setzen Nagle pro Socket, und die meisten Shooter (auch Rainbow Six Siege) nutzen UDP.",
  "Only matters for servers exhausting ports.": "Betrifft nur Server, denen die Ports ausgehen.",
  "Only affects the SMB file-sharing server.": "Betrifft nur den SMB-Dateifreigabe-Server.",
  "Windows does not reserve 20 % bandwidth; this is a long-standing myth.":
    "Windows reserviert keine 20 % Bandbreite; das ist ein alter Mythos.",
  "Already disabled by default on Windows 10/11 client editions.":
    "Auf Windows-10/11-Client-Editionen standardmäßig bereits deaktiviert.",
  "Already manual-start by default.": "Startet standardmäßig bereits manuell.",
  "No measurable effect with modern memory management.":
    "Mit moderner Speicherverwaltung kein messbarer Effekt.",
  "Since Windows 10 1803 last-access updates are system-managed and already off on most volumes.":
    "Seit Windows 10 1803 verwaltet das System die Zugriffszeitstempel selbst; auf den meisten Laufwerken sind sie bereits aus.",
  "The Cortana app was retired.": "Die Cortana-App wurde eingestellt.",
  "Windows 11 removed this global setting; it only works on Windows 10.":
    "Windows 11 hat diese globale Einstellung entfernt; sie wirkt nur unter Windows 10.",

  // ── optregistry: eligibility ───────────────────────────────────────────────
  "Not for laptops or battery-powered systems.": "Nicht für Laptops oder akkubetriebene Systeme.",
  "Only applies to Windows 10.": "Gilt nur für Windows 10.",
  "This system has no High performance power plan (Modern Standby).":
    "Dieses System hat keinen Höchstleistungsplan (Modern Standby).",
  "No dedicated GPU detected.": "Keine dedizierte GPU erkannt.",
  "A hard disk is present; prefetching still helps it.": "Eine Festplatte ist verbaut; Prefetching hilft ihr weiterhin.",
  "Not suitable for AMD dual-CCD X3D CPUs.": "Nicht geeignet für AMD-Dual-CCD-X3D-CPUs.",
  "Not suitable for CPUs with performance and efficiency cores.": "Nicht geeignet für CPUs mit P- und E-Kernen.",
  "The system drive has enough free space.": "Auf dem Systemlaufwerk ist genug Platz frei.",

  // ── optimizer: diagnosis ───────────────────────────────────────────────────
  "A previous attempt did not finish. Check the undo history.":
    "Ein früherer Versuch wurde nicht abgeschlossen. Prüfe den Rückgängig-Verlauf.",
  "Retired: no measurable benefit.": "Ausgemustert: kein messbarer Nutzen.",
  "This setting does not exist on this system.": "Diese Einstellung gibt es auf diesem System nicht.",
  "An equivalent or stronger setting is already active.": "Eine gleichwertige oder stärkere Einstellung ist bereits aktiv.",
  "Applied. No proven performance benefit; keep it only if you want its side effect.":
    "Angewendet. Kein belegter Leistungsnutzen; nur behalten, wenn du den Nebeneffekt willst.",
  "Only some of its values are set. Undo and apply again to repair it.":
    "Nur ein Teil der Werte ist gesetzt. Zum Reparieren rückgängig machen und neu anwenden.",
  "Correctly configured.": "Richtig eingestellt.",
  "The current state cannot be read on this system.": "Der aktuelle Zustand lässt sich auf diesem System nicht auslesen.",
  "Not part of one-click optimization.": "Nicht Teil der Ein-Klick-Optimierung.",
  "Not needed on this system.": "Auf diesem System nicht nötig.",
  "Needs administrator rights.": "Braucht Administratorrechte.",
  "Unknown optimization.": "Unbekannte Optimierung.",

  // ── optimizer: apply / reset outcomes ──────────────────────────────────────
  "The setting was confirmed in Windows; it takes effect after a restart.":
    "Die Einstellung wurde in Windows bestätigt; sie wirkt nach einem Neustart.",
  "The setting was confirmed in Windows; some programs pick it up after you sign out.":
    "Die Einstellung wurde in Windows bestätigt; manche Programme übernehmen sie erst nach dem Abmelden.",
  "The setting was confirmed in Windows.": "Die Einstellung wurde in Windows bestätigt.",
  "The command ran, but Windows offers no way to read this setting back.":
    "Der Befehl lief, aber Windows bietet keine Möglichkeit, diese Einstellung zurückzulesen.",
  "Reset; the original value was confirmed in Windows.":
    "Zurückgesetzt; der Originalwert wurde in Windows bestätigt.",
  "The original value is identical to this setting (Windows default); nothing to change.":
    "Der Originalwert entspricht dieser Einstellung (Windows-Standard); nichts zu ändern.",
  "Still active after the reset: another tool, a policy or another page of this app sets it again.":
    "Nach dem Zurücksetzen weiter aktiv: Ein anderes Programm, eine Richtlinie oder eine andere Seite dieser App setzt den Wert erneut.",
  "Only partly reset. Check the undo history.": "Nur teilweise zurückgesetzt. Prüfe den Rückgängig-Verlauf.",
  "Reset ran; Windows offers no way to read this setting back.":
    "Zurücksetzen lief; Windows bietet keine Möglichkeit, diese Einstellung zurückzulesen.",
  "The setting could not be read back.": "Die Einstellung ließ sich nicht zurücklesen.",
  "Already applied.": "Bereits angewendet.",

  // ── optimizer: gaming setup checks ─────────────────────────────────────────
  "Memory speed": "RAM-Takt",
  "Windows did not report the configured memory speed.": "Windows hat den eingestellten RAM-Takt nicht gemeldet.",
  "The rated speed comes from the modules' part number. Enable XMP (Intel) or EXPO/DOCP (AMD) in the BIOS, then check stability. Faster memory mainly helps CPU-bound games.":
    "Der Nenntakt stammt aus der Teilenummer der Module. XMP (Intel) bzw. EXPO/DOCP (AMD) im BIOS einschalten und danach die Stabilität prüfen. Schnellerer RAM hilft vor allem CPU-limitierten Spielen.",
  "The part number does not reveal the kit's rating. If the label on your modules shows a higher speed, enable XMP/EXPO in the BIOS.":
    "Die Teilenummer verrät den Nenntakt des Kits nicht. Steht auf dem Etikett der Module ein höherer Takt, XMP/EXPO im BIOS einschalten.",
  "The part number does not reveal the kit's rating; this speed is above the standard base speeds.":
    "Die Teilenummer verrät den Nenntakt des Kits nicht; dieser Takt liegt über den Standard-Grundtakten.",
  "Display refresh rate": "Bildwiederholrate",
  "The active displays could not be read.": "Die aktiven Bildschirme ließen sich nicht auslesen.",
  "Displays run at their highest refresh rate": "Die Bildschirme laufen mit ihrer höchsten Bildwiederholrate",
  "Optimizations for windowed games: on": "Optimierungen für Spiele im Fenstermodus: an",
  "DirectX 10/11 games in windowed or borderless mode use the modern presentation model (lower latency, Auto HDR and VRR in windowed mode).":
    "DirectX-10/11-Spiele im Fenster- oder randlosen Modus nutzen das moderne Darstellungsmodell (geringere Latenz, Auto HDR und VRR im Fenstermodus).",
  "Optimizations for windowed games: off": "Optimierungen für Spiele im Fenstermodus: aus",
  "Turning it on lets DirectX 10/11 games in windowed or borderless mode use the modern presentation model, with lower latency. Turn it off again if a game misbehaves.":
    "Eingeschaltet nutzen DirectX-10/11-Spiele im Fenster- oder randlosen Modus das moderne Darstellungsmodell mit geringerer Latenz. Wieder ausschalten, wenn ein Spiel Probleme macht.",
  "Optimizations for windowed games: Windows default": "Optimierungen für Spiele im Fenstermodus: Windows-Standard",
  "Not set explicitly. Check it under Settings > Display > Graphics; it can lower latency for DirectX 10/11 games in windowed or borderless mode.":
    "Nicht ausdrücklich gesetzt. Unter Einstellungen > Anzeige > Grafik prüfen; es kann die Latenz von DirectX-10/11-Spielen im Fenster- oder randlosen Modus senken.",
  "Variable refresh rate for all games: on": "Variable Bildwiederholrate für alle Spiele: an",
  "Games without their own VRR support also use G-SYNC/FreeSync.": "Auch Spiele ohne eigene VRR-Unterstützung nutzen G-SYNC/FreeSync.",
  "Variable refresh rate for all games: off": "Variable Bildwiederholrate für alle Spiele: aus",
  "Only useful if your monitor supports G-SYNC, FreeSync or VRR: then games without their own VRR support can use it too.":
    "Nur sinnvoll, wenn dein Monitor G-SYNC, FreeSync oder VRR unterstützt: Dann können es auch Spiele ohne eigene VRR-Unterstützung nutzen.",
  "Resizable BAR": "Resizable BAR",
  "Only NVIDIA GPUs can be checked here (via nvidia-smi). For AMD, see Smart Access Memory in AMD Software or the BIOS.":
    "Hier lassen sich nur NVIDIA-GPUs prüfen (über nvidia-smi). Bei AMD unter Smart Access Memory in der AMD Software oder im BIOS nachsehen.",
  "nvidia-smi did not report the BAR1 size.": "nvidia-smi hat die BAR1-Größe nicht gemeldet.",
  "Resizable BAR is active": "Resizable BAR ist aktiv",
  "Resizable BAR is off": "Resizable BAR ist aus",

  // ── optimizer: system checks ───────────────────────────────────────────────
  "Plan the update to the next Windows version.": "Plane das Update auf die nächste Windows-Version.",
  "Below about 10% free space, Windows updates and the page file can fail. Free up space with Cleanup or Disk analyzer.":
    "Unter etwa 10 % freiem Platz können Windows-Updates und die Auslagerungsdatei scheitern. Mit Bereinigung oder Festplattenanalyse Platz schaffen.",
  "The page file is disabled": "Die Auslagerungsdatei ist abgeschaltet",
  "Without a page file, programs can crash when memory runs out and no crash dumps are written. Re-enable automatic management under Advanced perf tweaks.":
    "Ohne Auslagerungsdatei können Programme abstürzen, wenn der Speicher voll ist, und es werden keine Absturzabbilder geschrieben. Die automatische Verwaltung unter den Leistungsoptimierungen wieder einschalten.",
  "Microsoft Defender real-time protection is off": "Der Echtzeitschutz von Microsoft Defender ist aus",
  "Turn it back on unless another antivirus is active. It is not a meaningful gaming bottleneck on modern PCs.":
    "Wieder einschalten, sofern kein anderer Virenschutz aktiv ist. Auf modernen PCs bremst er Spiele nicht nennenswert.",
  "Power saver plan is active on a desktop": "Auf einem Desktop-PC ist der Energiesparplan aktiv",
  "Power saver limits CPU clocks. Switch to Balanced or High performance under Power plan.":
    "Der Energiesparplan begrenzt die CPU-Takte. Unter Energieplan auf „Ausbalanciert“ oder „Höchstleistung“ wechseln.",
  "Windows runs from a hard disk": "Windows läuft von einer Festplatte",
  "A hard disk is the biggest bottleneck for boot and load times. No setting can fix that; an SSD can.":
    "Eine Festplatte ist die größte Bremse für Start- und Ladezeiten. Keine Einstellung behebt das, eine SSD schon.",
  "Current games commonly need 16 GB. No setting can replace memory.":
    "Aktuelle Spiele brauchen meist 16 GB. Keine Einstellung ersetzt Arbeitsspeicher.",
  "The graphics driver is more than a year old. New drivers often fix game issues; update from the vendor or the Drivers tool.":
    "Der Grafiktreiber ist über ein Jahr alt. Neue Treiber beheben oft Spielprobleme; beim Hersteller oder über das Treiber-Tool aktualisieren.",
  "A previous apply was interrupted. Open the undo history to restore those entries.":
    "Ein früheres Anwenden wurde unterbrochen. Im Rückgängig-Verlauf die betroffenen Einträge wiederherstellen.",
  "The health check is only available on Windows.": "Der Systemcheck ist nur unter Windows verfügbar.",
  "Only available on Windows.": "Nur unter Windows verfügbar.",
};

const hz = (s: string) =>
  s.replace(/ \(up to (\d+) Hz possible\)/g, " (bis $1 Hz möglich)").replace(/ Hz at /g, " Hz bei ");

type Pattern = [RegExp, (m: RegExpMatchArray, tr: (s: string) => string) => string];

const DE_PATTERNS: Pattern[] = [
  [/^Retired: no measurable benefit\. (.*) Reset restores the original values\.$/s,
    (m, tr) => `Ausgemustert: kein messbarer Nutzen. ${tr(m[1])} Zurücksetzen stellt die Originalwerte wieder her.`],
  [/^Applied, but not suitable here: (.*)$/s, (m, tr) => `Angewendet, passt aber nicht zu diesem PC: ${tr(m[1])}`],
  [/^Applied, but not recommended: (.*) Consider undoing it\.$/s,
    (m, tr) => `Angewendet, aber nicht empfohlen: ${tr(m[1])} Rückgängig machen erwägen.`],
  [/^Applied by this app earlier, but Windows no longer has that value \(changed by Windows, a driver, or another tool\)\. Undo closes the old entry and restores the values from before\.$/,
    () => "Früher von dieser App angewendet, aber Windows hat den Wert nicht mehr (geändert durch Windows, einen Treiber oder ein anderes Programm). Rückgängig schließt den alten Eintrag und stellt die vorherigen Werte wieder her."],
  [/^Applied by this app earlier, but Windows no longer has that value \(changed by Windows, a driver, or another tool\)\. Undo restores the values from before; apply it again afterwards if you still want it\.$/,
    () => "Früher von dieser App angewendet, aber Windows hat den Wert nicht mehr (geändert durch Windows, einen Treiber oder ein anderes Programm). Rückgängig stellt die vorherigen Werte wieder her; danach erneut anwenden, wenn du es weiter willst."],
  [/^Already covered by the applied "(.*)" tweak\.$/, (m) => `Bereits durch den angewendeten Tweak „${m[1]}“ abgedeckt.`],
  [/^The change could not be confirmed: Windows reports "(.*)"\. Use Undo to restore the previous values\.$/,
    (m) => `Die Änderung ließ sich nicht bestätigen: Windows meldet „${m[1]}“. Mit Rückgängig die vorherigen Werte wiederherstellen.`],
  [/^Memory runs at (\d+) MT\/s, the kit is rated for (\d+) MT\/s$/, (m) => `Der RAM läuft mit ${m[1]} MT/s, das Kit ist für ${m[2]} MT/s ausgelegt`],
  [/^Memory runs at its rated (\d+) MT\/s$/, (m) => `Der RAM läuft mit seinem Nenntakt von ${m[1]} MT/s`],
  [/^Matches the (\d+) MT\/s encoded in the part number, so XMP\/EXPO \(or equivalent\) is active\.$/,
    (m) => `Entspricht den ${m[1]} MT/s aus der Teilenummer, XMP/EXPO (oder Gleichwertiges) ist also aktiv.`],
  [/^Memory runs at (\d+) MT\/s, a standard base speed$/, (m) => `Der RAM läuft mit ${m[1]} MT/s, einem Standard-Grundtakt`],
  [/^Memory runs at (\d+) MT\/s$/, (m) => `Der RAM läuft mit ${m[1]} MT/s`],
  [/^(\d+) display\(s\) run below their maximum refresh rate$/, (m) => `${m[1]} Bildschirm(e) laufen unter ihrer höchsten Bildwiederholrate`],
  [/^(.*)\. Choose the highest rate under Settings > Display > Advanced display\.$/s,
    (m) => `${hz(m[1])}. Unter Einstellungen > Anzeige > Erweiterte Anzeige die höchste Rate wählen.`],
  // "Name: 240 Hz at 1920×1080; Name: 60 Hz at …" (several displays)
  [/ Hz at \d+×\d+/, (m) => hz(m.input ?? "")],
  [/^BAR1 (\d+) MiB covers the (\d+) MiB of video memory\. The NVIDIA driver uses it per game where it helps\.$/,
    (m) => `BAR1 mit ${m[1]} MiB deckt die ${m[2]} MiB Grafikspeicher ab. Der NVIDIA-Treiber nutzt es pro Spiel, wo es hilft.`],
  [/^BAR1 is only (\d+) MiB\. .*$/s,
    (m) => `BAR1 hat nur ${m[1]} MiB. „Above 4G Decoding“ und „Re-Size BAR Support“ im BIOS einzuschalten (UEFI-Start, CSM aus) hilft manchen Spielen; der Treiber nutzt es nur pro Spiel, wo es hilft.`],
  [/^(.+) no longer receives security updates$/, (m) => `${m[1]} erhält keine Sicherheitsupdates mehr`],
  [/^Support for this version ended on (.+)\. Update to a supported Windows version; no tweak can replace security updates\.$/,
    (m) => `Der Support für diese Version endete am ${m[1]}. Auf eine unterstützte Windows-Version aktualisieren; kein Tweak ersetzt Sicherheitsupdates.`],
  [/^(.+) support ends on (.+)$/, (m) => `Der Support für ${m[1]} endet am ${m[2]}`],
  [/^System drive almost full \((\d+)% free\)$/, (m) => `Systemlaufwerk fast voll (${m[1]} % frei)`],
  [/^(\d+) GB of RAM$/, (m) => `${m[1]} GB RAM`],
  [/^(.+): driver from (.+)$/, (m) => `${m[1]}: Treiber vom ${m[2]}`],
  [/^(\d+) change\(s\) did not finish$/, (m) => `${m[1]} Änderung(en) nicht abgeschlossen`],
  [/^System detection failed: (.*)$/s, (m) => `Systemerkennung fehlgeschlagen: ${m[1]}`],
  [/^No restore point was created \((.*)\); no driver was installed\.$/s,
    (m) => `Es wurde kein Wiederherstellungspunkt erstellt (${m[1]}); kein Treiber wurde installiert.`],
  [/^No restore point was created \((.*)\)\. Nothing was changed\. Untick the restore point to apply without one\.$/s,
    (m) => `Es wurde kein Wiederherstellungspunkt erstellt (${m[1]}). Nichts wurde geändert. Den Haken beim Wiederherstellungspunkt entfernen, um ohne ihn anzuwenden.`],
];

function translateDe(text: string): string {
  const exact = DE[text];
  if (exact !== undefined) return exact;
  for (const [re, fmt] of DE_PATTERNS) {
    const m = text.match(re);
    if (m) return fmt(m, translateDe);
  }
  return text;
}

/** Engine text in the UI language; unknown texts stay as they are. */
export function engineText(text: string | null | undefined, lang: Lang): string {
  if (!text) return "";
  return lang === "de" ? translateDe(text) : text;
}
