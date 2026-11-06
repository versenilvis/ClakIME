#include "log.h"
#include <cstdio>
#include <chrono>
#include <ctime>
#include <cstdlib>
#include <fcntl.h>
#include <unistd.h>
#include <sys/stat.h>
#include <unordered_map>
#include <fstream>

namespace clak {
namespace utils {

constexpr off_t kMaxLogSizeBytes = 2 * 1024 * 1024;
static bool s_log_enabled = false;
static bool s_checked_env = false;

const char* clakGetEnv(const char* key) {
    static std::unordered_map<std::string, std::string> s_test_env;
    static long s_last_mtime_sec = -1;
    static long s_last_mtime_nsec = -1;
    static bool s_file_present = false;

    struct stat st{};
    if (lstat("/tmp/clak_test_env.conf", &st) == 0 && S_ISREG(st.st_mode) && st.st_uid == getuid()) {
        if (!s_file_present || st.st_mtim.tv_sec != s_last_mtime_sec || st.st_mtim.tv_nsec != s_last_mtime_nsec) {
            s_file_present = true;
            s_last_mtime_sec = st.st_mtim.tv_sec;
            s_last_mtime_nsec = st.st_mtim.tv_nsec;
            s_test_env.clear();
            std::ifstream ifs("/tmp/clak_test_env.conf");
            std::string line;
            while (std::getline(ifs, line)) {
                auto eq = line.find('=');
                if (eq != std::string::npos) {
                    s_test_env[line.substr(0, eq)] = line.substr(eq + 1);
                }
            }
        }
        auto it = s_test_env.find(key);
        if (it != s_test_env.end()) {
            return it->second.c_str();
        }
        return nullptr;
    } else if (s_file_present) {
        s_file_present = false;
        s_test_env.clear();
    }
    return std::getenv(key);
}

void setLogEnabled(bool enabled) {
    s_log_enabled = enabled;
    s_checked_env = true;
}

bool isLogEnabled() {
    if (const char* t_env = clakGetEnv("CLAK_LOG")) {
        return (std::string(t_env) != "0" && std::string(t_env) != "false" && std::string(t_env) != "off");
    }
    if (!s_checked_env) {
        s_checked_env = true;
        const char* env = getenv("CLAK_LOG");
        if (env) {
            s_log_enabled = (std::string(env) != "0" && std::string(env) != "false" && std::string(env) != "off");
        }
    }
    return s_log_enabled;
}

void clakLog(const std::string& msg) {
    if (!isLogEnabled()) return;

    const char* path = "/tmp/clak.log";
    // open with nofollow and strict user permissions to mitigate symlink and info disclosure attacks
    int fd = open(path, O_WRONLY | O_CREAT | O_APPEND | O_NOFOLLOW | O_CLOEXEC, 0600);
    if (fd < 0) return;

    struct stat st{};
    if (fstat(fd, &st) != 0 || !S_ISREG(st.st_mode) || st.st_uid != getuid()) {
        close(fd);
        return;
    }

    if (st.st_size > kMaxLogSizeBytes) {
        // truncate atomically on open fd without reopening path
        if (ftruncate(fd, 0) != 0) {
            close(fd);
            return;
        }
    }

    FILE* log_fp = fdopen(fd, "a");
    if (!log_fp) {
        close(fd);
        return;
    }

    auto now = std::chrono::system_clock::now();
    auto t = std::chrono::system_clock::to_time_t(now);
    auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(now.time_since_epoch()).count() % 1000;
    struct tm tm_buf{};
    localtime_r(&t, &tm_buf);
    char ts[32];
    snprintf(ts, sizeof(ts), "%02d:%02d:%02d.%03ld",
             tm_buf.tm_hour, tm_buf.tm_min, tm_buf.tm_sec, ms);
    fprintf(log_fp, "[%s] %s\n", ts, msg.c_str());
    fclose(log_fp);
}

void logRttCsv(uint64_t rtt_us, double delta_ms, const std::string& frontend,
               const std::string& group, const std::string& app, const std::string& action) {
    const char* env_path = getenv("CLAK_RTT_CSV");
    const char* path = (env_path && env_path[0] != '\0') ? env_path : "/tmp/clak_rtt.csv";

    // open with nofollow and user-only permissions
    int fd = open(path, O_WRONLY | O_CREAT | O_APPEND | O_NOFOLLOW | O_CLOEXEC, 0600);
    if (fd < 0) return;

    struct stat st{};
    if (fstat(fd, &st) != 0 || !S_ISREG(st.st_mode) || st.st_uid != getuid()) {
        close(fd);
        return;
    }

    FILE* fp = fdopen(fd, "a");
    if (!fp) {
        close(fd);
        return;
    }

    // write header on newly created file
    if (st.st_size == 0) {
        fprintf(fp, "timestamp,rtt_us,rtt_ms,delta_ms,frontend,group,app,action\n");
    }

    auto now = std::chrono::system_clock::now();
    auto t = std::chrono::system_clock::to_time_t(now);
    auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(now.time_since_epoch()).count() % 1000;
    struct tm tm_buf{};
    localtime_r(&t, &tm_buf);
    char ts[32];
    snprintf(ts, sizeof(ts), "%02d:%02d:%02d.%03ld",
             tm_buf.tm_hour, tm_buf.tm_min, tm_buf.tm_sec, ms);

    double rtt_ms = static_cast<double>(rtt_us) / 1000.0;
    fprintf(fp, "%s,%llu,%.3f,%.2f,%s,%s,%s,%s\n",
            ts,
            static_cast<unsigned long long>(rtt_us),
            rtt_ms,
            delta_ms,
            frontend.empty() ? "unknown" : frontend.c_str(),
            group.empty() ? "unknown" : group.c_str(),
            app.empty() ? "unknown" : app.c_str(),
            action.c_str());
    fclose(fp);
}

} // namespace utils
} // namespace clak
