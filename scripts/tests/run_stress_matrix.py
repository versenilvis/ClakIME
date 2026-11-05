import os
import sys
import time
import json
import subprocess
import argparse

RAW_TEXT = "ddaay buoori sangs treen ddirnh nuis cao mang laij mootj carm giacs thaatj suwj thanh tharn vaf tuyeetj vowfi. nhuwxng tia nawngs vowis khoong gian roojng lowsn chieeus roji xuoongs khawps cais thung lungx, noowi cos nhuwxng tharm cor xanh mown mowrn vaf dofng suoosi nhor luowjn uoosn quanh qua tuwfng khe ddas. con nguwowfi noowi ddaay luoon chawm chir, kieen trif gawsn bos vowis ruoojng ddoofng, giuwx gifn truyeefn thoongs vawn hoas ddaamj ddaf barn sawsc cuar toor tieen. booj gox tieengs vieetj clak hoatj ddoojng voo cufng muowjt maf, pharn hoofi nhanh chongs vaf chinhs xacs treen cais mooi truwowfng terminal cuar heej ddieefu hanhf linux. duf laf vieest mootj ddoanj vawn hay laapj trinhf phaafn meefm phuwsc tapj, trari nghieemj gox phims luoon muowjt maf vaf ddem laij suwj thoari mais tuyeetj ddoosi cho nguwowif suwr dujng."

EXPECTED_TEXT = "đây buổi sáng trên đỉnh núi cao mang lại một cảm giác thật sự thanh thản và tuyệt vời. những tia nắng với không gian rộng lớn chiếu rọi xuống khắp cái thung lũng, nơi có những thảm cỏ xanh mơn mởn và dòng suối nhỏ lượn uốn quanh qua từng khe đá. con người nơi đây luôn chăm chỉ, kiên trì gắn bó với ruộng đồng, giữ gìn truyền thống văn hóa đậm đà bản sắc của tổ tiên. bộ gõ tiếng việt clak hoạt động vô cùng mượt mà, phản hồi nhanh chóng và chính xác trên cái môi trường terminal của hệ điều hành linux. dù là viết một đoạn văn hay lập trình phần mềm phức tạp, trải nghiệm gõ phím luôn mượt mà và đem lại sự thoải mái tuyệt đối cho người sử dụng."

