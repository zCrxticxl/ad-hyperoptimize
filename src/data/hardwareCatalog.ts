// ─────────────────────────────────────────────────────────────────────────────
// Curated hardware reference catalog for the Bottleneck Analyzer + PC
// Configurator. Scores are a relative composite performance index (0-100,
// gaming-weighted), normalized within each category (GPU/CPU/RAM/Storage)
// against the fastest real part in that category. Sourced from:
//   - GPU:     TechPowerUp "Relative Performance" table (real aggregated
//              review-benchmark data across 1080p/1440p/4K, anchored to the
//              fastest current card = 100%), read directly from
//              techpowerup.com/gpu-specs — it IS fetchable; an earlier pass
//              wrongly assumed it was JS-only and substituted PassMark's
//              G3D Mark, which compresses the high-end gap far too much
//              (e.g. shows a 4090 at ~98% of a 5090, vs TechPowerUp/reality's
//              ~76%) and was reverted after re-verification.
//   - CPU:     PassMark cpubenchmark.net "Top Gaming CPUs" chart (composite of
//              Single-Thread + Prime + Physics tests + L3 cache factor,
//              explicitly gaming-weighted). Chips too old to appear on that
//              chart (mostly LGA1151/LGA1200 8th-11th gen) are scored via
//              PassMark's Single-Thread Rating as a documented fallback proxy
//              — flagged inline below.
//   - RAM:     Real AIDA64 bandwidth/latency deltas between DDR4/DDR5 speed
//              grades + published gaming FPS uplift figures (Hardware
//              Unboxed/Tom's Hardware-class CPU-bound benchmarks). Capacity
//              tiers are penalized below/above the empirically-validated
//              32GB gaming sweet spot, not scored as "more is always better."
//   - Storage: Tom's Hardware SSD Benchmarks Hierarchy (Seq MB/s, Random
//              IOPS, Overall Score) for NVMe/SATA tiers; StorageReview/vendor
//              datasheets for 7200RPM HDD figures. HDD scores are
//              deliberately compressed near the floor — real HDDs are
//              10-50x+ slower in throughput/IOPS than NVMe, not ~20% slower
//              as the old hand-estimated scores implied.
// These are NOT official vendor metrics, and PassMark/AIDA64/Tom's figures
// drift over time — treat as a representative snapshot, re-derive
// periodically. Prices are rough USD market estimates and WILL drift; every
// part links out to live retailer search results via buyQuery. Treat this as
// a representative spread (budget → flagship, current + the last few
// generations that dominate the used market), not an exhaustive SKU list —
// new parts can be appended without touching any other file.
// ─────────────────────────────────────────────────────────────────────────────

export type Vendor = "Intel" | "AMD" | "NVIDIA";

export interface CatalogPart {
  id: string;
  name: string;
  vendor: Vendor;
  /** Relative composite performance index, 0-100. */
  score: number;
  tdpW: number;
  year: number;
  priceNew: number | null; // null = no longer sold new
  priceUsed: number;
  /** Free-text search query used to build retailer links. */
  buyQuery: string;
}

export interface Cpu extends CatalogPart {
  socket: string;
  cores: number;
  threads: number;
}

export interface Gpu extends CatalogPart {
  vramGb: number;
}

export interface RamKit {
  id: string;
  name: string;
  type: "DDR4" | "DDR5";
  capacityGb: number;
  speedMtps: number;
  score: number;
  priceNew: number;
  priceUsed: number;
  buyQuery: string;
}

export interface Motherboard {
  id: string;
  name: string;
  vendor: string;
  socket: string;
  chipset: string;
  ramType: "DDR4" | "DDR5";
  formFactor: "ATX" | "mATX" | "ITX";
  priceNew: number;
  priceUsed: number;
  buyQuery: string;
}

export interface StorageDrive {
  id: string;
  name: string;
  kind: "NVMe" | "SATA SSD" | "HDD";
  capacityGb: number;
  score: number;
  priceNew: number;
  priceUsed: number;
  buyQuery: string;
}

export interface Psu {
  id: string;
  name: string;
  watts: number;
  priceNew: number;
  priceUsed: number;
  buyQuery: string;
}

