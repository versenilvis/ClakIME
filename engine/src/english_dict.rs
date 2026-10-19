use std::collections::HashSet;
use std::sync::OnceLock;

static COMMON_ENGLISH_WORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();

pub fn common_english_words() -> &'static HashSet<&'static str> {
    COMMON_ENGLISH_WORDS.get_or_init(|| {
        [
            // articles, pronouns, prepositions, conjunctions
            "a", "about", "above", "across", "after", "again", "against", "all", "almost",
            "alone", "along", "already", "also", "although", "always", "am", "among", "an",
            "and", "another", "any", "anybody", "anyone", "anything", "anywhere", "are", "around",
            "as", "ask", "at", "back", "be", "became", "because", "become", "been", "before",
            "began", "behind", "being", "below", "between", "both", "but", "by", "came", "can",
            "cannot", "come", "could", "did", "do", "does", "doing", "done", "down", "during",
            "each", "either", "else", "even", "ever", "every", "everyone", "everything", "everywhere",
            "for", "from", "get", "give", "go", "good", "got", "had", "has", "have", "having",
            "he", "her", "here", "hers", "herself", "him", "himself", "his", "how", "i", "if",
            "in", "into", "is", "it", "its", "itself", "just", "me", "might", "more", "most",
            "much", "must", "my", "myself", "neither", "no", "nobody", "none", "noone", "nor",
            "not", "nothing", "now", "nowhere", "of", "off", "often", "on", "once", "one",
            "only", "onto", "or", "other", "others", "otherwise", "our", "ours", "ourselves",
            "out", "over", "own", "same", "shall", "she", "should", "since", "so", "some",
            "somebody", "someone", "something", "somewhere", "still", "such", "than", "that",
            "the", "their", "theirs", "them", "themselves", "then", "there", "therefore", "these",
            "they", "this", "those", "though", "through", "throughout", "thru", "thus", "to",
            "together", "too", "toward", "towards", "under", "until", "up", "upon", "us", "very",
            "was", "we", "well", "were", "what", "whatever", "when", "whence", "whenever", "where",
            "whereafter", "whereas", "whereby", "wherein", "whereupon", "wherever", "whether",
            "which", "while", "whither", "who", "whoever", "whole", "whom", "whose", "why",
            "will", "with", "within", "without", "would", "yet", "you", "your", "yours",
            "yourself", "yourselves",
            // gn and hn endings (high risk of coda swap gn->ng, hn->nh)
            "sign", "design", "align", "realign", "assign", "campaign", "foreign", "sovereign",
            "benign", "feign", "reign", "deign", "condign", "ensign", "john", "kahn", "cohn",
            // words ending in f (telex tone 'f' collisions)
            "beef", "belief", "brief", "buff", "calf", "chef", "chief", "cliff", "cuff", "deaf",
            "fluff", "golf", "grief", "half", "leaf", "loaf", "midriff", "muff", "off", "plaintiff",
            "puff", "reef", "relief", "rough", "scuff", "self", "shelf", "sheriff", "sniff", "staff",
            "stiff", "stuff", "tariff", "thief", "tough", "whiff", "wolf",
            // words ending in s (telex tone 's' collisions)
            "access", "across", "address", "always", "analysis", "basis", "boss", "business", "bus",
            "class", "close", "crisis", "cross", "dress", "focus", "gas", "glass", "guess", "less",
            "loss", "mass", "miss", "pass", "plus", "press", "process", "series", "status", "stress",
            "success", "this", "thus", "virus", "yes", "tests",
            // words ending in r (telex tone 'r' collisions)
            "actor", "air", "appear", "author", "bar", "bear", "better", "car", "chair", "clear",
            "color", "computer", "dear", "director", "doctor", "door", "ear", "enter", "error",
            "factor", "fair", "far", "fear", "floor", "four", "fur", "hair", "hear", "leader",
            "letter", "major", "manager", "member", "minor", "near", "never", "number", "offer",
            "order", "other", "pair", "paper", "particular", "player", "poor", "power", "prior",
            "remember", "river", "sector", "senior", "server", "sir", "star", "teacher", "tear",
            "user", "vector", "war", "water", "wear", "year",
            // words ending in x (telex tone 'x' collisions)
            "apex", "box", "complex", "fax", "fix", "flex", "fox", "index", "latex", "matrix",
            "max", "mix", "prefix", "six", "suffix", "syntax", "tax", "vertex", "wax",
            // words containing w (telex 'w' collisions)
            "allow", "arrow", "below", "blow", "borrow", "bow", "brown", "clown", "cow", "crow",
            "crown", "down", "draw", "drown", "fellow", "few", "flow", "follow", "frown", "glow",
            "grow", "hollow", "how", "know", "law", "low", "meadow", "narrow", "new", "now",
            "own", "raw", "review", "row", "saw", "shadow", "show", "slow", "snow", "sorrow",
            "sparrow", "throw", "town", "two", "view", "vow", "window", "wow", "yellow",
            // common double vowel words (telex aa, ee, oo collisions)
            "bleed", "bloom", "book", "boom", "boot", "breed", "cook", "cool", "creed", "deed",
            "deep", "doom", "feed", "feel", "fleet", "flood", "floor", "food", "fool", "foot",
            "free", "gloom", "good", "green", "greet", "heel", "hook", "keen", "keep", "knee",
            "look", "loop", "meet", "mood", "moon", "need", "noon", "peel", "pool", "poor",
            "queen", "roof", "room", "root", "screen", "seed", "seek", "seen", "sheet", "shook",
            "shoot", "sleep", "soon", "speed", "spoon", "steel", "steep", "sweet", "took", "tool",
            "tree", "weed", "week", "weep", "wheel", "wood", "wool", "zoom",
            // common technical and system keywords
            "abstract", "api", "app", "args", "async", "auto", "await", "bool", "break", "byte",
            "case", "cast", "catch", "char", "check", "chromium", "class", "client", "clone", "cmake",
            "code", "config", "const", "continue", "data", "date", "debug", "def", "default", "delete",
            "desk", "diff", "disk", "div", "double", "draftjs", "drop", "echo", "enum", "error",
            "event", "exit", "export", "extends", "false", "fast", "fcitx", "fcitx5", "file", "final",
            "finally", "find", "first", "float", "fn", "font", "func", "gecko", "git", "goto", "gui",
            "hide", "home", "host", "href", "html", "http", "https", "hyprland", "id", "ime", "impl",
            "import", "index", "info", "init", "input", "int", "interface", "io", "item", "json",
            "keep", "key", "last", "layout", "len", "let", "like", "line", "link", "linux", "list",
            "load", "log", "loop", "macro", "main", "make", "map", "mask", "match", "menu",
            "message", "meta", "method", "mod", "mode", "model", "mut", "name", "net", "new",
            "next", "nil", "node", "null", "num", "obj", "open", "opt", "output", "package",
            "page", "param", "parse", "part", "past", "path", "people", "pkg", "plugin", "point",
            "port", "post", "print", "private", "prop", "protected", "ptr", "pub", "public", "push",
            "query", "read", "ref", "req", "res", "reset", "resp", "rest", "result", "return",
            "risk", "root", "route", "row", "rule", "run", "rust", "rustc", "save", "say", "scan",
            "scope", "see", "seed", "select", "send", "server", "service", "set", "side", "size",
            "sort", "span", "src", "stack", "start", "state", "static", "stop", "str", "string",
            "struct", "style", "super", "switch", "sync", "table", "tag", "take", "task", "term",
            "test", "text", "theme", "think", "this", "throw", "time", "token", "trait", "true",
            "try", "type", "ui", "uinput", "undefined", "unit", "update", "url", "user", "val",
            "value", "var", "vec", "view", "void", "want", "warn", "way", "wayland", "where",
            "while", "wide", "width", "window", "word", "work", "write", "xml", "yield",
        ]
        .into_iter()
        .collect()
    })
}

// checks if word is in common english or tech vocabulary
pub fn is_english(word: &str) -> bool {
    let lower = word.to_lowercase();
    common_english_words().contains(lower.as_str())
}
