#include "text_utils.h"
#include <vector>

namespace clak {
namespace utils {

void popUtf8Chars(std::string& s, size_t count) {
    while (count > 0 && !s.empty()) {
        while (!s.empty() && (static_cast<unsigned char>(s.back()) & 0xc0) == 0x80) {
            s.pop_back();
        }
        if (!s.empty()) {
            s.pop_back();
        }
        --count;
    }
}

std::string extractWordBeforeCursor(const std::string& text, size_t cursor_chars) {
    if (text.empty() || cursor_chars == 0) {
        return "";
    }
    std::vector<std::string> chars;
    size_t byte_idx = 0;
    while (byte_idx < text.size() && chars.size() < cursor_chars) {
        size_t len = 1;
        unsigned char c = static_cast<unsigned char>(text[byte_idx]);
        if ((c & 0x80) == 0) len = 1;
        else if ((c & 0xe0) == 0xc0) len = 2;
        else if ((c & 0xf0) == 0xe0) len = 3;
        else if ((c & 0xf8) == 0xf0) len = 4;
        if (byte_idx + len > text.size()) break;
        chars.push_back(text.substr(byte_idx, len));
        byte_idx += len;
    }

    size_t start = chars.size();
    while (start > 0) {
        const std::string& ch = chars[start - 1];
        if (ch.size() == 1) {
            char c = ch[0];
            if (c <= 32 || c == '.' || c == ',' || c == ';' || c == ':' ||
                c == '?' || c == '!' || c == '/' || c == '\\' || c == '(' ||
                c == ')' || c == '[' || c == ']' || c == '{' || c == '}' ||
                c == '<' || c == '>' || c == '"' || c == '\'' || c == '`' ||
                c == '~' || c == '@' || c == '#' || c == '$' || c == '%' ||
                c == '^' || c == '&' || c == '*' || c == '-' || c == '+' ||
                c == '=' || c == '|') {
                break;
            }
        }
        start--;
    }

    std::string word;
    for (size_t i = start; i < chars.size(); ++i) {
        word += chars[i];
    }
    return word;
}

size_t utf8CharCount(const std::string& s) {
    size_t count = 0;
    for (char c : s) {
        if ((static_cast<unsigned char>(c) & 0xc0) != 0x80) {
            count++;
        }
    }
    return count;
}

std::string extractSelection(const std::string& text, size_t cursor_chars, size_t anchor_chars) {
    if (text.empty() || cursor_chars == anchor_chars) {
        return "";
    }
    size_t start_char = std::min(cursor_chars, anchor_chars);
    size_t end_char = std::max(cursor_chars, anchor_chars);

    std::vector<std::string> chars;
    size_t byte_idx = 0;
    while (byte_idx < text.size() && chars.size() < end_char) {
        size_t len = 1;
        unsigned char c = static_cast<unsigned char>(text[byte_idx]);
        if ((c & 0x80) == 0) len = 1;
        else if ((c & 0xe0) == 0xc0) len = 2;
        else if ((c & 0xf0) == 0xe0) len = 3;
        else if ((c & 0xf8) == 0xf0) len = 4;
        if (byte_idx + len > text.size()) break;
        chars.push_back(text.substr(byte_idx, len));
        byte_idx += len;
    }

    if (start_char >= chars.size()) {
        return "";
    }
    size_t actual_end = std::min(end_char, chars.size());
    std::string result;
    for (size_t i = start_char; i < actual_end; ++i) {
        result += chars[i];
    }
    return result;
}

} // namespace utils
} // namespace clak