// ── GPUs ──────────────────────────────────────────────────────────────────────
// Scores: TechPowerUp Relative Performance %, RTX 5090 = 100. This is a raw
// throughput ratio, not a "gaming adequacy" score — see ratingFromGpuScore()
// in pcAdvisor.ts for how this gets banded into bottleneck/good/excellent
// (a flat 30/55/80 split would wrongly flag e.g. a 19%-of-5090 RTX 3060,
// still a perfectly playable 1080p/1440p card, as a "bottleneck").
export const GPUS: Gpu[] = [
  { id: "rtx5090",      name: "GeForce RTX 5090",        vendor: "NVIDIA", vramGb: 32, tdpW: 575, score: 100, year: 2025, priceNew: 2600, priceUsed: 2200, buyQuery: "NVIDIA GeForce RTX 5090" },
  { id: "rtx5080",      name: "GeForce RTX 5080",        vendor: "NVIDIA", vramGb: 16, tdpW: 360, score: 66,  year: 2025, priceNew: 1300, priceUsed: 1050, buyQuery: "NVIDIA GeForce RTX 5080" },
  { id: "rtx5070ti",    name: "GeForce RTX 5070 Ti",     vendor: "NVIDIA", vramGb: 16, tdpW: 300, score: 57,  year: 2025, priceNew: 950,  priceUsed: 780,  buyQuery: "NVIDIA GeForce RTX 5070 Ti" },
  { id: "rtx5070",      name: "GeForce RTX 5070",        vendor: "NVIDIA", vramGb: 12, tdpW: 250, score: 45,  year: 2025, priceNew: 600,  priceUsed: 480,  buyQuery: "NVIDIA GeForce RTX 5070" },
  { id: "rtx5060ti16",  name: "GeForce RTX 5060 Ti 16GB",vendor: "NVIDIA", vramGb: 16, tdpW: 180, score: 32,  year: 2025, priceNew: 430,  priceUsed: 340,  buyQuery: "NVIDIA GeForce RTX 5060 Ti 16GB" },
  { id: "rtx5060",      name: "GeForce RTX 5060",        vendor: "NVIDIA", vramGb: 8,  tdpW: 150, score: 23,  year: 2025, priceNew: 330,  priceUsed: 260,  buyQuery: "NVIDIA GeForce RTX 5060" },
  { id: "rtx4090",      name: "GeForce RTX 4090",        vendor: "NVIDIA", vramGb: 24, tdpW: 450, score: 76,  year: 2022, priceNew: null, priceUsed: 1400, buyQuery: "NVIDIA GeForce RTX 4090" },
  { id: "rtx4080",      name: "GeForce RTX 4080",        vendor: "NVIDIA", vramGb: 16, tdpW: 320, score: 58,  year: 2022, priceNew: null, priceUsed: 680,  buyQuery: "NVIDIA GeForce RTX 4080" },
  { id: "rtx4080super", name: "GeForce RTX 4080 Super",  vendor: "NVIDIA", vramGb: 16, tdpW: 320, score: 59,  year: 2024, priceNew: 1100, priceUsed: 820,  buyQuery: "NVIDIA GeForce RTX 4080 Super" },
  { id: "rtx4070tisuper",name: "GeForce RTX 4070 Ti Super",vendor: "NVIDIA", vramGb: 16, tdpW: 285, score: 49,  year: 2024, priceNew: 850,  priceUsed: 650,  buyQuery: "NVIDIA GeForce RTX 4070 Ti Super" },
  { id: "rtx4070ti",    name: "GeForce RTX 4070 Ti",     vendor: "NVIDIA", vramGb: 12, tdpW: 285, score: 45,  year: 2023, priceNew: null, priceUsed: 520,  buyQuery: "NVIDIA GeForce RTX 4070 Ti" },
  { id: "rtx4070super", name: "GeForce RTX 4070 Super",  vendor: "NVIDIA", vramGb: 12, tdpW: 220, score: 42,  year: 2024, priceNew: 650,  priceUsed: 480,  buyQuery: "NVIDIA GeForce RTX 4070 Super" },
  { id: "rtx4070",      name: "GeForce RTX 4070",        vendor: "NVIDIA", vramGb: 12, tdpW: 200, score: 36,  year: 2023, priceNew: null, priceUsed: 380,  buyQuery: "NVIDIA GeForce RTX 4070" },
  { id: "rtx4060ti16",  name: "GeForce RTX 4060 Ti 16GB",vendor: "NVIDIA", vramGb: 16, tdpW: 165, score: 24,  year: 2023, priceNew: 470,  priceUsed: 350,  buyQuery: "NVIDIA GeForce RTX 4060 Ti 16GB" },
  { id: "rtx4060",      name: "GeForce RTX 4060",        vendor: "NVIDIA", vramGb: 8,  tdpW: 115, score: 23,  year: 2023, priceNew: 330,  priceUsed: 250,  buyQuery: "NVIDIA GeForce RTX 4060" },
  { id: "rtx3090ti",    name: "GeForce RTX 3090 Ti",     vendor: "NVIDIA", vramGb: 24, tdpW: 450, score: 49,  year: 2022, priceNew: null, priceUsed: 950,  buyQuery: "NVIDIA GeForce RTX 3090 Ti" },
  { id: "rtx3090",      name: "GeForce RTX 3090",        vendor: "NVIDIA", vramGb: 24, tdpW: 350, score: 44,  year: 2020, priceNew: null, priceUsed: 850,  buyQuery: "NVIDIA GeForce RTX 3090" },
  { id: "rtx3080ti",    name: "GeForce RTX 3080 Ti",     vendor: "NVIDIA", vramGb: 12, tdpW: 350, score: 43,  year: 2021, priceNew: null, priceUsed: 550,  buyQuery: "NVIDIA GeForce RTX 3080 Ti" },
  { id: "rtx3080",      name: "GeForce RTX 3080",        vendor: "NVIDIA", vramGb: 10, tdpW: 320, score: 39,  year: 2020, priceNew: null, priceUsed: 420,  buyQuery: "NVIDIA GeForce RTX 3080" },
  { id: "rtx3070",      name: "GeForce RTX 3070",        vendor: "NVIDIA", vramGb: 8,  tdpW: 220, score: 28,  year: 2020, priceNew: null, priceUsed: 280,  buyQuery: "NVIDIA GeForce RTX 3070" },
  { id: "rtx3060ti",    name: "GeForce RTX 3060 Ti",     vendor: "NVIDIA", vramGb: 8,  tdpW: 200, score: 24,  year: 2021, priceNew: null, priceUsed: 240,  buyQuery: "NVIDIA GeForce RTX 3060 Ti" },
  { id: "rtx3060",      name: "GeForce RTX 3060",        vendor: "NVIDIA", vramGb: 12, tdpW: 170, score: 19,  year: 2021, priceNew: null, priceUsed: 200,  buyQuery: "NVIDIA GeForce RTX 3060" },
  { id: "rtx3050",      name: "GeForce RTX 3050",        vendor: "NVIDIA", vramGb: 8,  tdpW: 130, score: 13,  year: 2022, priceNew: 230,  priceUsed: 160,  buyQuery: "NVIDIA GeForce RTX 3050" },
  { id: "rtx2080ti",    name: "GeForce RTX 2080 Ti",     vendor: "NVIDIA", vramGb: 11, tdpW: 250, score: 28,  year: 2018, priceNew: null, priceUsed: 280,  buyQuery: "NVIDIA GeForce RTX 2080 Ti" },
  { id: "rtx2080super", name: "GeForce RTX 2080 Super",  vendor: "NVIDIA", vramGb: 8,  tdpW: 250, score: 25,  year: 2019, priceNew: null, priceUsed: 220,  buyQuery: "NVIDIA GeForce RTX 2080 Super" },
  { id: "rtx2080",      name: "GeForce RTX 2080",        vendor: "NVIDIA", vramGb: 8,  tdpW: 215, score: 24,  year: 2018, priceNew: null, priceUsed: 190,  buyQuery: "NVIDIA GeForce RTX 2080" },
  { id: "rtx2070super", name: "GeForce RTX 2070 Super",  vendor: "NVIDIA", vramGb: 8,  tdpW: 215, score: 22,  year: 2019, priceNew: null, priceUsed: 170,  buyQuery: "NVIDIA GeForce RTX 2070 Super" },
  { id: "rtx2070",      name: "GeForce RTX 2070",        vendor: "NVIDIA", vramGb: 8,  tdpW: 175, score: 19,  year: 2018, priceNew: null, priceUsed: 150,  buyQuery: "NVIDIA GeForce RTX 2070" },
  { id: "rtx2060super", name: "GeForce RTX 2060 Super",  vendor: "NVIDIA", vramGb: 8,  tdpW: 175, score: 19,  year: 2019, priceNew: null, priceUsed: 130,  buyQuery: "NVIDIA GeForce RTX 2060 Super" },
  { id: "rtx2060",      name: "GeForce RTX 2060",        vendor: "NVIDIA", vramGb: 6,  tdpW: 160, score: 17,  year: 2019, priceNew: null, priceUsed: 110,  buyQuery: "NVIDIA GeForce RTX 2060" },
  { id: "gtx1660super", name: "GeForce GTX 1660 Super",  vendor: "NVIDIA", vramGb: 6,  tdpW: 125, score: 14,  year: 2019, priceNew: null, priceUsed: 120,  buyQuery: "NVIDIA GeForce GTX 1660 Super" },
  { id: "gtx1660ti",    name: "GeForce GTX 1660 Ti",     vendor: "NVIDIA", vramGb: 6,  tdpW: 120, score: 14,  year: 2019, priceNew: null, priceUsed: 110,  buyQuery: "NVIDIA GeForce GTX 1660 Ti" },
  { id: "gtx1660",      name: "GeForce GTX 1660",        vendor: "NVIDIA", vramGb: 6,  tdpW: 120, score: 13,  year: 2019, priceNew: null, priceUsed: 100,  buyQuery: "NVIDIA GeForce GTX 1660" },
  { id: "gtx1650",      name: "GeForce GTX 1650",        vendor: "NVIDIA", vramGb: 4,  tdpW: 75,  score: 8,   year: 2019, priceNew: null, priceUsed: 80,   buyQuery: "NVIDIA GeForce GTX 1650" },
  { id: "gtx1080ti",    name: "GeForce GTX 1080 Ti",     vendor: "NVIDIA", vramGb: 11, tdpW: 250, score: 22,  year: 2017, priceNew: null, priceUsed: 180,  buyQuery: "NVIDIA GeForce GTX 1080 Ti" },
  { id: "gtx1080",      name: "GeForce GTX 1080",        vendor: "NVIDIA", vramGb: 8,  tdpW: 180, score: 17,  year: 2016, priceNew: null, priceUsed: 140,  buyQuery: "NVIDIA GeForce GTX 1080" },
  { id: "gtx1070ti",    name: "GeForce GTX 1070 Ti",     vendor: "NVIDIA", vramGb: 8,  tdpW: 180, score: 16,  year: 2017, priceNew: null, priceUsed: 120,  buyQuery: "NVIDIA GeForce GTX 1070 Ti" },
  { id: "gtx1070",      name: "GeForce GTX 1070",        vendor: "NVIDIA", vramGb: 8,  tdpW: 150, score: 14,  year: 2016, priceNew: null, priceUsed: 100,  buyQuery: "NVIDIA GeForce GTX 1070" },
  { id: "gtx1060",      name: "GeForce GTX 1060 6GB",    vendor: "NVIDIA", vramGb: 6,  tdpW: 120, score: 11,  year: 2016, priceNew: null, priceUsed: 70,   buyQuery: "NVIDIA GeForce GTX 1060 6GB" },

  { id: "rx9070xt",     name: "Radeon RX 9070 XT",       vendor: "AMD",    vramGb: 16, tdpW: 304, score: 55,  year: 2025, priceNew: 700,  priceUsed: 560,  buyQuery: "AMD Radeon RX 9070 XT" },
  { id: "rx9070",       name: "Radeon RX 9070",          vendor: "AMD",    vramGb: 16, tdpW: 220, score: 49,  year: 2025, priceNew: 580,  priceUsed: 460,  buyQuery: "AMD Radeon RX 9070" },
  { id: "rx7900xtx",    name: "Radeon RX 7900 XTX",      vendor: "AMD",    vramGb: 24, tdpW: 355, score: 58,  year: 2022, priceNew: 950,  priceUsed: 700,  buyQuery: "AMD Radeon RX 7900 XTX" },
  { id: "rx7900xt",     name: "Radeon RX 7900 XT",       vendor: "AMD",    vramGb: 20, tdpW: 315, score: 50,  year: 2022, priceNew: 750,  priceUsed: 580,  buyQuery: "AMD Radeon RX 7900 XT" },
  { id: "rx7800xt",     name: "Radeon RX 7800 XT",       vendor: "AMD",    vramGb: 16, tdpW: 263, score: 38,  year: 2023, priceNew: 500,  priceUsed: 380,  buyQuery: "AMD Radeon RX 7800 XT" },
  { id: "rx7700xt",     name: "Radeon RX 7700 XT",       vendor: "AMD",    vramGb: 12, tdpW: 245, score: 31,  year: 2023, priceNew: 420,  priceUsed: 320,  buyQuery: "AMD Radeon RX 7700 XT" },
  { id: "rx7600",       name: "Radeon RX 7600",          vendor: "AMD",    vramGb: 8,  tdpW: 165, score: 20,  year: 2023, priceNew: 270,  priceUsed: 200,  buyQuery: "AMD Radeon RX 7600" },
  { id: "rx6950xt",     name: "Radeon RX 6950 XT",       vendor: "AMD",    vramGb: 16, tdpW: 335, score: 41,  year: 2022, priceNew: null, priceUsed: 420,  buyQuery: "AMD Radeon RX 6950 XT" },
  { id: "rx6900xt",     name: "Radeon RX 6900 XT",       vendor: "AMD",    vramGb: 16, tdpW: 300, score: 39,  year: 2020, priceNew: null, priceUsed: 380,  buyQuery: "AMD Radeon RX 6900 XT" },
  { id: "rx6800xt",     name: "Radeon RX 6800 XT",       vendor: "AMD",    vramGb: 16, tdpW: 300, score: 36,  year: 2020, priceNew: null, priceUsed: 350,  buyQuery: "AMD Radeon RX 6800 XT" },
  { id: "rx6800",       name: "Radeon RX 6800",          vendor: "AMD",    vramGb: 16, tdpW: 250, score: 32,  year: 2020, priceNew: null, priceUsed: 300,  buyQuery: "AMD Radeon RX 6800" },
  { id: "rx6700xt",     name: "Radeon RX 6700 XT",       vendor: "AMD",    vramGb: 12, tdpW: 230, score: 26,  year: 2021, priceNew: null, priceUsed: 260,  buyQuery: "AMD Radeon RX 6700 XT" },
  { id: "rx6600xt",     name: "Radeon RX 6600 XT",       vendor: "AMD",    vramGb: 8,  tdpW: 160, score: 19,  year: 2021, priceNew: null, priceUsed: 190,  buyQuery: "AMD Radeon RX 6600 XT" },
  { id: "rx6600",       name: "Radeon RX 6600",          vendor: "AMD",    vramGb: 8,  tdpW: 132, score: 18,  year: 2021, priceNew: null, priceUsed: 170,  buyQuery: "AMD Radeon RX 6600" },
  { id: "rx6500xt",     name: "Radeon RX 6500 XT",       vendor: "AMD",    vramGb: 4,  tdpW: 107, score: 11,  year: 2022, priceNew: 160,  priceUsed: 110,  buyQuery: "AMD Radeon RX 6500 XT" },
  { id: "rx6400",       name: "Radeon RX 6400",          vendor: "AMD",    vramGb: 4,  tdpW: 53,  score: 8,   year: 2022, priceNew: 140,  priceUsed: 90,   buyQuery: "AMD Radeon RX 6400" },
  { id: "rx5700xt",     name: "Radeon RX 5700 XT",       vendor: "AMD",    vramGb: 8,  tdpW: 225, score: 18,  year: 2019, priceNew: null, priceUsed: 160,  buyQuery: "AMD Radeon RX 5700 XT" },
  { id: "rx5600xt",     name: "Radeon RX 5600 XT",       vendor: "AMD",    vramGb: 6,  tdpW: 150, score: 17,  year: 2020, priceNew: null, priceUsed: 130,  buyQuery: "AMD Radeon RX 5600 XT" },
  { id: "rx580",        name: "Radeon RX 580 8GB",       vendor: "AMD",    vramGb: 8,  tdpW: 185, score: 11,  year: 2017, priceNew: null, priceUsed: 70,   buyQuery: "AMD Radeon RX 580 8GB" },
  { id: "rx570",        name: "Radeon RX 570 4GB",       vendor: "AMD",    vramGb: 4,  tdpW: 150, score: 9,   year: 2017, priceNew: null, priceUsed: 55,   buyQuery: "AMD Radeon RX 570 4GB" },

  { id: "arcb580",      name: "Intel Arc B580",          vendor: "Intel",  vramGb: 12, tdpW: 190, score: 23,  year: 2024, priceNew: 250,  priceUsed: 190,  buyQuery: "Intel Arc B580" },
  { id: "arcb570",      name: "Intel Arc B570",          vendor: "Intel",  vramGb: 10, tdpW: 150, score: 20,  year: 2024, priceNew: 220,  priceUsed: 170,  buyQuery: "Intel Arc B570" },
  { id: "arca770",      name: "Intel Arc A770",          vendor: "Intel",  vramGb: 16, tdpW: 225, score: 20,  year: 2022, priceNew: null, priceUsed: 210,  buyQuery: "Intel Arc A770" },
  { id: "arca750",      name: "Intel Arc A750",          vendor: "Intel",  vramGb: 8,  tdpW: 225, score: 18,  year: 2022, priceNew: null, priceUsed: 180,  buyQuery: "Intel Arc A750" },
];