def start_stress(load_type):
    procs = []
    script_dir = os.path.dirname(os.path.abspath(__file__))
    cpu_script = os.path.join(script_dir, "cpu_stress.py")
    if load_type in ("cpu", "cpu_gpu"):
        p_cpu = subprocess.Popen(["python3", cpu_script, "3600"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        procs.append(p_cpu)
    if load_type == "cpu_gpu":
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

def focus_terminal_window(term_type, window_title):
    lua = f'''
    for _, w in ipairs(hl.get_windows()) do
        local c = (w.class or ""):lower()
        local t = (w.title or ""):lower()
        if c:find("{term_type.lower()}", 1, true) or t:find("{window_title.lower()}", 1, true) or (c:find("python3", 1, true) and t:find("oracle", 1, true)) then
            hl.dispatch(hl.dsp.focus({{ window = w }}))
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
            if term_type.lower() in c or window_title.lower() in t:
                return True
        except Exception:
            pass
        time.sleep(0.05)
    return False

def run_single_trial(term_type, gap_ms, im_path="native"):
    tmp_out = "/tmp/matrix_term.bin"
    tmp_ready = "/tmp/matrix_term.ready"
    clak_log = "/tmp/clak.log"

    for f in (tmp_out, tmp_ready, clak_log):
        if os.path.exists(f):
            try:
                os.remove(f)
            except OSError:
                pass

    script_dir = os.path.dirname(os.path.abspath(__file__))
    receiver_script = os.path.join(script_dir, "raw_receiver.py")
    window_title = f"matrix_{term_type}_win"
    proc = None

    if term_type == "kitty":
        cmd = ["kitty", "--title", window_title, "python3", receiver_script, tmp_out, tmp_ready]
        proc = subprocess.Popen(cmd)
    elif term_type == "konsole":
        window_title = "konsole"
        cmd = ["konsole", "--separate", "-e", "python3", receiver_script, tmp_out, tmp_ready]
        proc = subprocess.Popen(cmd)
    elif term_type == "electron":
        el_dir = os.path.join(script_dir, "electron_terminal")
        env = dict(os.environ)
        env.pop("ELECTRON_RUN_AS_NODE", None)
        cmd = ["/usr/lib/electron40/electron", el_dir, tmp_out, tmp_ready]
        proc = subprocess.Popen(cmd, env=env)
        window_title = "clak_electron_xterm"
    elif term_type == "gtk":
        window_title = "clak_gtk_oracle"
        env = dict(os.environ)
        if im_path == "fcitx":
            env["GTK_IM_MODULE"] = "fcitx"
        else:
            env.pop("GTK_IM_MODULE", None)
        cmd = ["python3", os.path.join(script_dir, "gui_surrounding_oracle.py"), tmp_out, tmp_ready]
        proc = subprocess.Popen(cmd, env=env)

    # wait for ready file
    t0 = time.time()
    ready = False
    while time.time() - t0 < 5.0:
        if os.path.exists(tmp_ready):
            ready = True
            break
        time.sleep(0.05)

    if not ready:
        if proc:
            proc.kill()
        return False, "failed to start terminal ready marker", 0.0, 0.0, 0

    # focus window in hyprland and verify activewindow
    focus_terminal_window(term_type, window_title)
    time.sleep(0.4)

    subprocess.run(["fcitx5-remote", "-s", "clak"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    # emit text with temporary repeat_delay bump to prevent scheduler-induced repeat glitches
    subprocess.run(["hyprctl", "repl", "hl.config({ input = { repeat_delay = 1000 } })"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        subprocess.run(["wtype", "-d", str(gap_ms), RAW_TEXT], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(1.0)
        subprocess.run(["wtype", "-M", "ctrl", "-k", "d", "-m", "ctrl"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.5)
    finally:
        subprocess.run(["hyprctl", "repl", "hl.config({ input = { repeat_delay = 150 } })"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    if proc:
        try:
            proc.terminate()
            proc.wait(timeout=1.0)
        except Exception:
            try:
                proc.kill()
            except Exception:
                pass

    actual = ""
    if os.path.exists(tmp_out):
        with open(tmp_out, "rb") as f:
            raw_bytes = f.read()
        if term_type == "gtk":
            actual = raw_bytes.decode("utf-8", errors="ignore").strip()
        else:
            actual = decode_terminal_bytes(raw_bytes).strip()

    is_match = (actual == EXPECTED_TEXT)
    diff_desc = ""
    if not is_match:
        diff_desc = f"Mismatch! actual len={len(actual)}, expected len={len(EXPECTED_TEXT)}"
        fail_dump = f"/tmp/clak_fail_{term_type}_{int(time.time()*1000)}.bin"
        try:
            with open(fail_dump, "wb") as f:
                f.write(raw_bytes if 'raw_bytes' in locals() else b"")
            diff_desc += f"\n    (saved raw bytes to {fail_dump})"
        except Exception:
            pass
        exp_words = EXPECTED_TEXT.split()
        act_words = actual.split()
        mismatches = []
        for i, (ew, aw) in enumerate(zip(exp_words, act_words)):
            if ew != aw:
                mismatches.append(f"#{i}: exp '{ew}' != act '{aw}'")
        if len(exp_words) != len(act_words):
            mismatches.append(f"word count: exp={len(exp_words)} act={len(act_words)}")
        if mismatches:
            diff_desc += "\n    " + "\n    ".join(mismatches[:5])
        elif actual:
            diff_desc += f" (preview: {actual[:50]}...)"

    # parse latency and overflow from clak_log
    latencies = []
    overflow_count = 0
    if os.path.exists(clak_log):
        with open(clak_log, "r", encoding="utf-8", errors="ignore") as f:
            for line in f:
                if "key_buffer overflow" in line:
                    overflow_count += 1
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

    return is_match, diff_desc, p50, p95, overflow_count

def main():
    parser = argparse.ArgumentParser(description="Stress Test Matrix for Clak IME")
    parser.add_argument("--runs", type=int, default=10, help="Runs per combination (default: 10)")
    parser.add_argument("--gap", type=int, default=10, help="Typing gap in ms (default: 10)")
    parser.add_argument("--terminals", nargs="+", default=["kitty", "konsole", "electron"])
    parser.add_argument("--loads", nargs="+", default=["idle", "cpu", "cpu_gpu"])
    parser.add_argument("--im-paths", nargs="+", default=["native"], help="IM paths for GTK: native, fcitx (default: native)")
    args = parser.parse_args()

    print("================================================================================")
    print("CLAK IME STRESS TEST MATRIX (VĂN BẢN DÀI 140 TỪ)")
    print(f"Số lượt mỗi tổ hợp: {args.runs}")
    print(f"Tốc độ gõ: {args.gap}ms / phím (~100 phím/giây)")
    print(f"Terminals: {', '.join(args.terminals)}")
    print(f"Tải: {', '.join(args.loads)}")
    print(f"IM paths: {', '.join(args.im_paths)}")
    print("================================================================================\n")

    summary_results = []

    for term in args.terminals:
        im_modes = args.im_paths if term == "gtk" else ["native"]
        for im_p in im_modes:
            for load in args.loads:
                label = f"{term}[{im_p}]" if term == "gtk" else term
                print(f">>> Bắt đầu kiểm thử: Terminal={label.upper()} | Mức tải={load.upper()} ({args.runs} lượt)...")
                stress_procs = start_stress(load)
                success_count = 0
                total_overflows = 0
                p50_list = []
                p95_list = []

                for run_idx in range(1, args.runs + 1):
                    ok, diff, p50, p95, ovf = run_single_trial(term, args.gap, im_path=im_p)
                    if ok:
                        success_count += 1
                    else:
                        print(f"\n  [Lượt {run_idx}/{args.runs}] THẤT BẠI: {diff}")
                    total_overflows += ovf
                    if p50 > 0:
                        p50_list.append(p50)
                    if p95 > 0:
                        p95_list.append(p95)

                    sys.stdout.write(f"\r  Tiến độ: {run_idx}/{args.runs} - Thành công: {success_count}/{run_idx}")
                    sys.stdout.flush()

                stop_stress(stress_procs)
                print("")

                avg_p50 = sum(p50_list) / len(p50_list) if p50_list else 0.0
                avg_p95 = sum(p95_list) / len(p95_list) if p95_list else 0.0
                summary_results.append({
                    "terminal": label,
                    "load": load,
                    "total": args.runs,
                    "success": success_count,
                    "rate": (success_count / args.runs) * 100.0,
                    "overflows": total_overflows,
                    "p50": avg_p50,
                    "p95": avg_p95
                })
                time.sleep(0.5)

    print("\n================================================================================")
    print("TỔNG HỢP KẾT QUẢ MA TRẬN KIỂM THỬ (140 TỪ x 10 LƯỢT MỖI MỨC TẢI)")
    print("================================================================================")
    print(f"{'Terminal':<14} | {'Tải':<10} | {'Lượt':<6} | {'Chính xác':<12} | {'Overflows':<10} | {'p50 (ms)':<9} | {'p95 (ms)':<9}")
    print("-" * 80)
    for res in summary_results:
        acc_str = f"{res['success']}/{res['total']} ({res['rate']:.1f}%)"
        print(f"{res['terminal']:<14} | {res['load']:<10} | {res['total']:<6} | {acc_str:<12} | {res['overflows']:<10} | {res['p50']:<9.2f} | {res['p95']:<9.2f}")
    print("================================================================================\n")

if __name__ == "__main__":
    main()
