#ifndef CLAK_UTILS_TEXT_UTILS_H
#define CLAK_UTILS_TEXT_UTILS_H

#include <string>
#include <cstddef>

namespace clak {
namespace utils {

void popUtf8Chars(std::string& s, size_t count);
size_t utf8CharCount(const std::string& s);
std::string extractWordBeforeCursor(const std::string& text, size_t cursor_chars);
std::string extractSelection(const std::string& text, size_t cursor_chars, size_t anchor_chars);

} // namespace utils
} // namespace clak

#endif
