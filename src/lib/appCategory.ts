/** Rough category of an installed program, derived only from what the
 * uninstall registry entry says (name, publisher, install location, uninstall
 * command). It is a filter aid, not a verdict: unknown programs land in
 * "other". */
export type AppCategory =
  | "game"
  | "launcher"
  | "hardware"
  | "runtime"
  | "dev"
  | "browser"
  | "communication"
  | "media"
  | "other";

export const APP_CATEGORIES: AppCategory[] = [
  "game", "launcher", "hardware", "runtime", "dev", "browser", "communication", "media", "other",
];

type AppLike = { name?: string; publisher?: string; installLocation?: string; uninstallString?: string };

const RUNTIME = /visual c\+\+|redistributable|\.net (desktop |framework|runtime)|asp\.net|webview2 runtime|directx|java\(tm\)|java \d|openjdk|vulkan run ?time|microsoft update health/i;
// Store folders of the big launchers; the Steam uninstall command is steam://.
const GAME_PATH = /\\steamapps\\|steam:\/\/uninstall|\\epic games\\(?!launcher)|\\gog games\\|\\gog galaxy\\games\\|\\riot games\\(?!riot client)|\\ubisoft game launcher\\games\\|\\ea games\\|\\battlestate games\\|\\xboxgames\\/i;
const LAUNCHER = /^(steam|epic games launcher|battle\.net|ea app|ea desktop|origin|ubisoft connect|uplay|gog galaxy|riot client|curseforge|overwolf|battlestate games launcher|rockstar games launcher|playnite|heroic games launcher|xbox|amazon games|itch)\b/i;
const HARDWARE_PUBLISHER = /nvidia|advanced micro devices|^amd\b|intel|realtek|logitech|razer|corsair|steelseries|asustek|^asus|micro-star|^msi\b|gigabyte|asrock|elgato|hyperx|wooting|roccat|nzxt|western digital|samsung electronics|crucial|creative technology|focusrite|bluetooth/i;
const HARDWARE_NAME = /driver|chipset|firmware|bios|ryzen master|geforce|radeon|control center|armoury|icue|g hub|synapse/i;
const DEV = /visual studio|vs code|git( |$|hub)|jetbrains|intellij|pycharm|webstorm|rider|docker|python|node\.js|deno|^bun$|rust|rustup|cargo|go programming|golang|dbeaver|postman|insomnia|windows sdk|windows kits|cmake|llvm|mingw|msys2|android studio|actionlint|ast-grep|powershell \d|windows terminal|wsl|neovim|vim|notepad\+\+|sublime|heidisql|mysql|postgresql|mongodb|sql server|azure|terraform|kubectl|helm|nvm|yarn|pnpm|anaconda|miniconda/i;
const BROWSER = /chrome|firefox|microsoft edge|brave|opera|vivaldi|waterfox|librewolf|tor browser|arc\b/i;
const COMMUNICATION = /discord|teams|slack|zoom|teamspeak|skype|telegram|whatsapp|signal|mumble|element|thunderbird|outlook/i;
const MEDIA = /spotify|vlc|obs studio|audacity|itunes|foobar|winamp|mpc-hc|mpv|potplayer|handbrake|gimp|blender|krita|paint\.net|davinci|adobe|voicemeeter|nvidia broadcast|streamlabs|medal|shadowplay/i;

export function categorize(app: AppLike): AppCategory {
  const name = (app.name ?? "").trim();
  const publisher = (app.publisher ?? "").trim();
  const where = `${app.installLocation ?? ""} ${app.uninstallString ?? ""}`;
  if (RUNTIME.test(name)) return "runtime";
  if (LAUNCHER.test(name)) return "launcher";
  if (GAME_PATH.test(where)) return "game";
  if (HARDWARE_NAME.test(name) || HARDWARE_PUBLISHER.test(publisher)) return "hardware";
  if (DEV.test(name)) return "dev";
  if (BROWSER.test(name)) return "browser";
  if (COMMUNICATION.test(name)) return "communication";
  if (MEDIA.test(name)) return "media";
  return "other";
}

export type AppSort = "name" | "size" | "date" | "publisher";

/** InstallDate is yyyymmdd in the registry; anything else sorts last. */
const dateKey = (a: { installDate?: string }) => (/^\d{8}$/.test(a.installDate ?? "") ? a.installDate! : "");

export function sortApps<T extends AppLike & { sizeMb?: number; installDate?: string }>(apps: T[], by: AppSort): T[] {
  const byName = (a: T, b: T) => (a.name ?? "").localeCompare(b.name ?? "", undefined, { sensitivity: "base" });
  const out = [...apps];
  switch (by) {
    case "size": return out.sort((a, b) => (b.sizeMb ?? 0) - (a.sizeMb ?? 0) || byName(a, b));
    case "date": return out.sort((a, b) => dateKey(b).localeCompare(dateKey(a)) || byName(a, b));
    case "publisher":
      return out.sort((a, b) =>
        (a.publisher || "￿").localeCompare(b.publisher || "￿", undefined, { sensitivity: "base" }) || byName(a, b));
    default: return out.sort(byName);
  }
}
