import os
import sys
import time
import subprocess
import json
import argparse

def simulate_terminal(raw_bytes):
    # simulate terminal backspace deletion on byte stream
    words = []
    current_chars = []

    # decode utf-8 characters properly
    i = 0
    n = len(raw_bytes)
    while i < n:
        b = raw_bytes[i]
        if b in (0x7f, 0x08): # backspace
            if current_chars:
                current_chars.pop()
            i += 1
        elif b in (0x20, 0x0a, 0x0d): # space or newline (word boundary)
            if current_chars:
                words.append("".join(current_chars))
                current_chars = []
            i += 1
        else:
            # determine utf-8 byte length
            if b < 0x80:
                char_len = 1
            elif (b & 0xe0) == 0xc0:
                char_len = 2
            elif (b & 0xf0) == 0xe0:
                char_len = 3
            elif (b & 0xf8) == 0xf0:
                char_len = 4
            else:
                char_len = 1

            chunk = bytes(raw_bytes[i:i+char_len])
            try:
                char_str = chunk.decode("utf-8")
                current_chars.append(char_str)
            except Exception:
                pass
            i += char_len

    if current_chars:
        words.append("".join(current_chars))
    return words

def main():
    parser = argparse.ArgumentParser(description="Control Experiment for Clak Input Method")
    parser.add_argument("--count", type=int, default=300, help="Number of repetitions (default: 300)")
    parser.add_argument("--gap", type=int, default=5, help="Typing gap in ms for wtype (default: 5)")
    parser.add_argument("--word", type=str, default="tieengs", help="Raw input word (default: tieengs)")
    parser.add_argument("--expected", type=str, default="tiếng", help="Expected word (default: tiếng)")
    parser.add_argument("--delay-sleep", type=float, default=0.25, help="Sleep between words in seconds (default: 0.25)")
    args = parser.parse_args()

    events_file = "/tmp/clak_control_exp.jsonl"
    ready_file = "/tmp/clak_control_exp.ready"
    if os.path.exists(events_file):
        os.remove(events_file)
    if os.path.exists(ready_file):
        os.remove(ready_file)

    script_dir = os.path.dirname(os.path.abspath(__file__))
    receiver_script = os.path.join(script_dir, "raw_receiver.py")
    window_title = "clak_experiment_window"

    print(f"=== CLAK CONTROL EXPERIMENT ===")
    print(f"Trials: {args.count}")
    print(f"Key gap: {args.gap}ms (rapid typing)")
    print(f"Word: '{args.word}' -> expected: '{args.expected}'")
    print(f"Events log: {events_file}")

    # launch kitty running raw_receiver
    kitty_cmd = ["kitty", "--title", window_title, "python3", receiver_script, events_file, ready_file]
    proc = subprocess.Popen(kitty_cmd)

    # wait for receiver readiness marker
    t0 = time.time()
    while time.time() - t0 < 5.0:
        if os.path.exists(ready_file):
            break
        time.sleep(0.05)

    # ensure focus in hyprland and verify activewindow
    focus_lua = f'for _, w in ipairs(hl.get_windows()) do if w.title == "{window_title}" then hl.dispatch(hl.dsp.focus({{ window = w }})) break end end'
    subprocess.run(["hyprctl", "repl", focus_lua], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    t0 = time.time()
    while time.time() - t0 < 3.0:
        res = subprocess.run(["hyprctl", "activewindow", "-j"], capture_output=True, text=True)
        try:
            data = json.loads(res.stdout)
            if data.get("title") == window_title:
                break
        except Exception:
            pass
        time.sleep(0.05)

    time.sleep(0.1)

    # switch fcitx5 to clak
    subprocess.run(["fcitx5-remote", "-s", "clak"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    input_text = f"{args.word} "
    print(f"Bắt đầu phát {args.count} lần từ '{args.word}'...")
    start_time = time.time()

    for trial in range(1, args.count + 1):
        subprocess.run(["wtype", "-d", str(args.gap), input_text], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(args.delay_sleep)
        if trial % 25 == 0 or trial == args.count:
            elapsed = time.time() - start_time
            rate = trial / elapsed if elapsed > 0 else 0
            print(f"  Tiến độ: {trial}/{args.count} ({rate:.1f} từ/giây)")

    # finish receiver
    time.sleep(0.5)
    subprocess.run(["wtype", "-M", "ctrl", "-k", "d", "-m", "ctrl"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.5)

    try:
        proc.terminate()
        proc.wait(timeout=2.0)
    except Exception:
        proc.kill()

    # analyze results
    if not os.path.exists(events_file):
        print("ERROR: Events file was not created!")
        return

    raw_bytes = []
    with open(events_file, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                rec = json.loads(line)
                raw_bytes.append(rec["b"])
            except Exception:
                pass

    total_bytes = len(raw_bytes)
    bs_bytes = sum(1 for b in raw_bytes if b in (0x7f, 0x08))
    words = simulate_terminal(raw_bytes)

    print("\n=== KẾT QUẢ THÍ NGHIỆM ĐỐI CHỨNG ===")
    print(f"Tổng số byte nhận từ stdin: {total_bytes} (trong đó {bs_bytes} phím Backspace 0x7f)")
    print(f"Tổng số từ nhận được: {len(words)}")

    correct = 0
    state_machine_leaks = 0 # e.g. tiêngs
    late_bs_errors = 0      # e.g. tieếng, tiêếng
    other_errors = {}

    for w in words:
        if w == args.expected:
            correct += 1
        elif "s" in w and ("tiêng" in w or "tieng" in w):
            state_machine_leaks += 1
            other_errors[w] = other_errors.get(w, 0) + 1
        elif "tie" in w or "tiêê" in w:
            late_bs_errors += 1
            other_errors[w] = other_errors.get(w, 0) + 1
        else:
            other_errors[w] = other_errors.get(w, 0) + 1

    total_samples = len(words)
    acc = (correct / total_samples * 100) if total_samples > 0 else 0
    print(f"\nChính xác ('{args.expected}'): {correct}/{total_samples} ({acc:.2f}%)")
    print(f"Lỗi State Machine rơi phím thô (vd: tiêngs): {state_machine_leaks}")
    print(f"Lỗi Timing/Delay Backspace đến muộn (vd: tieếng, tiêếng): {late_bs_errors}")

    if other_errors:
        print("\nChi tiết các dạng lỗi xuất hiện:")
        for err_w, count in sorted(other_errors.items(), key=lambda x: -x[1]):
            print(f"  '{err_w}': {count} lần")

if __name__ == "__main__":
    main()