// ── CPUs ──────────────────────────────────────────────────────────────────────
// Scores: PassMark "Top Gaming CPUs" chart, normalized to the top entry = 100.
// Chips marked "[ST proxy]" don't appear on that gaming-specific chart (too
// old) — scored from PassMark's Single-Thread Rating instead, the documented
// fallback for chips outside the gaming chart's modern-enthusiast coverage.
export const CPUS: Cpu[] = [
  // AMD AM5 — current
  { id: "r9-9850x3d", name: "Ryzen 9 9850X3D",  vendor: "AMD",   socket: "AM5", cores: 16, threads: 32, tdpW: 170, score: 100, year: 2026, priceNew: 550, priceUsed: 480, buyQuery: "AMD Ryzen 9 9850X3D" },
  { id: "r9-9950x3d", name: "Ryzen 9 9950X3D",  vendor: "AMD",   socket: "AM5", cores: 16, threads: 32, tdpW: 170, score: 90,  year: 2025, priceNew: 650, priceUsed: 550, buyQuery: "AMD Ryzen 9 9950X3D" },
  { id: "r7-9800x3d", name: "Ryzen 7 9800X3D",  vendor: "AMD",   socket: "AM5", cores: 8,  threads: 16, tdpW: 120, score: 97,  year: 2024, priceNew: 440, priceUsed: 380, buyQuery: "AMD Ryzen 7 9800X3D" },
  { id: "r9-9900x",   name: "Ryzen 9 9900X",    vendor: "AMD",   socket: "AM5", cores: 12, threads: 24, tdpW: 120, score: 65,  year: 2024, priceNew: 430, priceUsed: 370, buyQuery: "AMD Ryzen 9 9900X" },
  { id: "r7-9700x",   name: "Ryzen 7 9700X",    vendor: "AMD",   socket: "AM5", cores: 8,  threads: 16, tdpW: 65,  score: 50,  year: 2024, priceNew: 350, priceUsed: 300, buyQuery: "AMD Ryzen 7 9700X" },
  { id: "r5-9600x",   name: "Ryzen 5 9600X",    vendor: "AMD",   socket: "AM5", cores: 6,  threads: 12, tdpW: 65,  score: 56,  year: 2024, priceNew: 270, priceUsed: 230, buyQuery: "AMD Ryzen 5 9600X" },
  { id: "r7-7800x3d", name: "Ryzen 7 7800X3D",  vendor: "AMD",   socket: "AM5", cores: 8,  threads: 16, tdpW: 120, score: 79,  year: 2023, priceNew: null, priceUsed: 360, buyQuery: "AMD Ryzen 7 7800X3D" },
  { id: "r9-7950x3d", name: "Ryzen 9 7950X3D",  vendor: "AMD",   socket: "AM5", cores: 16, threads: 32, tdpW: 120, score: 75,  year: 2023, priceNew: 500, priceUsed: 430, buyQuery: "AMD Ryzen 9 7950X3D" },
  { id: "r9-7900x",   name: "Ryzen 9 7900X",    vendor: "AMD",   socket: "AM5", cores: 12, threads: 24, tdpW: 170, score: 56,  year: 2022, priceNew: null, priceUsed: 320, buyQuery: "AMD Ryzen 9 7900X" },
  { id: "r7-7700x",   name: "Ryzen 7 7700X",    vendor: "AMD",   socket: "AM5", cores: 8,  threads: 16, tdpW: 105, score: 43,  year: 2022, priceNew: 280, priceUsed: 240, buyQuery: "AMD Ryzen 7 7700X" },
  { id: "r5-7600",    name: "Ryzen 5 7600",     vendor: "AMD",   socket: "AM5", cores: 6,  threads: 12, tdpW: 65,  score: 46,  year: 2023, priceNew: 200, priceUsed: 170, buyQuery: "AMD Ryzen 5 7600" },
  { id: "r5-7500f",   name: "Ryzen 5 7500F",    vendor: "AMD",   socket: "AM5", cores: 6,  threads: 12, tdpW: 65,  score: 48,  year: 2023, priceNew: 150, priceUsed: 130, buyQuery: "AMD Ryzen 5 7500F" },

  // AMD AM4 — legacy / huge used market
  { id: "r7-5800x3d", name: "Ryzen 7 5800X3D",  vendor: "AMD",   socket: "AM4", cores: 8,  threads: 16, tdpW: 105, score: 64,  year: 2022, priceNew: null, priceUsed: 230, buyQuery: "AMD Ryzen 7 5800X3D" },
  { id: "r9-5950x",   name: "Ryzen 9 5950X",    vendor: "AMD",   socket: "AM4", cores: 16, threads: 32, tdpW: 105, score: 32,  year: 2020, priceNew: null, priceUsed: 280, buyQuery: "AMD Ryzen 9 5950X" },
  { id: "r9-5900x",   name: "Ryzen 9 5900X",    vendor: "AMD",   socket: "AM4", cores: 12, threads: 24, tdpW: 105, score: 38,  year: 2020, priceNew: null, priceUsed: 180, buyQuery: "AMD Ryzen 9 5900X" },
  { id: "r7-5800x",   name: "Ryzen 7 5800X",    vendor: "AMD",   socket: "AM4", cores: 8,  threads: 16, tdpW: 105, score: 27,  year: 2020, priceNew: null, priceUsed: 140, buyQuery: "AMD Ryzen 7 5800X" }, // [ST proxy, interpolated]
  { id: "r7-5700x",   name: "Ryzen 7 5700X",    vendor: "AMD",   socket: "AM4", cores: 8,  threads: 16, tdpW: 65,  score: 31,  year: 2022, priceNew: null, priceUsed: 130, buyQuery: "AMD Ryzen 7 5700X" },
  { id: "r5-5600x",   name: "Ryzen 5 5600X",    vendor: "AMD",   socket: "AM4", cores: 6,  threads: 12, tdpW: 65,  score: 34,  year: 2020, priceNew: null, priceUsed: 110, buyQuery: "AMD Ryzen 5 5600X" },
  { id: "r5-5500",    name: "Ryzen 5 5500",     vendor: "AMD",   socket: "AM4", cores: 6,  threads: 12, tdpW: 65,  score: 22,  year: 2022, priceNew: 90,   priceUsed: 70,  buyQuery: "AMD Ryzen 5 5500" },
  { id: "r9-3900x",   name: "Ryzen 9 3900X",    vendor: "AMD",   socket: "AM4", cores: 12, threads: 24, tdpW: 105, score: 32,  year: 2019, priceNew: null, priceUsed: 130, buyQuery: "AMD Ryzen 9 3900X" },
  { id: "r7-3700x",   name: "Ryzen 7 3700X",    vendor: "AMD",   socket: "AM4", cores: 8,  threads: 16, tdpW: 65,  score: 24,  year: 2019, priceNew: null, priceUsed: 100, buyQuery: "AMD Ryzen 7 3700X" },
  { id: "r5-3600",    name: "Ryzen 5 3600",     vendor: "AMD",   socket: "AM4", cores: 6,  threads: 12, tdpW: 65,  score: 20,  year: 2019, priceNew: null, priceUsed: 80,  buyQuery: "AMD Ryzen 5 3600" }, // [ST proxy, interpolated]

  // Intel LGA1851 — current (Core Ultra 200S / "Plus" refresh)
  { id: "cu9-285k",   name: "Core Ultra 9 285K", vendor: "Intel", socket: "LGA1851", cores: 24, threads: 24, tdpW: 125, score: 83, year: 2024, priceNew: 480, priceUsed: 420, buyQuery: "Intel Core Ultra 9 285K" },
  { id: "cu7-270kplus",name:"Core Ultra 7 270K Plus", vendor: "Intel", socket: "LGA1851", cores: 20, threads: 20, tdpW: 125, score: 87, year: 2025, priceNew: 370, priceUsed: 320, buyQuery: "Intel Core Ultra 7 270K" },
  { id: "cu7-265k",   name: "Core Ultra 7 265K", vendor: "Intel", socket: "LGA1851", cores: 20, threads: 20, tdpW: 125, score: 75, year: 2024, priceNew: 350, priceUsed: 300, buyQuery: "Intel Core Ultra 7 265K" },
  { id: "cu5-250kplus",name:"Core Ultra 5 250K Plus", vendor: "Intel", socket: "LGA1851", cores: 14, threads: 14, tdpW: 125, score: 87, year: 2025, priceNew: 270, priceUsed: 230, buyQuery: "Intel Core Ultra 5 250K" },
  { id: "cu5-245k",   name: "Core Ultra 5 245K", vendor: "Intel", socket: "LGA1851", cores: 14, threads: 14, tdpW: 125, score: 73, year: 2024, priceNew: 260, priceUsed: 220, buyQuery: "Intel Core Ultra 5 245K" },

  // Intel LGA1700 — 12th-14th gen, huge installed base
  { id: "i9-14900k",  name: "Core i9-14900K",   vendor: "Intel", socket: "LGA1700", cores: 24, threads: 32, tdpW: 125, score: 63, year: 2023, priceNew: null, priceUsed: 380, buyQuery: "Intel Core i9-14900K" },
  { id: "i9-13900k",  name: "Core i9-13900K",   vendor: "Intel", socket: "LGA1700", cores: 24, threads: 32, tdpW: 125, score: 60, year: 2022, priceNew: null, priceUsed: 360, buyQuery: "Intel Core i9-13900K" },
  { id: "i7-14700k",  name: "Core i7-14700K",   vendor: "Intel", socket: "LGA1700", cores: 20, threads: 28, tdpW: 125, score: 58, year: 2023, priceNew: null, priceUsed: 290, buyQuery: "Intel Core i7-14700K" },
  { id: "i7-13700k",  name: "Core i7-13700K",   vendor: "Intel", socket: "LGA1700", cores: 16, threads: 24, tdpW: 125, score: 51, year: 2022, priceNew: null, priceUsed: 270, buyQuery: "Intel Core i7-13700K" },
  { id: "i9-12900k",  name: "Core i9-12900K",   vendor: "Intel", socket: "LGA1700", cores: 16, threads: 24, tdpW: 125, score: 43, year: 2021, priceNew: null, priceUsed: 280, buyQuery: "Intel Core i9-12900K" },
  { id: "i5-14600k",  name: "Core i5-14600K",   vendor: "Intel", socket: "LGA1700", cores: 14, threads: 20, tdpW: 125, score: 55, year: 2023, priceNew: null, priceUsed: 210, buyQuery: "Intel Core i5-14600K" },
  { id: "i7-12700k",  name: "Core i7-12700K",   vendor: "Intel", socket: "LGA1700", cores: 12, threads: 20, tdpW: 125, score: 36, year: 2021, priceNew: null, priceUsed: 230, buyQuery: "Intel Core i7-12700K" },
  { id: "i5-13600k",  name: "Core i5-13600K",   vendor: "Intel", socket: "LGA1700", cores: 14, threads: 20, tdpW: 125, score: 49, year: 2022, priceNew: null, priceUsed: 190, buyQuery: "Intel Core i5-13600K" },
  { id: "i5-13500",   name: "Core i5-13500",    vendor: "Intel", socket: "LGA1700", cores: 14, threads: 20, tdpW: 65,  score: 38, year: 2023, priceNew: null, priceUsed: 190, buyQuery: "Intel Core i5-13500" },
  { id: "i5-12600k",  name: "Core i5-12600K",   vendor: "Intel", socket: "LGA1700", cores: 10, threads: 16, tdpW: 125, score: 35, year: 2021, priceNew: null, priceUsed: 140, buyQuery: "Intel Core i5-12600K" },
  { id: "i5-12400",   name: "Core i5-12400",    vendor: "Intel", socket: "LGA1700", cores: 6,  threads: 12, tdpW: 65,  score: 27, year: 2022, priceNew: null, priceUsed: 130, buyQuery: "Intel Core i5-12400" },
  { id: "i3-12100",   name: "Core i3-12100",    vendor: "Intel", socket: "LGA1700", cores: 4,  threads: 8,  tdpW: 60,  score: 14, year: 2021, priceNew: null, priceUsed: 70,  buyQuery: "Intel Core i3-12100" },

  // Intel LGA1200 — 10th/11th gen, huge installed base (previously had ZERO
  // entries despite the chipset-socket map already recognizing H410/B460/
  // H470/Z490/H510/B560/H570/Z590 boards — any real 10th/11th-gen system
  // detected a socket but had no compatible CPUs/motherboards to offer).
  { id: "i9-11900k",  name: "Core i9-11900K",   vendor: "Intel", socket: "LGA1200", cores: 8,  threads: 16, tdpW: 125, score: 27, year: 2021, priceNew: null, priceUsed: 180, buyQuery: "Intel Core i9-11900K" }, // [ST proxy]
  { id: "i9-10900k",  name: "Core i9-10900K",   vendor: "Intel", socket: "LGA1200", cores: 10, threads: 20, tdpW: 125, score: 26, year: 2020, priceNew: null, priceUsed: 150, buyQuery: "Intel Core i9-10900K" }, // [ST proxy]
  { id: "i7-11700k",  name: "Core i7-11700K",   vendor: "Intel", socket: "LGA1200", cores: 8,  threads: 16, tdpW: 125, score: 26, year: 2021, priceNew: null, priceUsed: 150, buyQuery: "Intel Core i7-11700K" }, // [ST proxy]
  { id: "i7-10700k",  name: "Core i7-10700K",   vendor: "Intel", socket: "LGA1200", cores: 8,  threads: 16, tdpW: 125, score: 24, year: 2020, priceNew: null, priceUsed: 120, buyQuery: "Intel Core i7-10700K" }, // [ST proxy]
  { id: "i5-11600k",  name: "Core i5-11600K",   vendor: "Intel", socket: "LGA1200", cores: 6,  threads: 12, tdpW: 125, score: 20, year: 2021, priceNew: null, priceUsed: 110, buyQuery: "Intel Core i5-11600K" },
  { id: "i5-10400",   name: "Core i5-10400",    vendor: "Intel", socket: "LGA1200", cores: 6,  threads: 12, tdpW: 65,  score: 21, year: 2020, priceNew: null, priceUsed: 80,  buyQuery: "Intel Core i5-10400" }, // [ST proxy]

  // Intel LGA1151 — 8th/9th gen, still common budget/used (chipset map: Z390/
  // B365/H310/Z370/B360/H370/Q370 — see socketFromBoardModel in pcAdvisor.ts)
  { id: "i7-9700k",   name: "Core i7-9700K",    vendor: "Intel", socket: "LGA1151", cores: 8,  threads: 8,  tdpW: 95,  score: 22, year: 2018, priceNew: null, priceUsed: 110, buyQuery: "Intel Core i7-9700K" }, // [ST proxy]
  { id: "i7-8700k",   name: "Core i7-8700K",    vendor: "Intel", socket: "LGA1151", cores: 6,  threads: 12, tdpW: 95,  score: 21, year: 2017, priceNew: null, priceUsed: 95,  buyQuery: "Intel Core i7-8700K" }, // [ST proxy]
  { id: "i5-9600k",   name: "Core i5-9600K",    vendor: "Intel", socket: "LGA1151", cores: 6,  threads: 6,  tdpW: 95,  score: 21, year: 2018, priceNew: null, priceUsed: 75,  buyQuery: "Intel Core i5-9600K" }, // [ST proxy]
  { id: "i5-8400",    name: "Core i5-8400",     vendor: "Intel", socket: "LGA1151", cores: 6,  threads: 6,  tdpW: 65,  score: 11, year: 2017, priceNew: null, priceUsed: 60,  buyQuery: "Intel Core i5-8400" },
  { id: "i3-9100",    name: "Core i3-9100",     vendor: "Intel", socket: "LGA1151", cores: 4,  threads: 4,  tdpW: 65,  score: 6,  year: 2019, priceNew: null, priceUsed: 45,  buyQuery: "Intel Core i3-9100" },
];

