export const SUPPORTED_LANGUAGES = {
    "en": "English",
    "de": "German",
    "ar": "Arabic",
    "fr": "French",
    "hi": "Hindi",
    "it": "Italian",
    "ja": "Japanese",
    "ko": "Korean",
    "nl": "Dutch",
    "pl": "Polish",
    "ru": "Russian",
    "tr": "Turkish",
    "zh": "Chinese"
} as const;

export const LANGUAGE_CODES = Object.keys(SUPPORTED_LANGUAGES);