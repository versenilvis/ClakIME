#ifndef CLAK_CORE_H
#define CLAK_CORE_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#define CLAK_METHOD_TELEX    0
#define CLAK_METHOD_VNI      1
#define CLAK_METHOD_VIQR     2
#define CLAK_METHOD_TEIP_VNI 3

typedef void ClakCore;

ClakCore *clak_core_new(int32_t method);
void      clak_core_free(ClakCore *e);

void clak_core_set_method(ClakCore *e, int32_t method);
void clak_core_set_tone_style(ClakCore *e, int32_t modern);
void clak_core_set_free_marking(ClakCore *e, int32_t free);
void clak_core_set_short_w(ClakCore *e, int32_t enabled);
void clak_core_set_auto_restore(ClakCore *e, int32_t enabled);
void clak_core_set_dict(ClakCore *e, int32_t enabled);
void clak_core_add_word(ClakCore *e, const char *word);
void clak_core_clear_words(ClakCore *e);
char *clak_core_dict_words(void);
void clak_core_set_bracket_uo(ClakCore *e, int32_t enabled);

char *clak_core_transform(ClakCore *e, const char *input);
int32_t clak_core_is_valid(const char *s);
char *clak_autofix_text(const char *input, int32_t method, int32_t modern, int32_t *changed_out);
void clak_free_string(char *s);

uint8_t *clak_charset_encode(const char *input, int32_t charset, size_t *out_len);
char *clak_charset_decode(const uint8_t *input, size_t len, int32_t charset);
char *clak_charset_remove_tone(const char *input);
void clak_charset_free_buf(uint8_t *buf);

typedef void ClakContext;

#define CLAK_ACTION_FORWARD             0
#define CLAK_ACTION_COMMIT              1
#define CLAK_ACTION_REPLACE_SURROUNDING 2
#define CLAK_ACTION_ADDRESS_BAR_FIX     3
#define CLAK_ACTION_REPLACE             4
#define CLAK_ACTION_UINPUT_REPLACE      4
#define CLAK_ACTION_AUTOCORRECT         5

// Lifetime contract:
// commit_str and delete_str are borrowed from the internal buffers of ClakContext.
// They are valid synchronously until the next call to clak_process_key, clak_context_reset,
// or clak_context_free on the same context. Callers must copy them immediately if needed
// across asynchronous event loop dispatches.
typedef struct {
    int32_t     action_type;
    size_t      delete_count;
    const char *commit_str;
    const char *delete_str;
} ClakAction;

ClakContext *clak_context_new(int32_t method);
void         clak_context_free(ClakContext *ctx);
void         clak_context_reset(ClakContext *ctx);
bool         clak_is_composing(const ClakContext *ctx);

typedef void ClakConfig;

char       *clak_config_path(void);
ClakConfig *clak_config_load(void);
ClakConfig *clak_config_default(void);
void        clak_config_free(ClakConfig *cfg);
bool        clak_config_is_app_excluded(const ClakConfig *cfg, const char *app_name);
bool        clak_config_get_remember_state(const ClakConfig *cfg);
bool        clak_config_get_uinput_ack(const ClakConfig *cfg);
bool        clak_config_get_debug_log(const ClakConfig *cfg);
bool        clak_config_get_adaptive_pacing(const ClakConfig *cfg);
int32_t     clak_config_get_pacing_preset(const ClakConfig *cfg);
uint32_t    clak_config_get_draftjs_delay_ms(const ClakConfig *cfg);
uint32_t    clak_config_get_draftjs_gap_ms(const ClakConfig *cfg);
uint32_t    clak_config_get_terminal_delay_ms(const ClakConfig *cfg);
uint32_t    clak_config_get_terminal_gap_ms(const ClakConfig *cfg);
uint32_t    clak_config_get_heavy_app_delay_ms(const ClakConfig *cfg);
uint32_t    clak_config_get_heavy_app_gap_ms(const ClakConfig *cfg);
bool        clak_config_get_shift_autofix(const ClakConfig *cfg);
void        clak_config_set_shift_autofix(ClakConfig *cfg, bool enabled);
bool        clak_config_get_autocorrect(const ClakConfig *cfg);
void        clak_config_set_autocorrect(ClakConfig *cfg, bool enabled);
void        clak_config_set_auto_restore(ClakConfig *cfg, bool enabled);
void        clak_config_set_spelling(ClakConfig *cfg, bool enabled);
int32_t     clak_config_get_startup_mode(const ClakConfig *cfg);
int32_t     clak_config_get_method(const ClakConfig *cfg);
char       *clak_config_get_toggle_shortcut(const ClakConfig *cfg);
char       *clak_config_get_switch_shortcut(const ClakConfig *cfg);
void        clak_context_apply_config(ClakContext *ctx, const ClakConfig *cfg);

ClakAction   clak_process_key(ClakContext *ctx,
                              uint32_t key_sym,
                              const char *key_str,
                              bool has_ctrl_alt,
                              const char *surrounding_text,
                              size_t cursor,
                              size_t anchor);

bool         clak_observe_retype(const char *old_raw, const char *new_word);

#ifdef __cplusplus
}
#endif

#endif
