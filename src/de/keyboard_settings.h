#ifndef CLAK_DE_KEYBOARD_SETTINGS_H
#define CLAK_DE_KEYBOARD_SETTINGS_H

#include <cstdint>
#include <optional>

namespace clak {
namespace de {

uint64_t getRepeatDelayUs();
uint64_t getRepeatIntervalUs();

void setMockRepeatSettings(std::optional<uint64_t> delay_us, std::optional<uint64_t> interval_us);
void clearMockRepeatSettings();

} // namespace de

// backward compatibility alias
namespace platform {
using de::getRepeatDelayUs;
using de::getRepeatIntervalUs;
using de::setMockRepeatSettings;
using de::clearMockRepeatSettings;
} // namespace platform

} // namespace clak

#endif
