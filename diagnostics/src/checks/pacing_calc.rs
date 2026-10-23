use super::display::DisplayEnvironment;
use super::latency::LatencyBenchmark;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacingRecommendation {
    pub gap_ms: u32,
    pub terminal_post_delay_ms: u32,
    pub browser_post_delay_ms: u32,
    pub draftjs_post_delay_ms: u32,
    pub heavy_app_post_delay_ms: u32,
    pub estimated_replace_ms: f64,
    pub flicker_risk: String,
    pub explanation: String,
}

pub fn calculate_pacing(
    display: &DisplayEnvironment,
    latency: &LatencyBenchmark,
) -> PacingRecommendation {
    let frame = display.frame_budget_ms;
    let rtt = latency.measured_rtt_ms;
    let jitter = latency.measured_jitter_ms;

    // gap calculation: kernel loopback plus safety margin based on jitter
    let mut gap = (rtt + 1.5 * jitter).ceil() as u32;
    gap = gap.max(2);
    // fail-safe: gap must never exceed half of keyboard repeat interval to prevent long-press triggers
    let max_safe_gap = (latency.keyboard_repeat_interval_ms / 2).max(4);
    gap = gap.min(max_safe_gap);

    // post delay calculations proportional to compositor frame budget
    let terminal = ((0.5 * frame) + rtt).ceil().clamp(4.0, 20.0) as u32;
    let browser = ((1.0 * frame) + rtt).ceil().clamp(6.0, 25.0) as u32;
    let draftjs = ((1.5 * frame) + rtt).ceil().clamp(8.0, 35.0) as u32;
    let heavy = ((2.0 * frame) + rtt).ceil().clamp(12.0, 40.0) as u32;

    let replace_time = (2.0 * (gap as f64)) + (draftjs as f64);

    let (flicker_risk, explanation) = if replace_time <= frame {
        (
            "Hoàn hảo (Không nhấp nháy)".to_string(),
            format!(
                "Thao tác thay thế hoàn tất trong 1 frame ({:.1}ms <= {:.1}ms ở {:.0}Hz)",
                replace_time, frame, display.effective_refresh_rate
            ),
        )
    } else if replace_time <= 2.0 * frame {
        (
            "Thấp (Mắt khó nhận biết)".to_string(),
            format!(
                "Thao tác thay thế hoàn tất trong 2 frames ({:.1}ms <= {:.1}ms ở {:.0}Hz)",
                replace_time,
                2.0 * frame,
                display.effective_refresh_rate
            ),
        )
    } else if replace_time <= 30.0 {
        (
            "Trung bình (Chớp nhẹ ở rich text)".to_string(),
            format!(
                "Thao tác thay thế kéo dài {:.1}ms so với frame budget {:.1}ms",
                replace_time, frame
            ),
        )
    } else {
        (
            "Cao (Khuyên dùng Surrounding Text)".to_string(),
            format!(
                "Độ trễ thay thế {:.1}ms vượt ngưỡng 30ms, ưu tiên dùng Surrounding Text nếu app hỗ trợ",
                replace_time
            ),
        )
    };

    let full_explanation = if display.is_mixed_refresh_rate {
        format!(
            "{} [Áp dụng Fail-Safe đa màn hình: khóa mốc {:.0}Hz]",
            explanation, display.effective_refresh_rate
        )
    } else {
        explanation
    };

    PacingRecommendation {
        gap_ms: gap,
        terminal_post_delay_ms: terminal,
        browser_post_delay_ms: browser,
        draftjs_post_delay_ms: draftjs,
        heavy_app_post_delay_ms: heavy,
        estimated_replace_ms: replace_time,
        flicker_risk,
        explanation: full_explanation,
    }
}
