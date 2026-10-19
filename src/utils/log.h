#ifndef CLAK_UTILS_LOG_H
#define CLAK_UTILS_LOG_H

#include <string>
#include <cstdint>

namespace clak {
namespace utils {

void clakLog(const std::string& msg);
void logRttCsv(uint64_t rtt_us, double delta_ms, const std::string& frontend,
               const std::string& group, const std::string& app, const std::string& action);
void setLogEnabled(bool enabled);
bool isLogEnabled();

} // namespace utils
} // namespace clak

#endif