// ── RAM kits ──────────────────────────────────────────────────────────────────
// Scores: real AIDA64 bandwidth/latency deltas by speed grade + published
// gaming FPS uplift figures, normalized to the fastest well-rounded kit
// (DDR5-32GB-6000) = 100. Capacity is penalized below/above the empirically
// validated ~32GB gaming sweet spot, not treated as monotonically "more=better".
// Prices updated June 2026 for the ongoing AI-driven DRAM shortage: 32GB DDR5
// kits that sold for $80-120 in mid-2025 are now $300-500+ and climbing
// (SK hynix/Samsung have signaled supply constraints persisting well beyond
// 2026), so these are deliberately set far higher than a "normal" snapshot —
// still rough estimates, always confirm via the live Buy links.
export const RAM_KITS: RamKit[] = [
  { id: "ddr5-16-6000", name: "DDR5 16GB (2×8GB) 6000MT/s",  type: "DDR5", capacityGb: 16, speedMtps: 6000, score: 88, priceNew: 220, priceUsed: 160, buyQuery: "DDR5 16GB 6000MHz kit" },
  { id: "ddr5-32-6000", name: "DDR5 32GB (2×16GB) 6000MT/s", type: "DDR5", capacityGb: 32, speedMtps: 6000, score: 100, priceNew: 420, priceUsed: 310, buyQuery: "DDR5 32GB 6000MHz kit" },
  { id: "ddr5-32-5600", name: "DDR5 32GB (2×16GB) 5600MT/s", type: "DDR5", capacityGb: 32, speedMtps: 5600, score: 82, priceNew: 390, priceUsed: 290, buyQuery: "DDR5 32GB 5600MHz kit" },
  { id: "ddr5-64-6000", name: "DDR5 64GB (2×32GB) 6000MT/s", type: "DDR5", capacityGb: 64, speedMtps: 6000, score: 93, priceNew: 780, priceUsed: 600, buyQuery: "DDR5 64GB 6000MHz kit" },
  { id: "ddr5-48-6000", name: "DDR5 48GB (2×24GB) 6000MT/s", type: "DDR5", capacityGb: 48, speedMtps: 6000, score: 97, priceNew: 600, priceUsed: 460, buyQuery: "DDR5 48GB 6000MHz kit" },
  { id: "ddr4-8-3200",  name: "DDR4 8GB (2×4GB) 3200MT/s",   type: "DDR4", capacityGb: 8,  speedMtps: 3200, score: 22, priceNew: 35,  priceUsed: 22,  buyQuery: "DDR4 8GB 3200MHz kit" },
  { id: "ddr4-16-2666", name: "DDR4 16GB (2×8GB) 2666MT/s",  type: "DDR4", capacityGb: 16, speedMtps: 2666, score: 38, priceNew: 75,  priceUsed: 50,  buyQuery: "DDR4 16GB 2666MHz kit" },
  { id: "ddr4-16-3200", name: "DDR4 16GB (2×8GB) 3200MT/s",  type: "DDR4", capacityGb: 16, speedMtps: 3200, score: 48, priceNew: 85,  priceUsed: 55,  buyQuery: "DDR4 16GB 3200MHz kit" },
  { id: "ddr4-32-3600", name: "DDR4 32GB (2×16GB) 3600MT/s", type: "DDR4", capacityGb: 32, speedMtps: 3600, score: 55, priceNew: 150, priceUsed: 100, buyQuery: "DDR4 32GB 3600MHz kit" },
];

