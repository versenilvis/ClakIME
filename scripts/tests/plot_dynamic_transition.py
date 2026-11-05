#!/usr/bin/env python3
import csv
import sys
import os
from PIL import Image, ImageDraw, ImageFont

def plot_transition(csv_path="/tmp/dyn_adaptive.csv", out_png="/tmp/dynamic_load_transition.png"):
    if not os.path.exists(csv_path):
        print(f"Error: {csv_path} does not exist")
        return

    rows = []
    with open(csv_path, "r", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for r in reader:
            rows.append({
                "word_idx": int(r["word_index"]),
                "phase": int(r["phase"]),
                "exp": r["expected_word"],
                "act": r["actual_word"],
                "delay": int(r["post_delay_ms"]),
                "rtt": int(r["rtt_ms"]),
                "ssthresh": int(r["ssthresh_ms"]),
                "is_error": int(r["is_error"])
            })

    if not rows:
        print("Error: empty csv")
        return

    width, height = 1200, 600
    margin_left, margin_right = 80, 50
    margin_top, margin_bottom = 80, 80
    plot_w = width - margin_left - margin_right
    plot_h = height - margin_top - margin_bottom

    img = Image.new("RGB", (width, height), color=(26, 27, 38))
    draw = ImageDraw.Draw(img)

    max_delay = max(max(r["delay"] for r in rows), max(r["rtt"] for r in rows), 40)
    max_delay = ((max_delay + 9) // 10) * 10
    total_words = len(rows)

    def to_coords(w_idx, val):
        x = margin_left + ((w_idx - 1) / (total_words - 1)) * plot_w
        y = margin_top + plot_h - (val / max_delay) * plot_h
        return (x, y)

    # background grid and y-axis labels
    for v in range(0, max_delay + 1, 5):
        _, y = to_coords(1, v)
        draw.line([(margin_left, y), (width - margin_right, y)], fill=(45, 48, 65), width=1)
        draw.text((margin_left - 45, y - 7), f"{v:2d}ms", fill=(160, 165, 185))

    # phase boundaries
    p1_end = 42
    p2_end = 98

    x_p1_end, _ = to_coords(p1_end, 0)
    x_p2_end, _ = to_coords(p2_end, 0)

    # phase background shading
    draw.rectangle([margin_left, margin_top, x_p1_end, margin_top + plot_h], fill=(30, 36, 50))
    draw.rectangle([x_p1_end, margin_top, x_p2_end, margin_top + plot_h], fill=(42, 32, 45))
    draw.rectangle([x_p2_end, margin_top, width - margin_right, margin_top + plot_h], fill=(30, 36, 50))

    # re-draw grid lines over shading
    for v in range(0, max_delay + 1, 10):
        _, y = to_coords(1, v)
        draw.line([(margin_left, y), (width - margin_right, y)], fill=(55, 58, 80), width=1)

    # vertical dividing lines
    draw.line([(x_p1_end, margin_top), (x_p1_end, margin_top + plot_h)], fill=(255, 100, 100), width=2)
    draw.line([(x_p2_end, margin_top), (x_p2_end, margin_top + plot_h)], fill=(100, 255, 100), width=2)

    # phase banner labels
    draw.text((margin_left + 20, margin_top + 10), "PHA 1: RẢNH (Từ 1 - 42)", fill=(120, 180, 255))
    draw.text((x_p1_end + 20, margin_top + 10), "PHA 2: BẬT TẢI NẶNG (Từ 43 - 98)", fill=(255, 140, 140))
    draw.text((x_p2_end + 20, margin_top + 10), "PHA 3: TẮT TẢI, PHỤC HỒI (Từ 99 - 140)", fill=(140, 255, 140))

    # plot rtt line (light green)
    rtt_points = [to_coords(r["word_idx"], r["rtt"]) for r in rows]
    draw.line(rtt_points, fill=(100, 220, 150), width=2)

    # plot post_delay line (bright cyan/blue)
    delay_points = [to_coords(r["word_idx"], r["delay"]) for r in rows]
    draw.line(delay_points, fill=(80, 190, 255), width=3)

    # mark error words if any
    err_count = 0
    for r in rows:
        if r["is_error"]:
            err_count += 1
            x, y = to_coords(r["word_idx"], r["delay"])
            r_circle = 6
            draw.ellipse([x - r_circle, y - r_circle, x + r_circle, y + r_circle], fill=(255, 50, 50), outline=(255, 255, 255))
            draw.text((x - 10, y - 22), f"ERR#{r['word_idx']}", fill=(255, 80, 80))

    # axes borders
    draw.line([(margin_left, margin_top), (margin_left, margin_top + plot_h)], fill=(200, 205, 220), width=2)
    draw.line([(margin_left, margin_top + plot_h), (width - margin_right, margin_top + plot_h)], fill=(200, 205, 220), width=2)

    # x-axis ticks
    for w in range(10, total_words + 1, 10):
        x, y = to_coords(w, 0)
        draw.line([(x, y), (x, y + 5)], fill=(180, 185, 200), width=2)
        draw.text((x - 8, y + 10), str(w), fill=(160, 165, 185))

    # title and legend
    draw.text((margin_left, 25), "CLAK ADAPTIVE PACING: CHUỖI PHẢN ỨNG ĐỘ TRỄ KHI BẬT TẢI ĐỘT NGỘT", fill=(255, 255, 255))
    draw.text((margin_left, 45), f"Tập dữ liệu: {csv_path} | Tổng từ: {total_words} | Tổng lỗi: {err_count}", fill=(180, 185, 200))

    # legend
    leg_x = width - margin_right - 350
    draw.line([(leg_x, 35), (leg_x + 30, 35)], fill=(80, 190, 255), width=3)
    draw.text((leg_x + 35, 28), "Adaptive Post Delay (ms)", fill=(220, 225, 240))

    draw.line([(leg_x + 200, 35), (leg_x + 230, 35)], fill=(100, 220, 150), width=2)
    draw.text((leg_x + 235, 28), "Observed RTT (ms)", fill=(220, 225, 240))

    # x axis title
    draw.text((width // 2 - 50, height - margin_bottom + 45), "Thứ tự từ trong đoạn văn (1 -> 140)", fill=(200, 205, 220))

    img.save(out_png)
    print(f"Chart saved cleanly to {out_png}")

if __name__ == "__main__":
    csv_file = sys.argv[1] if len(sys.argv) > 1 else "/tmp/dyn_adaptive.csv"
    out_file = sys.argv[2] if len(sys.argv) > 2 else "/tmp/dynamic_load_transition.png"
    plot_transition(csv_file, out_file)

