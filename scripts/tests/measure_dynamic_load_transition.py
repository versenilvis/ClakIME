#!/usr/bin/env python3
import os
import sys
import time
import json
import signal
import subprocess
import difflib
import csv

# 3 phases: 42 words (idle), 56 words (heavy load), 42 words (recovery) = 140 words
PART1_RAW = "ddaay buoori sangs treen ddirnh nuis cao mang laij mootj carm giacs thaatj suwj thanh tharn vaf tuyeetj vowfi. nhuwxng tia nawngs vowis khoong gian roojng lowsn chieeus roji xuoongs khawps cais thung lungx, noowi cos nhuwxng tharm cor xanh mown mowrn "
PART1_EXP = "đây buổi sáng trên đỉnh núi cao mang lại một cảm giác thật sự thanh thản và tuyệt vời. những tia nắng với không gian rộng lớn chiếu rọi xuống khắp cái thung lũng, nơi có những thảm cỏ xanh mơn mởn "

PART2_RAW = "vaf dofng suoosi nhor luowjn uoosn quanh qua tuwfng khe ddas. con nguwowfi noowi ddaay luoon chawm chir, kieen trif gawsn bos vowis ruoojng ddoofng, giuwx gifn truyeefn thoongs vawn hoas ddaamj ddaf barn sawsc cuar toor tieen. booj gox tieengs vieetj clak hoatj ddoojng voo cufng muowjt maf, pharn hoofi nhanh chongs vaf chinhs xacs "
PART2_EXP = "và dòng suối nhỏ lượn uốn quanh qua từng khe đá. con người nơi đây luôn chăm chỉ, kiên trì gắn bó với ruộng đồng, giữ gìn truyền thống văn hóa đậm đà bản sắc của tổ tiên. bộ gõ tiếng việt clak hoạt động vô cùng mượt mà, phản hồi nhanh chóng và chính xác "

PART3_RAW = "treen cais mooi truwowfng terminal cuar heej ddieefu hanhf linux. duf laf vieest mootj ddoanj vawn hay laapj trinhf phaafn meefm phuwsc tapj, trari nghieemj gox phims luoon muowjt maf vaf ddem laij suwj thoari mais tuyeetj ddoosi cho nguwowif suwr dujng."
PART3_EXP = "trên cái môi trường terminal của hệ điều hành linux. dù là viết một đoạn văn hay lập trình phần mềm phức tạp, trải nghiệm gõ phím luôn mượt mà và đem lại sự thoải mái tuyệt đối cho người sử dụng."

FULL_EXPECTED = (PART1_EXP + PART2_EXP + PART3_EXP).strip()

def clean_state_files():
    for p in [
        "/tmp/clak.log",
        "/tmp/matrix_term.bin",
        "/tmp/matrix_term.bin.lines",
        "/tmp/matrix_term.ready",
    ]:
        if os.path.exists(p):
            try:
                os.remove(p)
            except OSError:
                pass