// ── Motherboards ──────────────────────────────────────────────────────────────
export const MOTHERBOARDS: Motherboard[] = [
  { id: "mobo-a620",  name: "A620 (entry AM5)",      vendor: "Various", socket: "AM5",     chipset: "A620",  ramType: "DDR5", formFactor: "mATX", priceNew: 90,  priceUsed: 65,  buyQuery: "A620 motherboard AM5" },
  { id: "mobo-b650",  name: "B650 (mainstream AM5)",  vendor: "Various", socket: "AM5",     chipset: "B650",  ramType: "DDR5", formFactor: "ATX",  priceNew: 150, priceUsed: 110, buyQuery: "B650 motherboard ATX" },
  { id: "mobo-x670e", name: "X670E (enthusiast AM5)", vendor: "Various", socket: "AM5",     chipset: "X670E", ramType: "DDR5", formFactor: "ATX",  priceNew: 280, priceUsed: 210, buyQuery: "X670E motherboard ATX" },
  { id: "mobo-a520",  name: "A520 (entry AM4)",       vendor: "Various", socket: "AM4",     chipset: "A520",  ramType: "DDR4", formFactor: "mATX", priceNew: 65,  priceUsed: 45,  buyQuery: "A520 motherboard AM4" },
  { id: "mobo-b550",  name: "B550 (mainstream AM4)",  vendor: "Various", socket: "AM4",     chipset: "B550",  ramType: "DDR4", formFactor: "ATX",  priceNew: 110, priceUsed: 80,  buyQuery: "B550 motherboard ATX" },
  { id: "mobo-h610",  name: "H610 (entry LGA1700)",   vendor: "Various", socket: "LGA1700", chipset: "H610",  ramType: "DDR4", formFactor: "mATX", priceNew: 80,  priceUsed: 55,  buyQuery: "H610 motherboard LGA1700" },
  { id: "mobo-b760",  name: "B760 (mainstream LGA1700)", vendor: "Various", socket: "LGA1700", chipset: "B760", ramType: "DDR5", formFactor: "ATX", priceNew: 150, priceUsed: 110, buyQuery: "B760 motherboard DDR5" },
  { id: "mobo-z790",  name: "Z790 (enthusiast LGA1700)", vendor: "Various", socket: "LGA1700", chipset: "Z790", ramType: "DDR5", formFactor: "ATX", priceNew: 280, priceUsed: 210, buyQuery: "Z790 motherboard DDR5" },
  { id: "mobo-b860",  name: "B860 (mainstream LGA1851)", vendor: "Various", socket: "LGA1851", chipset: "B860", ramType: "DDR5", formFactor: "ATX", priceNew: 170, priceUsed: 130, buyQuery: "B860 motherboard LGA1851" },
  { id: "mobo-z890",  name: "Z890 (enthusiast LGA1851)", vendor: "Various", socket: "LGA1851", chipset: "Z890", ramType: "DDR5", formFactor: "ATX", priceNew: 300, priceUsed: 230, buyQuery: "Z890 motherboard LGA1851" },
  { id: "mobo-h410",  name: "H410 (entry LGA1200)",   vendor: "Various", socket: "LGA1200", chipset: "H410",  ramType: "DDR4", formFactor: "mATX", priceNew: 65,  priceUsed: 45,  buyQuery: "H410 motherboard LGA1200" },
  { id: "mobo-b560",  name: "B560 (mainstream LGA1200)", vendor: "Various", socket: "LGA1200", chipset: "B560", ramType: "DDR4", formFactor: "ATX", priceNew: 110, priceUsed: 80,  buyQuery: "B560 motherboard LGA1200" },
  { id: "mobo-z590",  name: "Z590 (enthusiast LGA1200)", vendor: "Various", socket: "LGA1200", chipset: "Z590", ramType: "DDR4", formFactor: "ATX", priceNew: 220, priceUsed: 170, buyQuery: "Z590 motherboard LGA1200" },
  { id: "mobo-h310",  name: "H310 (entry LGA1151)",   vendor: "Various", socket: "LGA1151", chipset: "H310",  ramType: "DDR4", formFactor: "mATX", priceNew: 55,  priceUsed: 35,  buyQuery: "H310 motherboard LGA1151" },
  { id: "mobo-b360",  name: "B360 (mainstream LGA1151)", vendor: "Various", socket: "LGA1151", chipset: "B360", ramType: "DDR4", formFactor: "ATX",  priceNew: 90,  priceUsed: 65,  buyQuery: "B360 motherboard LGA1151" },
  { id: "mobo-z390",  name: "Z390 (enthusiast LGA1151)", vendor: "Various", socket: "LGA1151", chipset: "Z390", ramType: "DDR4", formFactor: "ATX",  priceNew: 160, priceUsed: 120, buyQuery: "Z390 motherboard LGA1151" },
];

