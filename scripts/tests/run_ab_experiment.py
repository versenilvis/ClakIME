#!/usr/bin/env python3
import os
import sys
import time
import json
import subprocess
import argparse
import difflib

RAW_TEXT = "ddaay buoori sangs treen ddirnh nuis cao mang laij mootj carm giacs thaatj suwj thanh tharn vaf tuyeetj vowfi. nhuwxng tia nawngs vowis khoong gian roojng lowsn chieeus roji xuoongs khawps cais thung lungx, noowi cos nhuwxng tharm cor xanh mown mowrn vaf dofng suoosi nhor luowjn uoosn quanh qua tuwfng khe ddas. con nguwowfi noowi ddaay luoon chawm chir, kieen trif gawsn bos vowis ruoojng ddoofng, giuwx gifn truyeefn thoongs vawn hoas ddaamj ddaf barn sawsc cuar toor tieen. booj gox tieengs vieetj clak hoatj ddoojng voo cufng muowjt maf, pharn hoofi nhanh chongs vaf chinhs xacs treen cais mooi truwowfng terminal cuar heej ddieefu hanhf linux. duf laf vieest mootj ddoanj vawn hay laapj trinhf phaafn meefm phuwsc tapj, trari nghieemj gox phims luoon muowjt maf vaf ddem laij suwj thoari mais tuyeetj ddoosi cho nguwowif suwr dujng."

EXPECTED_TEXT = "đây buổi sáng trên đỉnh núi cao mang lại một cảm giác thật sự thanh thản và tuyệt vời. những tia nắng với không gian rộng lớn chiếu rọi xuống khắp cái thung lũng, nơi có những thảm cỏ xanh mơn mởn và dòng suối nhỏ lượn uốn quanh qua từng khe đá. con người nơi đây luôn chăm chỉ, kiên trì gắn bó với ruộng đồng, giữ gìn truyền thống văn hóa đậm đà bản sắc của tổ tiên. bộ gõ tiếng việt clak hoạt động vô cùng mượt mà, phản hồi nhanh chóng và chính xác trên cái môi trường terminal của hệ điều hành linux. dù là viết một đoạn văn hay lập trình phần mềm phức tạp, trải nghiệm gõ phím luôn mượt mà và đem lại sự thoải mái tuyệt đối cho người sử dụng."

MODE_CONFIGS = {
    "adaptive": {"CLAK_UINPUT_HOLD_US": "1000"},
    "adaptive_no_ssthresh": {"CLAK_DISABLE_SSTHRESH": "1", "CLAK_UINPUT_HOLD_US": "1000"},
    "fixed_10": {"CLAK_FIXED_DELAY_MS": "10", "CLAK_FIXED_GAP_MS": "3", "CLAK_UINPUT_HOLD_US": "1000"},
    "fixed_13": {"CLAK_FIXED_DELAY_MS": "13", "CLAK_FIXED_GAP_MS": "3", "CLAK_UINPUT_HOLD_US": "1000"},
    "fixed_16": {"CLAK_FIXED_DELAY_MS": "16", "CLAK_FIXED_GAP_MS": "3", "CLAK_UINPUT_HOLD_US": "1000"},
    "fixed_20": {"CLAK_FIXED_DELAY_MS": "20", "CLAK_FIXED_GAP_MS": "3", "CLAK_UINPUT_HOLD_US": "1000"},
    "fixed_30": {"CLAK_FIXED_DELAY_MS": "30", "CLAK_FIXED_GAP_MS": "3", "CLAK_UINPUT_HOLD_US": "1000"},
}

