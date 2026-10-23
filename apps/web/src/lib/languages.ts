// ISO 639-2 (bibliographic + terminologic) and ISO 639-1 codes → English names.
const NAMES: Record<string, string> = {
  afr: "Afrikaans", alb: "Albanian", sqi: "Albanian", amh: "Amharic", ara: "Arabic", arm: "Armenian", hye: "Armenian",
  aze: "Azerbaijani", baq: "Basque", eus: "Basque", bel: "Belarusian", ben: "Bengali", bos: "Bosnian", bre: "Breton",
  bul: "Bulgarian", bur: "Burmese", mya: "Burmese", cat: "Catalan", chi: "Chinese", zho: "Chinese", cze: "Czech",
  ces: "Czech", dan: "Danish", dut: "Dutch", nld: "Dutch", eng: "English", est: "Estonian", fao: "Faroese",
  fil: "Filipino", fin: "Finnish", fre: "French", fra: "French", geo: "Georgian", kat: "Georgian", ger: "German",
  deu: "German", gle: "Irish", glg: "Galician", gre: "Greek", ell: "Greek", guj: "Gujarati", heb: "Hebrew",
  hin: "Hindi", hrv: "Croatian", hun: "Hungarian", ice: "Icelandic", isl: "Icelandic", ind: "Indonesian",
  ita: "Italian", jpn: "Japanese", kan: "Kannada", kaz: "Kazakh", khm: "Khmer", kor: "Korean", kur: "Kurdish",
  lao: "Lao", lat: "Latin", lav: "Latvian", lit: "Lithuanian", ltz: "Luxembourgish", mac: "Macedonian",
  mkd: "Macedonian", mal: "Malayalam", mao: "Maori", mri: "Maori", mar: "Marathi", may: "Malay", msa: "Malay",
  mlt: "Maltese", mon: "Mongolian", nep: "Nepali", nob: "Norwegian Bokmål", nno: "Norwegian Nynorsk",
  nor: "Norwegian", per: "Persian", fas: "Persian", pol: "Polish", por: "Portuguese", pan: "Punjabi",
  rum: "Romanian", ron: "Romanian", rus: "Russian", slo: "Slovak", slk: "Slovak", slv: "Slovenian",
  som: "Somali", spa: "Spanish", srp: "Serbian", swa: "Swahili", swe: "Swedish", tam: "Tamil", tel: "Telugu",
  tgl: "Tagalog", tha: "Thai", tib: "Tibetan", bod: "Tibetan", tur: "Turkish", ukr: "Ukrainian", urd: "Urdu",
  uzb: "Uzbek", vie: "Vietnamese", wel: "Welsh", cym: "Welsh", yid: "Yiddish", zul: "Zulu", yue: "Cantonese",
  cmn: "Mandarin", nan: "Min Nan", hat: "Haitian Creole", haw: "Hawaiian", ibo: "Igbo", yor: "Yoruba",
  hau: "Hausa", sin: "Sinhala", pus: "Pashto", tat: "Tatar", kir: "Kyrgyz", tuk: "Turkmen", tgk: "Tajik",
  und: "Unknown", mul: "Multiple", zxx: "No linguistic content", qaa: "Original",
  // ISO 639-1
  af: "Afrikaans", sq: "Albanian", am: "Amharic", ar: "Arabic", hy: "Armenian", az: "Azerbaijani", eu: "Basque",
  be: "Belarusian", bn: "Bengali", bs: "Bosnian", bg: "Bulgarian", my: "Burmese", ca: "Catalan", zh: "Chinese",
  cs: "Czech", da: "Danish", nl: "Dutch", en: "English", et: "Estonian", fi: "Finnish", fr: "French", ka: "Georgian",
  de: "German", ga: "Irish", gl: "Galician", el: "Greek", gu: "Gujarati", he: "Hebrew", hi: "Hindi", hr: "Croatian",
  hu: "Hungarian", is: "Icelandic", id: "Indonesian", it: "Italian", ja: "Japanese", kn: "Kannada", kk: "Kazakh",
  km: "Khmer", ko: "Korean", ku: "Kurdish", lo: "Lao", la: "Latin", lv: "Latvian", lt: "Lithuanian",
  lb: "Luxembourgish", mk: "Macedonian", ml: "Malayalam", mi: "Maori", mr: "Marathi", ms: "Malay", mt: "Maltese",
  mn: "Mongolian", ne: "Nepali", nb: "Norwegian Bokmål", nn: "Norwegian Nynorsk", no: "Norwegian", fa: "Persian",
  pl: "Polish", pt: "Portuguese", pa: "Punjabi", ro: "Romanian", ru: "Russian", sk: "Slovak", sl: "Slovenian",
  so: "Somali", es: "Spanish", sr: "Serbian", sw: "Swahili", sv: "Swedish", ta: "Tamil", te: "Telugu", tl: "Tagalog",
  th: "Thai", bo: "Tibetan", tr: "Turkish", uk: "Ukrainian", ur: "Urdu", uz: "Uzbek", vi: "Vietnamese", cy: "Welsh",
  yi: "Yiddish", zu: "Zulu",
};

/** "eng" → "English", "pt-BR" → "Portuguese (BR)", unknown codes are returned upper-cased. */
export function languageName(code: string | null | undefined): string | null {
  if (!code) return null;
  const raw = code.trim();
  if (!raw) return null;
  const [base = "", region] = raw.toLowerCase().split(/[-_]/);
  const name = NAMES[base];
  if (!name) return raw.length <= 3 ? raw.toUpperCase() : raw;
  return region ? `${name} (${region.toUpperCase()})` : name;
}

/**
 * Languages offered as audio and subtitle defaults, by name. Codes are ISO
 * 639-2/B, the form ffprobe reports; the server matches the other forms too.
 */
export const LANGUAGES: readonly { code: string; name: string }[] = [
  "ara", "bul", "cat", "chi", "cze", "dan", "dut", "eng", "fin", "fre", "ger", "gre", "heb", "hin", "hrv", "hun",
  "ind", "ita", "jpn", "kor", "nor", "per", "pol", "por", "rum", "rus", "slo", "spa", "srp", "swe", "tha", "tur",
  "ukr", "vie",
]
  .map((code) => ({ code, name: languageName(code) ?? code }))
  .sort((a, b) => a.name.localeCompare(b.name));