// ── Storage ───────────────────────────────────────────────────────────────────
// Scores: Tom's Hardware SSD Benchmarks Hierarchy (Seq MB/s, Random IOPS,
// Overall Score) for NVMe/SATA tiers; StorageReview/vendor datasheets for
// 7200RPM HDD figures. Normalized to PCIe Gen5 NVMe = 100. HDD scores are
// deliberately near the floor — real HDDs are 10-50x+ slower in throughput/
// IOPS than NVMe, not ~20% slower as the old hand-estimated scores implied.
export const STORAGE: StorageDrive[] = [
  { id: "nvme-gen5-2tb", name: "NVMe Gen5 SSD 2TB", kind: "NVMe",     capacityGb: 2000, score: 100, priceNew: 170, priceUsed: 130, buyQuery: "NVMe Gen5 SSD 2TB" },
  { id: "nvme-gen4-2tb", name: "NVMe Gen4 SSD 2TB", kind: "NVMe",     capacityGb: 2000, score: 65,  priceNew: 120, priceUsed: 90,  buyQuery: "NVMe Gen4 SSD 2TB" },
  { id: "nvme-gen4-1tb", name: "NVMe Gen4 SSD 1TB", kind: "NVMe",     capacityGb: 1000, score: 60,  priceNew: 70,  priceUsed: 50,  buyQuery: "NVMe Gen4 SSD 1TB" },
  { id: "nvme-gen3-1tb", name: "NVMe Gen3 SSD 1TB", kind: "NVMe",     capacityGb: 1000, score: 30,  priceNew: 55,  priceUsed: 40,  buyQuery: "NVMe Gen3 SSD 1TB" },
  { id: "sata-ssd-1tb",  name: "SATA SSD 1TB",      kind: "SATA SSD", capacityGb: 1000, score: 14,  priceNew: 55,  priceUsed: 40,  buyQuery: "SATA SSD 1TB" },
  { id: "sata-ssd-500gb",name: "SATA SSD 500GB",    kind: "SATA SSD", capacityGb: 500,  score: 12,  priceNew: 35,  priceUsed: 25,  buyQuery: "SATA SSD 500GB" },
  { id: "hdd-4tb",       name: "HDD 4TB 7200RPM",   kind: "HDD",      capacityGb: 4000, score: 4,   priceNew: 80,  priceUsed: 50,  buyQuery: "HDD 4TB 7200RPM" },
  { id: "hdd-2tb",       name: "HDD 2TB 7200RPM",   kind: "HDD",      capacityGb: 2000, score: 3,   priceNew: 55,  priceUsed: 35,  buyQuery: "HDD 2TB 7200RPM" },
  { id: "hdd-1tb",       name: "HDD 1TB 7200RPM",   kind: "HDD",      capacityGb: 1000, score: 2,   priceNew: 40,  priceUsed: 25,  buyQuery: "HDD 1TB 7200RPM" },
];