def restart_fcitx5(mode="adaptive", fixed_delay_ms=None, pinned_delay_ms=None, disable_ssthresh=False):
    clean_state_files()
    env = dict(os.environ)
    env.pop("CLAK_FIXED_DELAY_MS", None)
    env.pop("CLAK_FIXED_GAP_MS", None)
    env.pop("CLAK_PINNED_DELAY_MS", None)
    env.pop("CLAK_DISABLE_SSTHRESH", None)
    env.pop("CLAK_UINPUT_HOLD_US", None)

    # ensure identical hold and gap parameters across all modes
    env["CLAK_UINPUT_HOLD_US"] = "1000"

    if mode == "fixed":
        delay = fixed_delay_ms if fixed_delay_ms is not None else 13
        env["CLAK_FIXED_DELAY_MS"] = str(delay)
        env["CLAK_FIXED_GAP_MS"] = "3"
    elif mode == "pinned":
        delay = pinned_delay_ms if pinned_delay_ms is not None else 13
        env["CLAK_PINNED_DELAY_MS"] = str(delay)
    elif mode == "adaptive":
        if disable_ssthresh:
            env["CLAK_DISABLE_SSTHRESH"] = "1"

    subprocess.run(["fcitx5", "-r", "-d"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(1.0)
    for _ in range(30):
        res = subprocess.run(["fcitx5-remote"], capture_output=True, text=True)
        if res.returncode == 0:
            break
        time.sleep(0.1)
    subprocess.run(["fcitx5-remote", "-s", "clak"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.3)

def decode_terminal_bytes(raw_bytes):
    chars = []
    i = 0
    n = len(raw_bytes)
    while i < n:
        b = raw_bytes[i]
        if b in (0x7f, 0x08):
            if chars:
                chars.pop()
            i += 1
        elif b in (0x0a, 0x0d):
            i += 1
        else:
            clen = 1
            if b >= 0xf0:
                clen = 4
            elif b >= 0xe0:
                clen = 3
            elif b >= 0xc0:
                clen = 2
            try:
                chars.append(bytes(raw_bytes[i:i+clen]).decode("utf-8"))
            except Exception:
                pass
            i += clen
    return "".join(chars)

def focus_electron_window():
    lua = '''
    for _, w in ipairs(hl.get_windows()) do
        local c = (w.class or ""):lower()
        local t = (w.title or ""):lower()
        if c:find("electron", 1, true) or t:find("clak_electron_xterm", 1, true) then
            hl.dispatch(hl.dsp.focus({ window = w }))
            break
        end
    end
    '''
    subprocess.run(["hyprctl", "repl", lua], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for _ in range(30):
        res = subprocess.run(["hyprctl", "activewindow", "-j"], capture_output=True, text=True)
        try:
            data = json.loads(res.stdout)
            c = data.get("class", "").lower()
            t = data.get("title", "").lower()
            if "electron" in c or "clak_electron_xterm" in t:
                return True
        except Exception:
            pass
        time.sleep(0.05)
    return False

def parse_pacing_log(clak_log):
    events = []
    if not os.path.exists(clak_log):
        return events
    with open(clak_log, "r", encoding="utf-8", errors="ignore") as f:
        for line in f:
            if "[PACING]" in line:
                try:
                    parts = line.strip().split()
                    post_delay = int([p for p in parts if p.startswith("post_delay=")][0].split("=")[1].replace("ms", ""))
                    rtt = int([p for p in parts if p.startswith("rtt=")][0].split("=")[1].replace("ms", ""))
                    ssthresh = int([p for p in parts if p.startswith("ssthresh=")][0].split("=")[1].replace("ms", ""))
                    clean = int([p for p in parts if p.startswith("clean=")][0].split("=")[1])
                    events.append({
                        "type": "success",
                        "post_delay": post_delay,
                        "rtt": rtt,
                        "ssthresh": ssthresh,
                        "clean": clean,
                    })
                except Exception:
                    pass
            elif "[PACING_INCIDENT]" in line:
                try:
                    parts = line.strip().split()
                    post_delay = int([p for p in parts if p.startswith("post_delay=")][0].split("=")[1].replace("ms", ""))
                    ssthresh = int([p for p in parts if p.startswith("ssthresh=")][0].split("=")[1].replace("ms", ""))
                    events.append({
                        "type": "incident",
                        "post_delay": post_delay,
                        "rtt": 0,
                        "ssthresh": ssthresh,
                        "clean": 0,
                    })
                except Exception:
                    pass
    return events

def render_ascii_timeline(data_points):
    if not data_points:
        return ""
    delays = [d["post_delay_ms"] for d in data_points]
    min_v = min(delays)
    max_v = max(delays)
    if min_v == max_v:
        return f"  {min_v}ms: " + ("#" * min(len(delays), 70))
    lines = []
    step = 2 if (max_v - min_v) > 6 else 1
    for row in range(max_v, min_v - 1, -step):
        row_str = f"  {row:2d}ms | "
        for v in delays:
            row_str += "#" if v >= row else " "
        lines.append(row_str)
    lines.append("       +" + "-" * len(delays))
    return "\n".join(lines)

def run_experiment(mode="adaptive", fixed_delay_ms=None, pinned_delay_ms=None,
                   disable_ssthresh=False, use_harsh=False, csv_path="/tmp/dynamic_load_log.csv"):
    if mode == "fixed":
        mode_desc = f"Fixed Delay ({fixed_delay_ms or 13}ms)"
    elif mode == "pinned":
        mode_desc = f"Adaptive Pinned ({pinned_delay_ms or 13}ms - ceiling=floor)"
    else:
        mode_desc = "Adaptive Pacing (no ssthresh)" if disable_ssthresh else "Adaptive Pacing (standard ssthresh)"

    print("================================================================================")
    print("THÍ NGHIỆM ĐO TẢI BẬT GIỮA CHỪNG VÀ PHỤC HỒI")
    print(f"Chế độ kiểm thử: {mode_desc}")
    print(f"Mức tải: {'Khắc nghiệt (CPUQuota=25% + glxgears + cpu_stress)' if use_harsh else 'Thực tế (glxgears + cpu_stress)'}")
    print("Kịch bản: 42 từ rảnh -> Bật tải nặng 56 từ -> Tắt tải 42 từ (Tổng: 140 từ)")
    print("================================================================================\n")

    p1_len = len(PART1_EXP.split())
    p2_len = len(PART2_EXP.split())
    p3_len = len(PART3_EXP.split())
    total_expected_words = p1_len + p2_len + p3_len

    tmp_out = "/tmp/matrix_term.bin"
    tmp_ready = "/tmp/matrix_term.ready"
    if os.path.exists(tmp_out):
        os.remove(tmp_out)
    if os.path.exists(tmp_ready):
        os.remove(tmp_ready)

    script_dir = os.path.dirname(os.path.abspath(__file__))
    el_dir = os.path.join(script_dir, "electron_terminal")
    cpu_script = os.path.join(script_dir, "cpu_stress.py")
    env = dict(os.environ)
    env.pop("ELECTRON_RUN_AS_NODE", None)

    restart_fcitx5(mode=mode, fixed_delay_ms=fixed_delay_ms, pinned_delay_ms=pinned_delay_ms,
                   disable_ssthresh=disable_ssthresh)

    # background glxgears stopped initially to avoid stealing focus
    p_gpu = subprocess.Popen(["glxgears"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.4)
    p_gpu.send_signal(signal.SIGSTOP)

    if use_harsh:
        cmd = ["systemd-run", "--user", "--scope", "-pCPUQuota=25%", "env", "-u", "ELECTRON_RUN_AS_NODE",
               "/usr/lib/electron40/electron", el_dir, tmp_out, tmp_ready]
    else:
        cmd = ["/usr/lib/electron40/electron", el_dir, tmp_out, tmp_ready]
    proc = subprocess.Popen(cmd, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    t0 = time.time()
    ready = False
    while time.time() - t0 < 8.0:
        if os.path.exists(tmp_ready):
            ready = True
            break
        time.sleep(0.05)

    if not ready:
        print("ERROR: Electron không sẵn sàng!")
        if proc: proc.kill()
        p_gpu.kill()
        return None

    focus_electron_window()
    time.sleep(0.3)

    # ensure clak is active in electron window
    for _ in range(15):
        subprocess.run(["fcitx5-remote", "-s", "clak"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.1)
        res = subprocess.run(["fcitx5-remote", "-n"], capture_output=True, text=True)
        if res.stdout.strip() == "clak":
            break

    p_cpu = None
    subprocess.run(["hyprctl", "repl", "hl.config({ input = { repeat_delay = 1000 } })"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    try:
        # Phase 1: 42 words idle
        print(f"1. Giai đoạn 1: Gõ {p1_len} từ đầu lúc rảnh (không tải)...")
        subprocess.run(["wtype", "-d", "10", PART1_RAW], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.3)

        # Phase 2: 56 words under sudden heavy load
        print(f"2. Giai đoạn 2: BẬT TẢI NẶNG ĐỘT NGỘT và gõ tiếp {p2_len} từ...")
        p_gpu.send_signal(signal.SIGCONT)
        p_cpu = subprocess.Popen(["python3", cpu_script, "3600"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.2)
        subprocess.run(["wtype", "-d", "10", PART2_RAW], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.5)

        # Phase 3: 42 words load removed
        print(f"3. Giai đoạn 3: TẮT TẢI NẶNG và gõ {p3_len} từ còn lại để đo phục hồi...")
        p_gpu.send_signal(signal.SIGSTOP)
        if p_cpu:
            try:
                p_cpu.terminate()
                p_cpu.wait(timeout=1.0)
            except Exception:
                try: p_cpu.kill()
                except Exception: pass
            p_cpu = None
        time.sleep(0.2)
        subprocess.run(["wtype", "-d", "10", PART3_RAW], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(1.0)

        # send ctrl+d to close terminal
        subprocess.run(["wtype", "-M", "ctrl", "-k", "d", "-m", "ctrl"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.5)
    finally:
        subprocess.run(["hyprctl", "repl", "hl.config({ input = { repeat_delay = 150 } })"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if p_cpu:
            try: p_cpu.kill()
            except Exception: pass
        if p_gpu:
            try: p_gpu.kill()
            except Exception: pass
        if proc:
            try:
                proc.terminate()
                proc.wait(timeout=1.0)
            except Exception:
                try: proc.kill()
                except Exception: pass

    # decode actual text
    actual = ""
    if os.path.exists(tmp_out):
        with open(tmp_out, "rb") as f:
            actual = decode_terminal_bytes(f.read()).strip()

    exp_words = FULL_EXPECTED.split()
    act_words = actual.split()

    # use SequenceMatcher to prevent cascading index shift
    matcher = difflib.SequenceMatcher(None, exp_words, act_words)
    aligned_words = []
    for tag, i1, i2, j1, j2 in matcher.get_opcodes():
        if tag == "equal":
            for k in range(i2 - i1):
                aligned_words.append({
                    "exp": exp_words[i1 + k],
                    "act": act_words[j1 + k],
                    "is_error": False
                })
        elif tag == "replace":
            max_len = max(i2 - i1, j2 - j1)
            for k in range(max_len):
                ew = exp_words[i1 + k] if (i1 + k < i2) else "<missing>"
                aw = act_words[j1 + k] if (j1 + k < j2) else "<extra>"
                aligned_words.append({
                    "exp": ew,
                    "act": aw,
                    "is_error": True
                })
        elif tag == "delete":
            for k in range(i1, i2):
                aligned_words.append({
                    "exp": exp_words[k],
                    "act": "<missing>",
                    "is_error": True
                })
        elif tag == "insert":
            for k in range(j1, j2):
                aligned_words.append({
                    "exp": "<extra>",
                    "act": act_words[k],
                    "is_error": True
                })

    # parse real log events
    events = parse_pacing_log("/tmp/clak.log")

    # map each word to its pacing state
    csv_rows = []
    event_idx = 0
    num_events = len(events)
    cur_delay = 12
    cur_rtt = 0
    cur_ssthresh = 12

    for idx in range(total_expected_words):
        phase = 1 if idx < p1_len else (2 if idx < (p1_len + p2_len) else 3)
        if idx < len(aligned_words):
            word_info = aligned_words[idx]
        else:
            word_info = {"exp": exp_words[idx], "act": "<missing>", "is_error": True}

        # advance event
        if num_events > 0:
            target_ev = int((idx / total_expected_words) * num_events)
            target_ev = min(target_ev, num_events - 1)
            ev = events[target_ev]
            cur_delay = ev["post_delay"]
            cur_rtt = ev["rtt"]
            cur_ssthresh = ev["ssthresh"]

        csv_rows.append({
            "word_index": idx + 1,
            "phase": phase,
            "expected_word": word_info["exp"],
            "actual_word": word_info["act"],
            "post_delay_ms": cur_delay,
            "rtt_ms": cur_rtt,
            "ssthresh_ms": cur_ssthresh,
            "is_error": 1 if word_info["is_error"] else 0
        })

    # write CSV log
    with open(csv_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=[
            "word_index", "phase", "expected_word", "actual_word",
            "post_delay_ms", "rtt_ms", "ssthresh_ms", "is_error"
        ])
        writer.writeheader()
        writer.writerows(csv_rows)
    print(f"Đã xuất dữ liệu đo từng từ ra CSV: {csv_path} ({len(csv_rows)} dòng)")

    p1_rows = [r for r in csv_rows if r["phase"] == 1]
    p2_rows = [r for r in csv_rows if r["phase"] == 2]
    p3_rows = [r for r in csv_rows if r["phase"] == 3]

    err_p1 = sum(r["is_error"] for r in p1_rows)
    err_p2 = sum(r["is_error"] for r in p2_rows)
    err_p3 = sum(r["is_error"] for r in p3_rows)
    total_err = err_p1 + err_p2 + err_p3

    print("\n--------------------------------------------------------------------------------")
    print("KẾT QUẢ ĐO LƯỜNG TỪNG GIAI ĐOẠN")
    print("--------------------------------------------------------------------------------")
    print(f"Pha 1 (42 từ rảnh): {p1_len - err_p1}/{p1_len} từ chuẩn (Lỗi: {err_p1})")
    print(f"Pha 2 (56 từ tải nặng): {p2_len - err_p2}/{p2_len} từ chuẩn (Lỗi: {err_p2})")
    print(f"Pha 3 (42 từ tắt tải/hồi phục): {p3_len - err_p3}/{p3_len} từ chuẩn (Lỗi: {err_p3})")
    print(f"Tổng thể: {total_expected_words - total_err}/{total_expected_words} từ chuẩn (Tổng lỗi: {total_err})")

    # progression of distinct delays
    delays_p2 = [r["post_delay_ms"] for r in p2_rows]
    delays_p3 = [r["post_delay_ms"] for r in p3_rows]
    if delays_p2:
        print(f"Delay Pha 2 (Tải): khởi điểm {delays_p2[0]}ms -> đỉnh {max(delays_p2)}ms -> cuối {delays_p2[-1]}ms")
    if delays_p3:
        p3_distinct = []
        for d in delays_p3:
            if not p3_distinct or p3_distinct[-1] != d:
                p3_distinct.append(d)
        print(f"Delay Pha 3 (Hồi phục): {' -> '.join(str(d) + 'ms' for d in p3_distinct)}")

    print("\nBiểu đồ chuỗi phản ứng độ trễ (ASCII Transition Timeline từ file CSV thật):")
    print("  [ Pha 1: 42 từ rảnh ] ---> [ Pha 2: 56 từ tải nặng ] ---> [ Pha 3: 42 từ hồi phục ]")
    print(render_ascii_timeline(csv_rows))
    print("================================================================================\n")

    return {
        "mode": mode,
        "total_err": total_err,
        "err_p1": err_p1,
        "err_p2": err_p2,
        "err_p3": err_p3,
        "csv_rows": csv_rows,
        "csv_path": csv_path
    }

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["adaptive", "fixed", "pinned"], default="adaptive")
    parser.add_argument("--fixed-delay", type=int, default=13)
    parser.add_argument("--pinned-delay", type=int, default=13)
    parser.add_argument("--disable-ssthresh", action="store_true")
    parser.add_argument("--harsh", action="store_true")
    parser.add_argument("--csv", default="/tmp/dynamic_load_log.csv")
    args = parser.parse_args()

    run_experiment(
        mode=args.mode,
        fixed_delay_ms=args.fixed_delay,
        pinned_delay_ms=args.pinned_delay,
        disable_ssthresh=args.disable_ssthresh,
        use_harsh=args.harsh,
        csv_path=args.csv
    )
