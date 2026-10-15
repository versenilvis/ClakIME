#ifndef CLAK_CONFIG_CONFIG_H
#define CLAK_CONFIG_CONFIG_H

#include <cstddef>
#include <cstdint>

namespace clak {
namespace config {

constexpr size_t kMaxBufferedKeys = 50;
constexpr int kMismatchThreshold = 3;
constexpr uint64_t kSafetyTimeoutUs = 200000;
constexpr uint64_t kSelectionDeletionTimeoutUs = 250000;
constexpr uint32_t kAddressBarPostDelayMs = 20;
constexpr uint32_t kTerminalPostDelayMs = 4;
constexpr uint32_t kTerminalGapMs = 2;
constexpr uint32_t kDraftJsPostDelayMs = 16;
constexpr uint32_t kDraftJsGapMs = 4;
constexpr uint64_t kCacheCheckIntervalUs = 500000;

} // namespace config
} // namespace clak

#endif