// ── Power supplies ────────────────────────────────────────────────────────────
export const PSUS: Psu[] = [
  { id: "psu-450",  name: "450W 80+ Bronze",   watts: 450,  priceNew: 45,  priceUsed: 30,  buyQuery: "450W 80 Plus Bronze PSU" },
  { id: "psu-550",  name: "550W 80+ Bronze",   watts: 550,  priceNew: 55,  priceUsed: 40,  buyQuery: "550W 80 Plus Bronze PSU" },
  { id: "psu-650",  name: "650W 80+ Gold",     watts: 650,  priceNew: 80,  priceUsed: 60,  buyQuery: "650W 80 Plus Gold PSU" },
  { id: "psu-750",  name: "750W 80+ Gold",     watts: 750,  priceNew: 95,  priceUsed: 70,  buyQuery: "750W 80 Plus Gold PSU" },
  { id: "psu-850",  name: "850W 80+ Gold",     watts: 850,  priceNew: 115, priceUsed: 85,  buyQuery: "850W 80 Plus Gold PSU" },
  { id: "psu-1000", name: "1000W 80+ Platinum",watts: 1000, priceNew: 160, priceUsed: 120, buyQuery: "1000W 80 Plus Platinum PSU" },
  { id: "psu-1200", name: "1200W 80+ Platinum",watts: 1200, priceNew: 220, priceUsed: 170, buyQuery: "1200W 80 Plus Platinum PSU" },
];

