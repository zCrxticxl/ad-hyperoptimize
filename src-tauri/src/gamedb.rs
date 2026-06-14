//! Static game database: process names, genre, per-preset in-game setting recommendations.
//! All values are best-practice community knowledge (no WMI/PS needed here).

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Game {
    pub id:          &'static str,
    pub name:        &'static str,
    pub processes:   &'static [&'static str], // lowercase exe names
    pub genre:       &'static str,
    pub competitive: bool,
    pub presets:     Presets,
    pub tips:        &'static [&'static str],
}

#[derive(Debug, Clone, Serialize)]
pub struct Presets {
    pub performance: Preset,
    pub balanced:    Preset,
    pub quality:     Preset,
}

#[derive(Debug, Clone, Serialize)]
pub struct Preset {
    pub power_plan:  &'static str, // "ultimate" | "high_performance" | "balanced"
    pub description: &'static str,
    pub settings:    &'static [S],
}

#[derive(Debug, Clone, Serialize)]
pub struct S {
    pub cat:   &'static str,
    pub name:  &'static str,
    pub value: &'static str,
}

const fn s(cat: &'static str, name: &'static str, value: &'static str) -> S {
    S { cat, name, value }
}

pub static GAMES: &[Game] = &[

// ─────────────────────────────────────────────
// COMPETITIVE FPS
// ─────────────────────────────────────────────

Game {
    id: "cs2", name: "Counter-Strike 2",
    processes: &["cs2.exe"],
    genre: "FPS Competitive", competitive: true,
    presets: Presets {
        performance: Preset {
            power_plan: "ultimate",
            description: "Minimum input lag, maximum FPS — the only mode for ranked play.",
            settings: &[
                s("Display",  "Resolution",              "1920×1080 or 1280×960 stretched"),
                s("Display",  "Display Mode",            "Fullscreen (not Borderless)"),
                s("Display",  "Refresh Rate",            "Monitor max"),
                s("Video",    "Global Shadow Quality",   "Very Low"),
                s("Video",    "Model / Texture Detail",  "Low"),
                s("Video",    "Effect Detail",           "Low"),
                s("Video",    "Shader Detail",           "Low"),
                s("Video",    "Boost Player Contrast",   "Enabled"),
                s("Video",    "Multicore Rendering",     "Enabled"),
                s("Video",    "Texture Filtering",       "Bilinear"),
                s("Video",    "Anti-Aliasing (MSAA)",    "None"),
                s("Video",    "V-Sync",                  "Disabled"),
                s("Video",    "Motion Blur",             "Disabled"),
                s("Reflex",   "NVIDIA Reflex",           "On + Boost"),
                s("Launch",   "Steam Launch Options",    "-novid -console -high +fps_max 0"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "Medium textures for readable models, low everything else.",
            settings: &[
                s("Video", "Global Shadow Quality",  "Medium"),
                s("Video", "Model / Texture Detail", "Medium"),
                s("Video", "Effect Detail",          "Low"),
                s("Video", "Shader Detail",          "Medium"),
                s("Video", "Texture Filtering",      "Trilinear"),
                s("Video", "Anti-Aliasing (MSAA)",   "None"),
                s("Video", "V-Sync",                 "Disabled"),
                s("Reflex", "NVIDIA Reflex",         "On"),
            ],
        },
        quality: Preset {
            power_plan: "high_performance",
            description: "Demo recording / content creation quality.",
            settings: &[
                s("Video", "Global Shadow Quality",  "High"),
                s("Video", "Model / Texture Detail", "High"),
                s("Video", "Effect Detail",          "High"),
                s("Video", "Shader Detail",          "High"),
                s("Video", "Texture Filtering",      "Anisotropic 16x"),
                s("Video", "Anti-Aliasing (MSAA)",   "4×MSAA"),
            ],
        },
    },
    tips: &[
        "Disable GeForce / Radeon overlay — adds measurable latency",
        "cl_interp_ratio 1  |  rate 786432  for 128-tick",
        "Disable Windows Game Mode — it adds scheduler latency",
        "Set Windows power plan to Ultimate Performance before launching",
    ],
},

Game {
    id: "valorant", name: "Valorant",
    processes: &["valorant-win64-shipping.exe", "vanguard.exe"],
    genre: "FPS Competitive", competitive: true,
    presets: Presets {
        performance: Preset {
            power_plan: "high_performance", // Ultimate can cause Vanguard issues
            description: "Lowest input lag — all visual fluff off.",
            settings: &[
                s("Display",  "Resolution",         "1920×1080 (native recommended)"),
                s("Display",  "Display Mode",       "Fullscreen"),
                s("Display",  "Limit FPS (Menu)",   "30"),
                s("Display",  "Limit FPS (BG)",     "10"),
                s("Display",  "Limit FPS (Always)", "Uncapped or monitor Hz + 10"),
                s("Graphics", "Material Quality",   "Low"),
                s("Graphics", "Texture Quality",    "Low"),
                s("Graphics", "Detail Quality",     "Low"),
                s("Graphics", "UI Quality",         "Low"),
                s("Graphics", "Vignette",           "Off"),
                s("Graphics", "V-Sync",             "Off"),
                s("Graphics", "Anti-Aliasing",      "None"),
                s("Graphics", "Enhanced Gunfire",   "Off"),
                s("Graphics", "Shadows",            "Off"),
                s("Graphics", "Bloom",              "Off"),
                s("Graphics", "Distortion",         "Off"),
                s("Graphics", "Cast Shadows",       "Off"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "Medium quality, still very competitive.",
            settings: &[
                s("Graphics", "Material Quality",   "Medium"),
                s("Graphics", "Texture Quality",    "Medium"),
                s("Graphics", "Anti-Aliasing",      "MSAA 2x"),
                s("Graphics", "Shadows",            "Medium"),
                s("Graphics", "V-Sync",             "Off"),
            ],
        },
        quality: Preset {
            power_plan: "high_performance",
            description: "Maximum visuals — not recommended for ranked.",
            settings: &[
                s("Graphics", "Material Quality",   "High"),
                s("Graphics", "Texture Quality",    "High"),
                s("Graphics", "Anti-Aliasing",      "MSAA 4x"),
                s("Graphics", "Shadows",            "High"),
                s("Graphics", "Bloom",              "On"),
                s("Graphics", "Distortion",         "On"),
            ],
        },
    },
    tips: &[
        "Do NOT use Ultimate Performance — can trigger Vanguard false flags",
        "Close all overlays (Discord, Steam, GeForce) — Vanguard scans them",
        "Cap FPS at monitor refresh + 10% to reduce GPU heat and coil whine",
        "Use Fullscreen, not Borderless — lower input lag",
    ],
},

Game {
    id: "apex", name: "Apex Legends",
    processes: &["r5apex.exe"],
    genre: "FPS Battle Royale", competitive: true,
    presets: Presets {
        performance: Preset {
            power_plan: "ultimate",
            description: "Max FPS — critical for tracking fast-moving targets.",
            settings: &[
                s("Display",  "Display Mode",              "Full Screen"),
                s("Display",  "Resolution",                "1920×1080"),
                s("Display",  "Field of View",             "110 (personal preference)"),
                s("Display",  "V-Sync",                    "Disabled"),
                s("Display",  "Adaptive Resolution FPS",   "0 (disabled)"),
                s("Video",    "Texture Streaming Budget",  "Medium (2–4 GB)"),
                s("Video",    "Texture Filtering",         "Anisotropic 2x"),
                s("Video",    "Ambient Occlusion Quality", "Disabled"),
                s("Video",    "Sun Shadow Coverage",       "Low"),
                s("Video",    "Sun Shadow Detail",         "Low"),
                s("Video",    "Spot Shadow Detail",        "Disabled"),
                s("Video",    "Volumetric Lighting",       "Disabled"),
                s("Video",    "Dynamic Spot Shadows",      "Disabled"),
                s("Video",    "Model Detail",              "Low"),
                s("Video",    "Effects Detail",            "Low"),
                s("Video",    "Impact Marks",              "Low"),
                s("Video",    "Ragdolls",                  "Medium"),
                s("Anti-Aliasing", "Mode",                 "None or TSAA"),
                s("NVIDIA",   "Reflex",                    "Enabled + Boost"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "Good performance + readable visuals.",
            settings: &[
                s("Video", "Texture Streaming Budget",  "High"),
                s("Video", "Ambient Occlusion Quality", "Low"),
                s("Video", "Sun Shadow Coverage",       "Medium"),
                s("Video", "Spot Shadow Detail",        "Low"),
                s("Video", "Effects Detail",            "Medium"),
                s("Video", "Model Detail",              "Medium"),
                s("Anti-Aliasing", "Mode",              "TSAA"),
            ],
        },
        quality: Preset {
            power_plan: "high_performance",
            description: "Maximum visuals — for content creation.",
            settings: &[
                s("Video", "Texture Streaming Budget",  "Very High"),
                s("Video", "Ambient Occlusion Quality", "High"),
                s("Video", "Sun Shadow Coverage",       "High"),
                s("Video", "Spot Shadow Detail",        "High"),
                s("Video", "Volumetric Lighting",       "Enabled"),
                s("Video", "Effects Detail",            "High"),
                s("Video", "Model Detail",              "High"),
            ],
        },
    },
    tips: &[
        "+fps_max unlimited in launch options (Origin/EA App)",
        "Adaptive Resolution FPS = 0 disables dynamic resolution which tanks clarity",
        "Medium Ragdolls — High ragdolls visible performance hit in late-game fights",
        "TSAA is the only AA option that doesn't look terrible — None only if FPS-starved",
    ],
},

Game {
    id: "fortnite", name: "Fortnite",
    processes: &["fortniteclient-win64-shipping.exe"],
    genre: "FPS Battle Royale", competitive: true,
    presets: Presets {
        performance: Preset {
            power_plan: "ultimate",
            description: "Performance Mode (DX11) — maximizes FPS for competitive.",
            settings: &[
                s("Display",  "Rendering Mode",       "Performance (Alpha) — DX11"),
                s("Display",  "Resolution",           "1920×1080"),
                s("Display",  "Window Mode",          "Fullscreen"),
                s("Display",  "Frame Rate Limit",     "Uncapped or monitor Hz"),
                s("Display",  "V-Sync",               "Off"),
                s("Display",  "Motion Blur",          "Off"),
                s("Display",  "Show FPS",             "On (to verify)"),
                s("Graphics", "3D Resolution",        "100%"),
                s("Graphics", "View Distance",        "Near (enemy rendering same regardless)"),
                s("Graphics", "Shadows",              "Off"),
                s("Graphics", "Anti-Aliasing",        "Off (Performance Mode)"),
                s("Graphics", "Textures",             "Low"),
                s("Graphics", "Effects",              "Low"),
                s("Graphics", "Post Processing",      "Low"),
                s("NVIDIA",   "Reflex",               "On + Boost"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "DX12 with medium settings — better visuals, still smooth.",
            settings: &[
                s("Display",  "Rendering Mode",  "DirectX 12"),
                s("Graphics", "Shadows",         "Medium"),
                s("Graphics", "Anti-Aliasing",   "TSR or TAA"),
                s("Graphics", "Textures",        "High"),
                s("Graphics", "Effects",         "Medium"),
                s("Graphics", "Post Processing", "Medium"),
                s("Display",  "V-Sync",         "Off"),
            ],
        },
        quality: Preset {
            power_plan: "high_performance",
            description: "Nanite + Lumen visuals — Unreal Engine 5 showcase.",
            settings: &[
                s("Display",  "Rendering Mode",          "Unreal Engine 5 (Nanite + Lumen)"),
                s("Graphics", "Shadows",                 "High"),
                s("Graphics", "Global Illumination",     "Lumen"),
                s("Graphics", "Reflections",             "Lumen"),
                s("Graphics", "Anti-Aliasing",           "TSR High"),
                s("Graphics", "Textures",                "Epic"),
                s("Graphics", "Effects",                 "Epic"),
                s("Graphics", "Post Processing",         "Epic"),
            ],
        },
    },
    tips: &[
        "Performance Mode (DX11) gives massive FPS uplift on mid-range GPUs",
        "View Distance Near — bush/tree density doesn't affect enemy player rendering",
        "Disable Motion Blur — obscures targets during builds",
        "Add -NOTEXTURESTREAMING to launch args for sharper textures in Perf Mode",
    ],
},

Game {
    id: "warzone", name: "Call of Duty: Warzone",
    processes: &["cod.exe", "modernwarfare.exe", "codmw2.exe"],
    genre: "FPS Battle Royale", competitive: true,
    presets: Presets {
        performance: Preset {
            power_plan: "ultimate",
            description: "Maximize FPS and enemy visibility.",
            settings: &[
                s("Display",  "Display Mode",              "Fullscreen Exclusive"),
                s("Display",  "V-Sync",                    "Disabled (all three settings)"),
                s("Display",  "NVIDIA Reflex",             "Enabled + Boost"),
                s("Display",  "Frame Rate Limit",          "Uncapped"),
                s("Quality",  "Render Resolution",         "100"),
                s("Quality",  "Upscaling / Sharpening",    "DLSS Quality or FSR2 Quality"),
                s("Quality",  "Texture Resolution",        "Normal"),
                s("Quality",  "Texture Filter Anisotropic","Normal"),
                s("Quality",  "Particle Quality",          "Low"),
                s("Quality",  "Bullet Impacts",            "Disabled"),
                s("Quality",  "Shader Quality",            "Low"),
                s("Quality",  "Shadow Map Resolution",     "Low"),
                s("Quality",  "Cache Spot Shadows",        "Disabled"),
                s("Quality",  "Cache Sun Shadows",         "Disabled"),
                s("Quality",  "Ambient Occlusion",         "Disabled"),
                s("Quality",  "Screen Space Reflection",   "Disabled"),
                s("Quality",  "Weather Grid Volumes",      "Disabled"),
                s("Quality",  "Water Caustics",            "Disabled"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "Playable quality with stable high FPS.",
            settings: &[
                s("Quality", "Texture Resolution",         "High"),
                s("Quality", "Shader Quality",             "Medium"),
                s("Quality", "Shadow Map Resolution",      "Medium"),
                s("Quality", "Ambient Occlusion",          "GTAO Medium"),
                s("Quality", "Upscaling",                  "DLSS Balanced"),
                s("Quality", "Anti-Aliasing",              "TAA"),
            ],
        },
        quality: Preset {
            power_plan: "high_performance",
            description: "Maximum visuals for screenshots or low-player-count modes.",
            settings: &[
                s("Quality", "Texture Resolution",         "Ultra"),
                s("Quality", "Shadow Map Resolution",      "High"),
                s("Quality", "Cache Spot Shadows",         "Enabled"),
                s("Quality", "Cache Sun Shadows",          "Enabled"),
                s("Quality", "Ambient Occlusion",          "GTAO High"),
                s("Quality", "Screen Space Reflection",    "Enabled"),
                s("Quality", "Anti-Aliasing",              "TAA High"),
            ],
        },
    },
    tips: &[
        "All 3 V-Sync options must be Off for lowest input lag",
        "Texture Resolution Normal vs High — minimal FPS difference, significant VRAM delta",
        "Depth of Field Off — blurs critical threat distances",
        "Film Grain 0.00 — improves visual clarity significantly",
    ],
},

Game {
    id: "overwatch2", name: "Overwatch 2",
    processes: &["overwatch.exe"],
    genre: "FPS Competitive", competitive: true,
    presets: Presets {
        performance: Preset {
            power_plan: "ultimate",
            description: "Highest FPS — crucial for tracking fast heroes.",
            settings: &[
                s("Display",  "Display Mode",      "Fullscreen"),
                s("Display",  "V-Sync",            "Off"),
                s("Display",  "Triple Buffering",  "Off"),
                s("Display",  "Reduce Buffering",  "On"),
                s("Display",  "Frame Rate Cap",    "Custom — monitor Hz or 300"),
                s("Graphics", "Render Scale",      "100%"),
                s("Graphics", "Graphics Quality",  "Low"),
                s("Graphics", "High Quality Upsampling", "Auto (or DLAA for 4K)"),
                s("Graphics", "Texture Quality",   "Medium"),
                s("Graphics", "Texture Filtering", "2x Anisotropic"),
                s("Graphics", "Local Fog Detail",  "Low"),
                s("Graphics", "Dynamic Reflections","Off"),
                s("Graphics", "Shadow Detail",     "Off"),
                s("Graphics", "Model Detail",      "Low"),
                s("Graphics", "Effects Detail",    "Low"),
                s("Graphics", "Lighting Quality",  "Low"),
                s("Graphics", "Antialias Quality", "Off"),
                s("Graphics", "Refraction Quality","Low"),
                s("Graphics", "Screenshot Quality","1x"),
                s("Graphics", "Ambient Occlusion", "Off"),
                s("Graphics", "Local Reflections", "Off"),
                s("Graphics", "Damage FX",         "Low"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "Medium quality, high FPS.",
            settings: &[
                s("Graphics", "Graphics Quality",   "Medium"),
                s("Graphics", "Texture Quality",    "High"),
                s("Graphics", "Shadow Detail",      "Medium"),
                s("Graphics", "Model Detail",       "Medium"),
                s("Graphics", "Antialias Quality",  "High"),
            ],
        },
        quality: Preset {
            power_plan: "high_performance",
            description: "Maximum visual fidelity.",
            settings: &[
                s("Graphics", "Graphics Quality",       "Ultra"),
                s("Graphics", "Texture Quality",        "Ultra"),
                s("Graphics", "Dynamic Reflections",    "On"),
                s("Graphics", "Shadow Detail",          "Ultra"),
                s("Graphics", "Local Reflections",      "On"),
                s("Graphics", "Ambient Occlusion",      "On"),
                s("Graphics", "Antialias Quality",      "Ultra"),
            ],
        },
    },
    tips: &[
        "Reduce Buffering ON — significantly lowers input lag",
        "High Frame Rate cap > 300 — Overwatch engine benefits from very high FPS even on 144Hz",
        "Texture Quality Medium vs High — nearly zero FPS difference",
        "Damage FX Low — reduces visual clutter during fights",
    ],
},

Game {
    id: "r6siege", name: "Rainbow Six Siege",
    processes: &["rainbowsix.exe"],
    genre: "FPS Tactical", competitive: true,
    presets: Presets {
        performance: Preset {
            power_plan: "ultimate",
            description: "Lowest input lag — operator/gadget clarity maintained.",
            settings: &[
                s("Display",  "Display Mode",          "Fullscreen"),
                s("Display",  "V-Sync",                "Off"),
                s("Display",  "FPS Limit",             "Uncapped"),
                s("Rendering","Rendering Scaling",     "100"),
                s("Rendering","Sharpening",            "50"),
                s("Graphics", "Texture Quality",       "High (enemy clarity)"),
                s("Graphics", "Texture Filter",        "Trilinear"),
                s("Graphics", "LOD Quality",           "High"),
                s("Graphics", "Shading Quality",       "Low"),
                s("Graphics", "Shadow Quality",        "Medium"),
                s("Graphics", "Spot Shadows",          "Off"),
                s("Graphics", "Contact Shadows",       "Off"),
                s("Graphics", "Ambient Occlusion",     "Off"),
                s("Graphics", "Lens Flare",            "Off"),
                s("Graphics", "Zoom-in Depth of Field","Off"),
                s("Graphics", "Anti-Aliasing",         "T-AA"),
                s("Graphics", "NVIDIA DLSS / FSR",     "Quality"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "Good balance of FPS and visual detail.",
            settings: &[
                s("Graphics", "Shading Quality",   "Medium"),
                s("Graphics", "Shadow Quality",    "High"),
                s("Graphics", "Ambient Occlusion", "SSBC Low"),
                s("Graphics", "Anti-Aliasing",     "T-AA"),
            ],
        },
        quality: Preset {
            power_plan: "high_performance",
            description: "Maximum visual fidelity.",
            settings: &[
                s("Graphics", "Shading Quality",   "Ultra"),
                s("Graphics", "Shadow Quality",    "Ultra"),
                s("Graphics", "Spot Shadows",      "High"),
                s("Graphics", "Contact Shadows",   "On"),
                s("Graphics", "Ambient Occlusion", "SSBC High"),
            ],
        },
    },
    tips: &[
        "Texture Quality High — directly affects how clearly you read opponents through doors/windows",
        "LOD Quality High — ensures distant operators don't pop in",
        "T-AA recommended: removes aliasing that can obscure targets at distance",
        "Vulkan renderer (launch option) can improve FPS on some AMD/NVIDIA configs",
    ],
},

// ─────────────────────────────────────────────
// MOBA / STRATEGY
// ─────────────────────────────────────────────

Game {
    id: "lol", name: "League of Legends",
    processes: &["league of legends.exe"],
    genre: "MOBA", competitive: true,
    presets: Presets {
        performance: Preset {
            power_plan: "high_performance",
            description: "Stable high FPS — LoL is CPU-limited on most PCs.",
            settings: &[
                s("Video", "Resolution",           "1920×1080"),
                s("Video", "Window Mode",          "Fullscreen"),
                s("Video", "Character Quality",    "Very High (affects gameplay clarity)"),
                s("Video", "Environment Quality",  "Low"),
                s("Video", "Effects Quality",      "Low"),
                s("Video", "Shadow Quality",       "No Shadows"),
                s("Video", "Frame Rate Cap",       "Uncapped or 240"),
                s("Video", "V-Sync",               "Unchecked"),
                s("Video", "Anti-Aliasing",        "Unchecked"),
                s("Video", "Wait for Vertical Sync","Unchecked"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "Balanced quality and performance.",
            settings: &[
                s("Video", "Character Quality",   "Very High"),
                s("Video", "Environment Quality", "Medium"),
                s("Video", "Effects Quality",     "Medium"),
                s("Video", "Shadow Quality",      "Medium"),
                s("Video", "Anti-Aliasing",       "Checked"),
            ],
        },
        quality: Preset {
            power_plan: "balanced",
            description: "Highest visual quality for streaming/content.",
            settings: &[
                s("Video", "Character Quality",   "Very High"),
                s("Video", "Environment Quality", "Very High"),
                s("Video", "Effects Quality",     "Very High"),
                s("Video", "Shadow Quality",      "Very High"),
                s("Video", "Anti-Aliasing",       "Checked"),
            ],
        },
    },
    tips: &[
        "Character Quality affects spell hit-box visibility — never lower than Medium",
        "Frame rate cap above your monitor Hz reduces GPU usage while maintaining performance",
        "LoL is primarily single-threaded — CPU clock speed matters more than core count",
    ],
},

Game {
    id: "dota2", name: "Dota 2",
    processes: &["dota2.exe"],
    genre: "MOBA", competitive: true,
    presets: Presets {
        performance: Preset {
            power_plan: "high_performance",
            description: "Max FPS for fast-paced teamfights.",
            settings: &[
                s("Video", "Rendering API",           "Vulkan (best perf on modern GPU)"),
                s("Video", "Resolution",              "1920×1080"),
                s("Video", "Display Mode",            "Fullscreen"),
                s("Video", "Shader Quality",          "Low"),
                s("Video", "Texture Quality",         "Medium"),
                s("Video", "Water Quality",           "No Water Effects"),
                s("Video", "Shadow Quality",          "Off"),
                s("Video", "Game Screen Render Quality","100%"),
                s("Video", "Anti-Aliasing",           "None"),
                s("Video", "Ambient Creatures",       "Off"),
                s("Video", "Specular",                "Off"),
                s("Video", "Atmospheric Fog / Caustics","Off"),
                s("Video", "V-Sync",                  "Off"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "Medium quality, high FPS.",
            settings: &[
                s("Video", "Shader Quality",   "Medium"),
                s("Video", "Texture Quality",  "High"),
                s("Video", "Shadow Quality",   "Medium"),
                s("Video", "Anti-Aliasing",    "FXAA"),
            ],
        },
        quality: Preset {
            power_plan: "balanced",
            description: "Maximum visual quality.",
            settings: &[
                s("Video", "Shader Quality",          "High"),
                s("Video", "Texture Quality",         "High"),
                s("Video", "Water Quality",           "Reflect All"),
                s("Video", "Shadow Quality",          "High"),
                s("Video", "Anti-Aliasing",           "MSAA 4x"),
                s("Video", "Ambient Creatures",       "On"),
                s("Video", "Atmospheric Fog / Caustics","On"),
            ],
        },
    },
    tips: &[
        "Vulkan API typically outperforms DX11/DX9 on Dota 2 by 10–20%",
        "Game Screen Render Quality 100% — reducing it makes heroes hard to identify",
    ],
},

// ─────────────────────────────────────────────
// OPEN WORLD / SINGLE PLAYER
// ─────────────────────────────────────────────

Game {
    id: "cyberpunk", name: "Cyberpunk 2077",
    processes: &["cyberpunk2077.exe"],
    genre: "Open World RPG", competitive: false,
    presets: Presets {
        performance: Preset {
            power_plan: "ultimate",
            description: "Stable 60+ FPS — no ray tracing, DLSS Performance.",
            settings: &[
                s("Display",   "Resolution",                "Native or DLSS/FSR output"),
                s("Display",   "V-Sync",                   "Off"),
                s("Graphics",  "Texture Quality",          "High"),
                s("Graphics",  "Field of View",            "90"),
                s("Graphics",  "Film Grain",               "Off"),
                s("Graphics",  "Chromatic Aberration",     "Off"),
                s("Graphics",  "Depth of Field",           "Off"),
                s("Graphics",  "Motion Blur",              "Off"),
                s("Graphics",  "Contact Shadows",          "Off"),
                s("Graphics",  "Improved Facial Lighting", "Off"),
                s("Graphics",  "Ambient Occlusion",        "Off"),
                s("Graphics",  "Reflections",              "Off"),
                s("Graphics",  "Screen Space Reflections", "Off"),
                s("Graphics",  "Volumetric Fog",           "Medium"),
                s("Graphics",  "Ray Tracing",              "Off (all)"),
                s("Upscaling", "DLSS / FSR Mode",          "Performance (1440p) or Quality (4K)"),
                s("Upscaling", "Frame Generation",         "On (RTX 40-series or RX 7000)"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "60 FPS with path tracing via DLSS3 Frame Generation.",
            settings: &[
                s("Graphics",  "Texture Quality",      "High"),
                s("Graphics",  "Ambient Occlusion",    "High"),
                s("Graphics",  "Reflections",          "High (SSR)"),
                s("Graphics",  "Ray Tracing",          "Medium preset"),
                s("Upscaling", "DLSS Mode",            "Quality"),
                s("Upscaling", "Frame Generation",     "On"),
            ],
        },
        quality: Preset {
            power_plan: "ultimate",
            description: "Path Tracing (Overdrive) — RTX 4080+ / RTX 3090 only.",
            settings: &[
                s("Graphics",  "Ray Tracing",          "Overdrive (Path Tracing)"),
                s("Graphics",  "Texture Quality",      "Ultra"),
                s("Graphics",  "Ambient Occlusion",    "Included in PT"),
                s("Graphics",  "Contact Shadows",      "On"),
                s("Graphics",  "Improved Facial Lighting", "On"),
                s("Upscaling", "DLSS Mode",            "Quality or Balanced"),
                s("Upscaling", "Frame Generation",     "On (required for playable FPS with PT)"),
                s("Upscaling", "DLSS Ray Reconstruction","On"),
            ],
        },
    },
    tips: &[
        "Film Grain / Chromatic Aberration / DOF off — massive clarity improvement with zero FPS cost",
        "Frame Generation requires RTX 4000 (DLSS3) or RX 7000 / RTX 3000+ (FSR3)",
        "Texture Quality High vs Ultra — minimal FPS delta, big VRAM delta (use High on <12 GB)",
        "Disable HDD-based city streaming stutter: enable ReBAR / Smart Access Memory in BIOS",
    ],
},

Game {
    id: "elden_ring", name: "Elden Ring",
    processes: &["eldenring.exe"],
    genre: "Action RPG", competitive: false,
    presets: Presets {
        performance: Preset {
            power_plan: "ultimate",
            description: "Stable 60 FPS — game engine caps at 60 natively.",
            settings: &[
                s("Graphics", "Texture Quality",  "Maximum"),
                s("Graphics", "Anti-Aliasing",    "High"),
                s("Graphics", "SSAO",             "Medium"),
                s("Graphics", "Depth of Field",   "Medium (gameplay tell)"),
                s("Graphics", "Motion Blur",      "Off"),
                s("Graphics", "Shadow Quality",   "Medium"),
                s("Graphics", "Lighting Quality", "Medium"),
                s("Graphics", "Effects Quality",  "Medium"),
                s("Graphics", "Volumetric Quality","Low"),
                s("Graphics", "Ray Tracing",      "Off"),
                s("Display",  "Frame Rate",       "60 (engine cap)"),
                s("Mod",      "FPS Unlocker",     "LimitFps=0 in .ini if using community mod"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "High quality within the 60 FPS cap.",
            settings: &[
                s("Graphics", "Shadow Quality",    "High"),
                s("Graphics", "Lighting Quality",  "High"),
                s("Graphics", "Effects Quality",   "High"),
                s("Graphics", "Volumetric Quality","Medium"),
                s("Graphics", "SSAO",              "High"),
            ],
        },
        quality: Preset {
            power_plan: "high_performance",
            description: "Maximum fidelity within 60 FPS.",
            settings: &[
                s("Graphics", "Shadow Quality",    "Maximum"),
                s("Graphics", "Lighting Quality",  "Maximum"),
                s("Graphics", "Effects Quality",   "Maximum"),
                s("Graphics", "Volumetric Quality","Maximum"),
                s("Graphics", "Ray Tracing",       "On (minimal visual impact)"),
            ],
        },
    },
    tips: &[
        "Texture Quality Maximum — negligible performance impact, huge visual improvement",
        "Motion Blur Off — essential: obscures parry timing windows",
        "Use the community FPS Unlocker mod to break the 60 FPS cap (120 FPS gameplay at higher Hz)",
        "Depth of Field on Medium: the game uses DOF as a gameplay cue for item highlights",
    ],
},

Game {
    id: "gtav", name: "GTA V",
    processes: &["gta5.exe"],
    genre: "Open World", competitive: false,
    presets: Presets {
        performance: Preset {
            power_plan: "high_performance",
            description: "Stable 60+ FPS in Online — avoids memory leak stutter.",
            settings: &[
                s("Graphics", "DirectX Version",            "DX11"),
                s("Graphics", "Screen Type",                "Fullscreen"),
                s("Graphics", "MSAA",                       "Off"),
                s("Graphics", "FXAA",                       "On"),
                s("Graphics", "VSync",                      "Off"),
                s("Graphics", "Population Density",         "Middle"),
                s("Graphics", "Population Variety",         "Middle"),
                s("Graphics", "Distance Scaling",           "Middle"),
                s("Graphics", "Texture Quality",            "Normal"),
                s("Graphics", "Shader Quality",             "High"),
                s("Graphics", "Shadow Quality",             "Normal"),
                s("Graphics", "Reflection Quality",         "Normal"),
                s("Graphics", "Reflection MSAA",            "Off"),
                s("Graphics", "Water Quality",              "High"),
                s("Graphics", "Particles Quality",          "Normal"),
                s("Graphics", "Grass Quality",              "Normal"),
                s("Graphics", "Ambient Occlusion",          "Off"),
                s("Graphics", "Anisotropic Filtering",      "x2"),
                s("Advanced", "Frame Scaling Mode",         "Off"),
                s("Advanced", "Extended Distance Scaling",  "Off"),
                s("Advanced", "High Resolution Shadows",    "Off"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "Solid quality with stable FPS.",
            settings: &[
                s("Graphics", "Texture Quality",    "High"),
                s("Graphics", "Shadow Quality",     "High"),
                s("Graphics", "Ambient Occlusion",  "Medium"),
                s("Graphics", "Reflection Quality", "High"),
                s("Graphics", "Anisotropic Filtering","x8"),
            ],
        },
        quality: Preset {
            power_plan: "high_performance",
            description: "Maximum RAGE engine visuals.",
            settings: &[
                s("Graphics", "Texture Quality",    "Very High"),
                s("Graphics", "Shadow Quality",     "Very High"),
                s("Graphics", "Ambient Occlusion",  "High"),
                s("Graphics", "MSAA",               "x4"),
                s("Graphics", "Anisotropic Filtering","x16"),
                s("Advanced", "High Resolution Shadows","On"),
                s("Advanced", "Long Shadows",        "On"),
            ],
        },
    },
    tips: &[
        "Texture Quality Very High requires 6+ GB VRAM — use Normal/High on <8 GB",
        "GTA Online stutter: disable cloud saves and set texture quality to Normal/High (not Very High)",
        "Population Density above 75% significantly increases CPU load in Online sessions",
    ],
},

Game {
    id: "tarkov", name: "Escape from Tarkov",
    processes: &["escapefromtarkov.exe"],
    genre: "FPS Hardcore", competitive: true,
    presets: Presets {
        performance: Preset {
            power_plan: "ultimate",
            description: "Stable FPS during raids — stutter costs lives in EFT.",
            settings: &[
                s("Graphics", "Screen Resolution",      "1920×1080"),
                s("Graphics", "Screen Mode",            "Fullscreen"),
                s("Graphics", "VSync",                  "Off"),
                s("Graphics", "Overall Graphics Quality","Low"),
                s("Graphics", "Antialiasing",           "Off"),
                s("Graphics", "Shadow Visibility",      "40 m"),
                s("Graphics", "LOD",                    "2"),
                s("Graphics", "Overall Visibility",     "1000"),
                s("Graphics", "Shadow Quality",         "Low"),
                s("Graphics", "Texture Quality",        "High (enemy visibility)"),
                s("Graphics", "Sharpness",              "1.3–1.6"),
                s("Graphics", "Anisotropic Filtering",  "Per Texture"),
                s("Graphics", "SSAO",                   "Disabled"),
                s("Graphics", "Contact Shadows",        "Disabled"),
                s("Graphics", "Object Illumination",    "0"),
                s("Graphics", "Grass Shadows",          "Disabled"),
                s("Graphics", "Rain Quality",           "Low"),
                s("Graphics", "Resampling",             "1x Off"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "Better visuals while maintaining playable FPS.",
            settings: &[
                s("Graphics", "Overall Graphics Quality","Medium"),
                s("Graphics", "Shadow Quality",         "Medium"),
                s("Graphics", "LOD",                    "3"),
                s("Graphics", "Antialiasing",           "TAA"),
            ],
        },
        quality: Preset {
            power_plan: "high_performance",
            description: "High quality for streaming.",
            settings: &[
                s("Graphics", "Overall Graphics Quality","High"),
                s("Graphics", "Shadow Quality",         "High"),
                s("Graphics", "SSAO",                   "Enabled"),
                s("Graphics", "Antialiasing",           "TAA High"),
            ],
        },
    },
    tips: &[
        "Clear shader cache folder before major updates — eliminates first-raid stutter",
        "Texture Quality High regardless of preset — affects enemy/loot identification",
        "Shadow Visibility 40m is optimal: further shadows add CPU cost with no gameplay benefit",
        "Disable GeForce overlay, Discord overlay, and all background recording during raids",
        "LOD 2 is sweet spot — LOD 1 causes visible pop-in on scoped shots",
    ],
},

Game {
    id: "rust", name: "Rust",
    processes: &["rustclient.exe"],
    genre: "Survival Multiplayer", competitive: true,
    presets: Presets {
        performance: Preset {
            power_plan: "ultimate",
            description: "Stable FPS in large bases and combat zones.",
            settings: &[
                s("Graphics", "Graphics Quality",  "0–1"),
                s("Graphics", "Shadow Quality",    "0"),
                s("Graphics", "Shadow Cascades",   "0"),
                s("Graphics", "Shadow Distance",   "0"),
                s("Graphics", "Anisotropic Filtering","0"),
                s("Graphics", "Particle Quality",  "0"),
                s("Graphics", "Object Quality",    "100"),
                s("Graphics", "Tree Quality",      "50"),
                s("Graphics", "Grass Quality",     "0"),
                s("Graphics", "Grass Displacement","Off"),
                s("Graphics", "Parallax Mapping",  "Off"),
                s("Graphics", "Ambient Occlusion", "Off"),
                s("Graphics", "Depth of Field",    "Off"),
                s("Graphics", "Motion Blur",       "Off"),
                s("Graphics", "Bloom",             "Off"),
                s("Graphics", "Lens Dirt",         "Off"),
                s("Graphics", "Sharpen",           "On (1.0)"),
                s("Graphics", "Max Gibs",          "0"),
                s("Graphics", "Draw Distance",     "1500"),
                s("Advanced", "Max Renderable Objects","1500"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "Looks reasonable without destroying FPS.",
            settings: &[
                s("Graphics", "Graphics Quality",  "2"),
                s("Graphics", "Shadow Quality",    "1"),
                s("Graphics", "Grass Quality",     "50"),
                s("Graphics", "Particle Quality",  "50"),
                s("Graphics", "Object Quality",    "150"),
            ],
        },
        quality: Preset {
            power_plan: "high_performance",
            description: "Nice visuals for screenshots.",
            settings: &[
                s("Graphics", "Graphics Quality",  "4"),
                s("Graphics", "Shadow Quality",    "3"),
                s("Graphics", "Ambient Occlusion", "On"),
                s("Graphics", "Grass Quality",     "100"),
                s("Graphics", "Depth of Field",    "On"),
                s("Graphics", "Bloom",             "On"),
            ],
        },
    },
    tips: &[
        "Grass Quality 0 is contentious — provides visibility advantage but not bannable",
        "Max Gibs 0 eliminates the GPU spike from explosive/death effects",
        "Object Quality 100 is minimum to see player models clearly at distance",
        "Use F2 console: gfx.ssaa false, gfx.taa true for cleaner AA with less performance cost",
    ],
},

Game {
    id: "minecraft", name: "Minecraft (Java)",
    processes: &["javaw.exe", "minecraft.exe"],
    genre: "Sandbox", competitive: false,
    presets: Presets {
        performance: Preset {
            power_plan: "high_performance",
            description: "High FPS with Sodium/OptiFine — smooth multiplayer and redstone.",
            settings: &[
                s("Video",  "Render Distance",          "8–12 chunks"),
                s("Video",  "Simulation Distance",      "8 chunks"),
                s("Video",  "Max Framerate",            "Uncapped or 2× monitor Hz"),
                s("Video",  "Smooth Lighting",          "Off"),
                s("Video",  "Clouds",                   "Off"),
                s("Video",  "Weather",                  "Off"),
                s("Video",  "Entity Shadows",           "Off"),
                s("Video",  "Mipmap Levels",            "0"),
                s("Video",  "Anisotropic Filtering",    "Off (OptiFine)"),
                s("Video",  "Anti-Aliasing (OptiFine)", "Off"),
                s("Video",  "Dynamic Lights",           "Off"),
                s("JVM",    "RAM Allocation",           "-Xmx6G -Xms6G (launcher JVM args)"),
                s("JVM",    "GC",                       "-XX:+UseG1GC in launcher"),
                s("Mod",    "Performance Mods",         "Sodium + Lithium + Phosphor (Fabric)"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "Good performance with Optifine-level shaders.",
            settings: &[
                s("Video",  "Render Distance",    "12–16 chunks"),
                s("Video",  "Smooth Lighting",    "On"),
                s("Video",  "Clouds",             "Fancy"),
                s("Video",  "Mipmap Levels",      "4"),
                s("Shaders","Shader Pack",        "ComplementaryUnbound (low preset)"),
                s("JVM",    "RAM Allocation",     "-Xmx8G"),
            ],
        },
        quality: Preset {
            power_plan: "balanced",
            description: "Cinematic shaders — screenshot and video quality.",
            settings: &[
                s("Video",  "Render Distance",    "20–32 chunks"),
                s("Shaders","Shader Pack",        "BSL or Complementary (high preset)"),
                s("Shaders","Shadows",            "2048+ resolution"),
                s("JVM",    "RAM Allocation",     "-Xmx10G"),
            ],
        },
    },
    tips: &[
        "Sodium (Fabric) outperforms OptiFine on modern Java — 2–3× FPS improvement",
        "Allocate 6 GB RAM max — over-allocation causes GC pauses (worse than too little)",
        "-XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 JVM flags for smoother GC",
        "Simulation Distance separate from Render Distance — keep simulation lower than render",
    ],
},

Game {
    id: "pubg", name: "PUBG: Battlegrounds",
    processes: &["tslgame.exe"],
    genre: "FPS Battle Royale", competitive: true,
    presets: Presets {
        performance: Preset {
            power_plan: "ultimate",
            description: "Maximum FPS — enemy visibility is paramount.",
            settings: &[
                s("Display",  "Screen Mode",               "Fullscreen"),
                s("Display",  "Display Resolution",        "1920×1080"),
                s("Display",  "FPS Limit",                 "Uncapped"),
                s("Display",  "VSync",                     "Off"),
                s("Display",  "Motion Blur",               "Off"),
                s("Graphics", "Overall Quality",           "Custom"),
                s("Graphics", "Anti-Aliasing",             "Ultra (paradoxically cheaper in UE4)"),
                s("Graphics", "Post-Processing",           "Very Low"),
                s("Graphics", "Shadows",                   "Very Low"),
                s("Graphics", "Textures",                  "Ultra (spotting enemies in bushes)"),
                s("Graphics", "Effects",                   "Very Low"),
                s("Graphics", "Foliage",                   "Very Low"),
                s("Graphics", "View Distance",             "Ultra (see enemies further)"),
                s("Graphics", "Sharpen",                   "On"),
                s("NVIDIA",   "Reflex",                    "Enabled + Boost"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "Good visuals with high FPS.",
            settings: &[
                s("Graphics", "Anti-Aliasing",  "Ultra"),
                s("Graphics", "Post-Processing","Low"),
                s("Graphics", "Shadows",        "Low"),
                s("Graphics", "Textures",       "Ultra"),
                s("Graphics", "Foliage",        "Low"),
                s("Graphics", "View Distance",  "Ultra"),
            ],
        },
        quality: Preset {
            power_plan: "high_performance",
            description: "Full visual quality.",
            settings: &[
                s("Graphics", "Anti-Aliasing",  "Ultra"),
                s("Graphics", "Post-Processing","Ultra"),
                s("Graphics", "Shadows",        "Ultra"),
                s("Graphics", "Textures",       "Ultra"),
                s("Graphics", "Foliage",        "Ultra"),
                s("Graphics", "View Distance",  "Ultra"),
            ],
        },
    },
    tips: &[
        "Textures Ultra + View Distance Ultra — these two have the biggest gameplay impact regardless of preset",
        "Anti-Aliasing Ultra in PUBG actually runs faster than lower settings due to UE4 implementation",
        "Foliage Very Low: significant FPS gain, reduces hiding spots (not bannable, server-side)",
        "Motion Blur Off — reduces target tracking clarity significantly",
    ],
},

Game {
    id: "rdr2", name: "Red Dead Redemption 2",
    processes: &["rdr2.exe"],
    genre: "Open World", competitive: false,
    presets: Presets {
        performance: Preset {
            power_plan: "ultimate",
            description: "Stable 60 FPS — RDR2 is CPU-limited in towns.",
            settings: &[
                s("Display",  "VSync",                    "Off"),
                s("Display",  "Triple Buffering",         "Off"),
                s("Display",  "Frame Rate Cap",           "60"),
                s("Graphics", "Texture Quality",          "Ultra"),
                s("Graphics", "Anisotropic Filtering",    "x8"),
                s("Graphics", "Lighting Quality",         "Medium"),
                s("Graphics", "Global Illumination",      "Medium"),
                s("Graphics", "Shadow Quality",           "Medium"),
                s("Graphics", "Far Shadow Quality",       "Medium"),
                s("Graphics", "Screen Space Ambient Occlusion","Medium"),
                s("Graphics", "Reflection Quality",       "High"),
                s("Graphics", "Mirror Quality",           "High"),
                s("Graphics", "Water Quality",            "High"),
                s("Graphics", "Volumetric Lighting",      "Low"),
                s("Graphics", "Volumetric Quality",       "Low"),
                s("Graphics", "Particle Lighting",        "Medium"),
                s("Graphics", "Soft Shadows",             "Medium"),
                s("Graphics", "MSAA",                     "Off"),
                s("Graphics", "TAA",                      "High + DLSS/FSR"),
                s("Graphics", "Motion Blur",              "Off"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "Beautiful and smooth.",
            settings: &[
                s("Graphics", "Lighting Quality",    "High"),
                s("Graphics", "Shadow Quality",      "High"),
                s("Graphics", "Volumetric Lighting", "Medium"),
                s("Graphics", "SSAO",                "High"),
                s("Graphics", "TAA",                 "High"),
            ],
        },
        quality: Preset {
            power_plan: "ultimate",
            description: "One of the best-looking games on PC at max settings.",
            settings: &[
                s("Graphics", "Lighting Quality",    "Ultra"),
                s("Graphics", "Shadow Quality",      "Ultra"),
                s("Graphics", "Far Shadow Quality",  "Ultra"),
                s("Graphics", "Volumetric Lighting", "Ultra"),
                s("Graphics", "SSAO",                "Ultra"),
                s("Graphics", "Water Quality",       "Ultra"),
                s("Graphics", "MSAA",                "x4"),
            ],
        },
    },
    tips: &[
        "Texture Quality Ultra — uses VRAM but no performance hit on 8+ GB",
        "Frame Rate Cap 60 — game engine is optimized for 60; uncapped causes physics quirks",
        "Vulkan API generally outperforms DX12 on NVIDIA hardware",
        "Towns like Saint Denis are CPU-bound — no GPU setting will help there",
    ],
},

Game {
    id: "bf2042", name: "Battlefield 2042",
    processes: &["bf2042.exe"],
    genre: "FPS Multiplayer", competitive: true,
    presets: Presets {
        performance: Preset {
            power_plan: "ultimate",
            description: "Stable FPS in large 128-player battles.",
            settings: &[
                s("Display",  "Screen Mode",          "Fullscreen"),
                s("Display",  "VSync",                "Off"),
                s("Display",  "Frame Rate Limit",     "Uncapped"),
                s("Graphics", "Resolution Scale",     "100%"),
                s("Graphics", "Future Frame Rendering","On"),
                s("Graphics", "NVIDIA Reflex",        "On + Boost"),
                s("Graphics", "Texture Quality",      "Medium"),
                s("Graphics", "Texture Filtering",    "Anisotropic 4x"),
                s("Graphics", "Lighting Quality",     "Low"),
                s("Graphics", "Effects Quality",      "Low"),
                s("Graphics", "Post Process Quality", "Low"),
                s("Graphics", "Mesh Quality",         "Low"),
                s("Graphics", "Terrain Quality",      "Low"),
                s("Graphics", "Undergrowth Quality",  "Low"),
                s("Graphics", "Anti-Aliasing",        "TAA"),
                s("Graphics", "Ambient Occlusion",    "Off"),
                s("Graphics", "Motion Blur",          "Off"),
                s("Graphics", "Depth of Field",       "Off"),
                s("Graphics", "Film Grain",           "Off"),
                s("Graphics", "Chromatic Aberration", "Off"),
                s("Graphics", "Vignette",             "Off"),
            ],
        },
        balanced: Preset {
            power_plan: "high_performance",
            description: "Good balance for 128-player maps.",
            settings: &[
                s("Graphics", "Texture Quality",     "High"),
                s("Graphics", "Lighting Quality",    "Medium"),
                s("Graphics", "Effects Quality",     "Medium"),
                s("Graphics", "Mesh Quality",        "Medium"),
                s("Graphics", "Ambient Occlusion",   "SSAO"),
                s("Graphics", "Anti-Aliasing",       "TAA"),
            ],
        },
        quality: Preset {
            power_plan: "high_performance",
            description: "Maximum visual quality.",
            settings: &[
                s("Graphics", "Texture Quality",     "Ultra"),
                s("Graphics", "Lighting Quality",    "Ultra"),
                s("Graphics", "Effects Quality",     "Ultra"),
                s("Graphics", "Mesh Quality",        "Ultra"),
                s("Graphics", "Ambient Occlusion",   "HBAO+"),
                s("Graphics", "Anti-Aliasing",       "TAA High"),
            ],
        },
    },
    tips: &[
        "Future Frame Rendering On — reduces CPU-bottleneck stutter in large battles",
        "Film Grain / Chromatic Aberration / Vignette all Off — increases visual clarity with no FPS cost",
        "Undergrowth Quality Low — biggest single FPS gain setting in BF2042",
    ],
},

]; // end GAMES

/// Find a game by any of its process names (case-insensitive).
#[allow(dead_code)]
pub fn find_by_process(exe: &str) -> Option<&'static Game> {
    let lower = exe.to_lowercase();
    GAMES.iter().find(|g| g.processes.iter().any(|p| *p == lower))
}

pub fn get_all() -> &'static [Game] { GAMES }