def restart_fcitx5_for_mode(mode):
    env = dict(os.environ)
    env.pop("CLAK_FIXED_DELAY_MS", None)
    env.pop("CLAK_FIXED_GAP_MS", None)
    env.pop("CLAK_PINNED_DELAY_MS", None)
    env.pop("CLAK_DISABLE_SSTHRESH", None)
    env.pop("CLAK_UINPUT_HOLD_US", None)

    for k, v in MODE_CONFIGS[mode].items():
        env[k] = v

    subprocess.run(["fcitx5", "-r", "-d"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(1.0)
    for _ in range(30):
        res = subprocess.run(["fcitx5-remote"], capture_output=True, text=True)
        if res.returncode == 0:
            break
        time.sleep(0.1)
    subprocess.run(["fcitx5-remote", "-s", "clak"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.3)

def start_stress(load_type):
    procs = []
    script_dir = os.path.dirname(os.path.abspath(__file__))
    cpu_script = os.path.join(script_dir, "cpu_stress.py")
    if load_type in ("cpu", "cpu_gpu", "harsh"):
        p_cpu = subprocess.Popen(["python3", cpu_script, "3600"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        procs.append(p_cpu)
    if load_type in ("cpu_gpu", "harsh"):
        p_gpu = subprocess.Popen(["glxgears"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        procs.append(p_gpu)
    if procs:
        time.sleep(1.0)
    return procs

def stop_stress(procs):
    for p in procs:
        try:
            p.terminate()
            p.wait(timeout=1.0)
        except Exception:
            try:
                p.kill()
            except Exception:
                pass

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

def verify_preflight_and_prepare(proc, tmp_ready, max_wait_sec=15.0):
    t0 = time.time()
    ready = False
    while time.time() - t0 < max_wait_sec:
        if os.path.exists(tmp_ready):
            ready = True
            break
        time.sleep(0.05)

    if not ready:
        return False

    focus_electron_window()
    time.sleep(0.3)

    # ensure active IM is clak in focused window
    ime_ok = False
    for _ in range(20):
        subprocess.run(["fcitx5-remote", "-s", "clak"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.08)
        res = subprocess.run(["fcitx5-remote", "-n"], capture_output=True, text=True)
        if res.stdout.strip() == "clak":
            ime_ok = True
            break

    return ime_ok

def run_single_trial(mode, load_type, gap_ms, cpu_quota=25):
    tmp_out = "/tmp/matrix_term.bin"
    tmp_ready = "/tmp/matrix_term.ready"
    clak_log = "/tmp/clak.log"

    for attempt in range(3):
        for f in (tmp_out, tmp_ready, clak_log):
            if os.path.exists(f):
                try:
                    os.remove(f)
                except OSError:
                    pass

        script_dir = os.path.dirname(os.path.abspath(__file__))
        el_dir = os.path.join(script_dir, "electron_terminal")
        env = dict(os.environ)
        env.pop("ELECTRON_RUN_AS_NODE", None)

        if load_type == "harsh":
            cmd = ["systemd-run", "--user", "--scope", f"-pCPUQuota={cpu_quota}%", "env", "-u", "ELECTRON_RUN_AS_NODE", "/usr/lib/electron40/electron", el_dir, tmp_out, tmp_ready]
        else:
            cmd = ["/usr/lib/electron40/electron", el_dir, tmp_out, tmp_ready]

        proc = subprocess.Popen(cmd, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

        if not verify_preflight_and_prepare(proc, tmp_ready, max_wait_sec=15.0):
            try: proc.kill()
            except Exception: pass
            time.sleep(0.5)
            continue

        subprocess.run(["hyprctl", "repl", "hl.config({ input = { repeat_delay = 1000 } })"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            subprocess.run(["wtype", "-d", str(gap_ms), RAW_TEXT], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            time.sleep(0.8)
            subprocess.run(["wtype", "-M", "ctrl", "-k", "d", "-m", "ctrl"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            time.sleep(0.4)
        finally:
            subprocess.run(["hyprctl", "repl", "hl.config({ input = { repeat_delay = 150 } })"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

        if proc:
            try:
                proc.terminate()
                proc.wait(timeout=1.0)
            except Exception:
                try: proc.kill()
                except Exception: pass

        actual = ""
        raw_bytes = b""
        if os.path.exists(tmp_out):
            with open(tmp_out, "rb") as f:
                raw_bytes = f.read()
            actual = decode_terminal_bytes(raw_bytes).strip()

        exp_words = EXPECTED_TEXT.split()
        act_words = actual.split()

        matcher = difflib.SequenceMatcher(None, exp_words, act_words)
        word_errors = sum(max(i2 - i1, j2 - j1) for tag, i1, i2, j1, j2 in matcher.get_opcodes() if tag != "equal")
        is_clean = (actual == EXPECTED_TEXT)
        diff_str = ""
        if not is_clean:
            diff_parts = []
            for tag, i1, i2, j1, j2 in matcher.get_opcodes():
                if tag != "equal":
                    diff_parts.append(f"[{tag} exp='{' '.join(exp_words[i1:i2])}' act='{' '.join(act_words[j1:j2])}']")
            if not diff_parts and actual != EXPECTED_TEXT:
                diff_parts.append(f"formatting/whitespace diff (len exp={len(EXPECTED_TEXT)}, act={len(actual)})")
            diff_str = " | ".join(diff_parts)

        latencies = []
        if os.path.exists(clak_log):
            with open(clak_log, "r", encoding="utf-8", errors="ignore") as f:
                for line in f:
                    if "[LATENCY]" in line and "REPLACE" in line:
                        parts = line.split()
                        for p in parts:
                            if p.startswith("delta_ms="):
                                try:
                                    latencies.append(float(p.split("=")[1]))
                                except ValueError:
                                    pass

        p50, p95 = 0.0, 0.0
        if latencies:
            latencies.sort()
            n = len(latencies)
            p50 = latencies[int(0.50 * (n - 1))]
            p95 = latencies[int(0.95 * (n - 1))]

        return is_clean, word_errors, len(exp_words), p50, p95, diff_str

    return False, 140, 140, 0.0, 0.0, "preflight failed"

def main():
    parser = argparse.ArgumentParser(description="Strict A/B Experiment: Adaptive Pacing vs Fixed Delays")
    parser.add_argument("--runs", type=int, default=20, help="Runs per test cell (default: 20)")
    parser.add_argument("--gap", type=int, default=10, help="Typing gap in ms (default: 10)")
    parser.add_argument("--modes", nargs="+", default=["adaptive", "fixed_10", "fixed_13", "fixed_16", "fixed_20", "fixed_30"])
    parser.add_argument("--loads", nargs="+", default=["harsh"], help="Loads: idle, harsh")
    parser.add_argument("--cpu-quota", type=int, default=25, help="CPUQuota percentage for harsh load (default: 25)")
    parser.add_argument("--out-json", type=str, default="/tmp/clak_ab_strict_results.json")
    args = parser.parse_args()

    print("================================================================================")
    print("THÍ NGHIỆM ĐỐI CHỨNG A/B CHUẨN HÓA (STRICT PARAMETER PARITY)")
    print(f"Số lượt mỗi tổ hợp: {args.runs}")
    print(f"Tốc độ gõ: {args.gap}ms / phím (~100 phím/giây)")
    print(f"Chế độ Pacing: {', '.join(args.modes)}")
    print(f"Mức tải: {', '.join(args.loads)} (harsh = cpu_stress + glxgears + CPUQuota={args.cpu_quota}%)")
    print("Tham số cố định: gap=3ms, hold=1000us, pre_delay động đồng bộ 100%")
    print("================================================================================\n")

    summary_table = []

    for mode in args.modes:
        print(f"\n================================================================================")
        print(f">>> KHỞI ĐỘNG LẠI FCITX5 VỚI CHẾ ĐỘ: {mode.upper()}")
        print(f"================================================================================")
        restart_fcitx5_for_mode(mode)

        for load in args.loads:
            print(f"\n>>> Bắt đầu đo: Mode={mode.upper()} | Mức tải={load.upper()} ({args.runs} lượt)...")
            stress_procs = start_stress(load) if load == "harsh" else []

            clean_runs = 0
            total_word_errors = 0
            total_words = 0
            all_p50 = []
            all_p95 = []

            for run_i in range(1, args.runs + 1):
                is_clean, err_words, num_words, p50, p95, diff_str = run_single_trial(mode, load, args.gap, args.cpu_quota)
                if is_clean:
                    clean_runs += 1
                total_word_errors += err_words
                total_words += num_words
                if p50 > 0: all_p50.append(p50)
                if p95 > 0: all_p95.append(p95)

                status_mark = "✔ CLEAN" if is_clean else f"❌ {err_words} ERRORS"
                print(f"  [{mode}|{load}] Lượt {run_i:02d}/{args.runs}: {status_mark} (p50={p50:.1f}ms, p95={p95:.1f}ms)")
                if diff_str:
                    print(f"       -> Diff: {diff_str}")

            if stress_procs:
                stop_stress(stress_procs)

            mean_p50 = sum(all_p50) / len(all_p50) if all_p50 else 0.0
            mean_p95 = sum(all_p95) / len(all_p95) if all_p95 else 0.0
            pass_rate = (clean_runs / args.runs) * 100.0
            word_err_rate = (total_word_errors / total_words) * 100.0 if total_words > 0 else 0.0

            summary_table.append({
                "mode": mode,
                "load": load,
                "clean_runs": clean_runs,
                "total_runs": args.runs,
                "pass_rate": pass_rate,
                "word_errors": total_word_errors,
                "total_words": total_words,
                "word_err_rate": word_err_rate,
                "p50_ms": mean_p50,
                "p95_ms": mean_p95
            })

    print("\n================================================================================")
    print("BẢNG TỔNG HỢP KẾT QUẢ ĐỐI CHỨNG A/B CHUẨN HÓA (N=20 LƯỢT MỖI CHẾ ĐỘ)")
    print("================================================================================")
    print(f"{'Chế độ':<12} | {'Mức tải':<8} | {'Tỉ lệ sạch':<12} | {'Số từ lỗi':<14} | {'p50 (ms)':<10} | {'p95 (ms)':<10}")
    print("-" * 80)
    for row in summary_table:
        pass_str = f"{row['clean_runs']}/{row['total_runs']} ({row['pass_rate']:.1f}%)"
        err_str = f"{row['word_errors']}/{row['total_words']} ({row['word_err_rate']:.2f}%)"
        print(f"{row['mode']:<12} | {row['load']:<8} | {pass_str:<12} | {err_str:<14} | {row['p50_ms']:<10.2f} | {row['p95_ms']:<10.2f}")
    print("================================================================================\n")

    with open(args.out_json, "w", encoding="utf-8") as f:
        json.dump(summary_table, f, indent=2)
    print(f"Đã lưu kết quả chi tiết ra {args.out_json}")

if __name__ == "__main__":
    main()