// ── Retailer link builder ─────────────────────────────────────────────────────
export type BuyLink = { label: string; url: string };

/** UI language code → retailer store list. Keyed loosely on the app's `Lang`
 * type (kept as plain strings here to avoid a circular import with i18n.tsx).
 * Each entry is a store that actually serves/ships to that market — the old
 * version hardcoded amazon.com + ebay.com + a German price-comparison site
 * for every locale, which silently breaks for anyone outside the US/DE (e.g.
 * amazon.com often blocks or mis-prices non-US checkout). Only domains/search
 * URL patterns that are well-documented and stable are included; locales
 * without a confirmed-good regional retailer fall back to Amazon's local
 * domain (when it exists) + Newegg, which ships internationally to most of
 * Europe/Turkey. */
type StoreDef = { label: string; build: (q: string) => string };

const STORES_BY_LANG: Record<string, StoreDef[]> = {
  en: [
    { label: "Amazon",      build: (q) => `https://www.amazon.com/s?k=${q}` },
    { label: "Newegg",      build: (q) => `https://www.newegg.com/p/pl?d=${q}` },
    { label: "PCPartPicker", build: (q) => `https://pcpartpicker.com/search/?q=${q}` },
    { label: "eBay",        build: (q) => `https://www.ebay.com/sch/i.html?_nkw=${q}` },
  ],
  de: [ // Germany/Austria/Switzerland
    { label: "Amazon.de",   build: (q) => `https://www.amazon.de/s?k=${q}` },
    { label: "Geizhals",    build: (q) => `https://geizhals.de/?fs=${q}` },
    { label: "eBay.de",     build: (q) => `https://www.ebay.de/sch/i.html?_nkw=${q}` },
    { label: "Newegg",      build: (q) => `https://www.newegg.com/p/pl?d=${q}` },
  ],
  fr: [
    { label: "Amazon.fr",   build: (q) => `https://www.amazon.fr/s?k=${q}` },
    { label: "eBay.fr",     build: (q) => `https://www.ebay.fr/sch/i.html?_nkw=${q}` },
    { label: "Newegg",      build: (q) => `https://www.newegg.com/p/pl?d=${q}` },
    { label: "PCPartPicker", build: (q) => `https://pcpartpicker.com/search/?q=${q}` },
  ],
  es: [
    { label: "Amazon.es",   build: (q) => `https://www.amazon.es/s?k=${q}` },
    { label: "eBay.es",     build: (q) => `https://www.ebay.es/sch/i.html?_nkw=${q}` },
    { label: "Newegg",      build: (q) => `https://www.newegg.com/p/pl?d=${q}` },
    { label: "PCPartPicker", build: (q) => `https://pcpartpicker.com/search/?q=${q}` },
  ],
  pt: [ // Amazon/eBay have no dedicated .pt marketplace — amazon.es ships to/serves Portugal
    { label: "Amazon.es",   build: (q) => `https://www.amazon.es/s?k=${q}` },
    { label: "Newegg",      build: (q) => `https://www.newegg.com/p/pl?d=${q}` },
    { label: "PCPartPicker", build: (q) => `https://pcpartpicker.com/search/?q=${q}` },
  ],
  pl: [
    { label: "Amazon.pl",   build: (q) => `https://www.amazon.pl/s?k=${q}` },
    { label: "x-kom",       build: (q) => `https://www.x-kom.pl/szukaj?q=${q}` },
    { label: "Allegro",     build: (q) => `https://allegro.pl/listing?string=${q}` },
    { label: "Newegg",      build: (q) => `https://www.newegg.com/p/pl?d=${q}` },
  ],
  ru: [ // Amazon/eBay/Newegg do not reliably serve Russia — domestic retailers only
    { label: "DNS",         build: (q) => `https://www.dns-shop.ru/search/?q=${q}` },
    { label: "Citilink",    build: (q) => `https://www.citilink.ru/search/?text=${q}` },
    { label: "Yandex Market", build: (q) => `https://market.yandex.ru/search?text=${q}` },
  ],
  tr: [
    { label: "Amazon.com.tr", build: (q) => `https://www.amazon.com.tr/s?k=${q}` },
    { label: "Hepsiburada", build: (q) => `https://www.hepsiburada.com/ara?q=${q}` },
    { label: "Newegg",      build: (q) => `https://www.newegg.com/p/pl?d=${q}` },
    { label: "PCPartPicker", build: (q) => `https://pcpartpicker.com/search/?q=${q}` },
  ],
  sv: [ // Sweden — no dedicated eBay.se marketplace
    { label: "Amazon.se",   build: (q) => `https://www.amazon.se/s?k=${q}` },
    { label: "Newegg",      build: (q) => `https://www.newegg.com/p/pl?d=${q}` },
    { label: "PCPartPicker", build: (q) => `https://pcpartpicker.com/search/?q=${q}` },
  ],
};

/** Builds direct search-result links instead of static product URLs — exact
 * product page IDs go stale; a search query never 404s and always reflects
 * the current live price. Stores are chosen per UI language so links actually
 * work for the user's region instead of always pointing at US/DE-only sites. */
export function buyLinks(query: string, lang: string = "en"): BuyLink[] {
  const q = encodeURIComponent(query);
  const stores = STORES_BY_LANG[lang] ?? STORES_BY_LANG.en;
  return stores.map((s) => ({ label: s.label, url: s.build(q) }));
}
