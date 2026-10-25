package app.pley.android.model.format

/*
 * Language names for audio and subtitle tracks, as apps/web/src/lib/languages.ts:
 * ISO 639-2 (both forms) and 639-1 codes to English names.
 */

private val NAMES: Map<String, String> = mapOf(
    "afr" to "Afrikaans", "alb" to "Albanian", "sqi" to "Albanian", "amh" to "Amharic", "ara" to "Arabic",
    "arm" to "Armenian", "hye" to "Armenian", "aze" to "Azerbaijani", "baq" to "Basque", "eus" to "Basque",
    "bel" to "Belarusian", "ben" to "Bengali", "bos" to "Bosnian", "bre" to "Breton", "bul" to "Bulgarian",
    "bur" to "Burmese", "mya" to "Burmese", "cat" to "Catalan", "chi" to "Chinese", "zho" to "Chinese",
    "cze" to "Czech", "ces" to "Czech", "dan" to "Danish", "dut" to "Dutch", "nld" to "Dutch", "eng" to "English",
    "est" to "Estonian", "fao" to "Faroese", "fil" to "Filipino", "fin" to "Finnish", "fre" to "French",
    "fra" to "French", "geo" to "Georgian", "kat" to "Georgian", "ger" to "German", "deu" to "German",
    "gle" to "Irish", "glg" to "Galician", "gre" to "Greek", "ell" to "Greek", "guj" to "Gujarati",
    "heb" to "Hebrew", "hin" to "Hindi", "hrv" to "Croatian", "hun" to "Hungarian", "ice" to "Icelandic",
    "isl" to "Icelandic", "ind" to "Indonesian", "ita" to "Italian", "jpn" to "Japanese", "kan" to "Kannada",
    "kaz" to "Kazakh", "khm" to "Khmer", "kor" to "Korean", "kur" to "Kurdish", "lao" to "Lao", "lat" to "Latin",
    "lav" to "Latvian", "lit" to "Lithuanian", "ltz" to "Luxembourgish", "mac" to "Macedonian",
    "mkd" to "Macedonian", "mal" to "Malayalam", "mao" to "Maori", "mri" to "Maori", "mar" to "Marathi",
    "may" to "Malay", "msa" to "Malay", "mlt" to "Maltese", "mon" to "Mongolian", "nep" to "Nepali",
    "nob" to "Norwegian Bokmål", "nno" to "Norwegian Nynorsk", "nor" to "Norwegian", "per" to "Persian",
    "fas" to "Persian", "pol" to "Polish", "por" to "Portuguese", "pan" to "Punjabi", "rum" to "Romanian",
    "ron" to "Romanian", "rus" to "Russian", "slo" to "Slovak", "slk" to "Slovak", "slv" to "Slovenian",
    "som" to "Somali", "spa" to "Spanish", "srp" to "Serbian", "swa" to "Swahili", "swe" to "Swedish",
    "tam" to "Tamil", "tel" to "Telugu", "tgl" to "Tagalog", "tha" to "Thai", "tib" to "Tibetan",
    "bod" to "Tibetan", "tur" to "Turkish", "ukr" to "Ukrainian", "urd" to "Urdu", "uzb" to "Uzbek",
    "vie" to "Vietnamese", "wel" to "Welsh", "cym" to "Welsh", "yid" to "Yiddish", "zul" to "Zulu",
    "yue" to "Cantonese", "cmn" to "Mandarin", "nan" to "Min Nan", "hat" to "Haitian Creole",
    "haw" to "Hawaiian", "ibo" to "Igbo", "yor" to "Yoruba", "hau" to "Hausa", "sin" to "Sinhala",
    "pus" to "Pashto", "tat" to "Tatar", "kir" to "Kyrgyz", "tuk" to "Turkmen", "tgk" to "Tajik",
    "und" to "Unknown", "mul" to "Multiple", "zxx" to "No linguistic content", "qaa" to "Original",
    // ISO 639-1
    "af" to "Afrikaans", "sq" to "Albanian", "am" to "Amharic", "ar" to "Arabic", "hy" to "Armenian",
    "az" to "Azerbaijani", "eu" to "Basque", "be" to "Belarusian", "bn" to "Bengali", "bs" to "Bosnian",
    "bg" to "Bulgarian", "my" to "Burmese", "ca" to "Catalan", "zh" to "Chinese", "cs" to "Czech",
    "da" to "Danish", "nl" to "Dutch", "en" to "English", "et" to "Estonian", "fi" to "Finnish",
    "fr" to "French", "ka" to "Georgian", "de" to "German", "ga" to "Irish", "gl" to "Galician",
    "el" to "Greek", "gu" to "Gujarati", "he" to "Hebrew", "hi" to "Hindi", "hr" to "Croatian",
    "hu" to "Hungarian", "is" to "Icelandic", "id" to "Indonesian", "it" to "Italian", "ja" to "Japanese",
    "kn" to "Kannada", "kk" to "Kazakh", "km" to "Khmer", "ko" to "Korean", "ku" to "Kurdish", "lo" to "Lao",
    "la" to "Latin", "lv" to "Latvian", "lt" to "Lithuanian", "lb" to "Luxembourgish", "mk" to "Macedonian",
    "ml" to "Malayalam", "mi" to "Maori", "mr" to "Marathi", "ms" to "Malay", "mt" to "Maltese",
    "mn" to "Mongolian", "ne" to "Nepali", "nb" to "Norwegian Bokmål", "nn" to "Norwegian Nynorsk",
    "no" to "Norwegian", "fa" to "Persian", "pl" to "Polish", "pt" to "Portuguese", "pa" to "Punjabi",
    "ro" to "Romanian", "ru" to "Russian", "sk" to "Slovak", "sl" to "Slovenian", "so" to "Somali",
    "es" to "Spanish", "sr" to "Serbian", "sw" to "Swahili", "sv" to "Swedish", "ta" to "Tamil",
    "te" to "Telugu", "tl" to "Tagalog", "th" to "Thai", "bo" to "Tibetan", "tr" to "Turkish",
    "uk" to "Ukrainian", "ur" to "Urdu", "uz" to "Uzbek", "vi" to "Vietnamese", "cy" to "Welsh",
    "yi" to "Yiddish", "zu" to "Zulu",
)

/** "eng" → "English", "pt-BR" → "Portuguese (BR)"; an unknown code comes back upper-cased; `null` without one. */
fun languageName(code: String?): String? {
    val raw = code?.trim().orEmpty()
    if (raw.isEmpty()) return null
    val parts = raw.lowercase().split('-', '_')
    val name = NAMES[parts[0]] ?: return if (raw.length <= 3) raw.uppercase() else raw
    val region = parts.getOrNull(1)
    return if (region != null) "$name (${region.uppercase()})" else name
}
