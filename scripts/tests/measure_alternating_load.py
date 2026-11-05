#!/usr/bin/env python3
import os
import sys
import time
import json
import signal
import subprocess

# 4 phases x 35 words = 140 words
P1_RAW = "ddaay buoori sangs treen ddirnh nuis cao mang laij mootj carm giacs thaatj suwj thanh tharn vaf tuyeetj vowfi. nhuwxng tia nawngs vowis khoong gian roojng lowsn chieeus roji xuoongs khawps cais "
P1_EXP = "đây buổi sáng trên đỉnh núi cao mang lại một cảm giác thật sự thanh thản và tuyệt vời. những tia nắng với không gian rộng lớn chiếu rọi xuống khắp cái "

P2_RAW = "thung lungx, noowi cos nhuwxng tharm cor xanh mown mowrn vaf dofng suoosi nhor luowjn uoosn quanh qua tuwfng khe ddas. con nguwowfi noowi ddaay luoon chawm chir, kieen trif gawsn bos vowis ruoojng "
P2_EXP = "thung lũng, nơi có những thảm cỏ xanh mơn mởn và dòng suối nhỏ lượn uốn quanh qua từng khe đá. con người nơi đây luôn chăm chỉ, kiên trì gắn bó với ruộng "

P3_RAW = "ddoofng, giuwx gifn truyeefn thoongs vawn hoas ddaamj ddaf barn sawsc cuar toor tieen. booj gox tieengs vieetj clak hoatj ddoojng voo cufng muowjt maf, pharn hoofi nhanh chongs vaf chinhs xacs treen cais "
P3_EXP = "đồng, giữ gìn truyền thống văn hóa đậm đà bản sắc của tổ tiên. bộ gõ tiếng việt clak hoạt động vô cùng mượt mà, phản hồi nhanh chóng và chính xác trên cái "

P4_RAW = "mooi truwowfng terminal cuar heej ddieefu hanhf linux. duf laf vieest mootj ddoanj vawn hay laapj trinhf phaafn meefm phuwsc tapj, trari nghieemj gox phims luoon muowjt maf cho nguwowif suwr dujng."
P4_EXP = "môi trường terminal của hệ điều hành linux. dù là viết một đoạn văn hay lập trình phần mềm phức tạp, trải nghiệm gõ phím luôn mượt mà cho người sử dụng."

FULL_EXPECTED = (P1_EXP + P2_EXP + P3_EXP + P4_EXP).strip()

def clean_state_files():
    for p in ["/tmp/clak.log", "/tmp/matrix_term.bin", "/tmp/matrix_term.ready"]:
        if os.path.exists(p):
            try: os.remove(p)
            except OSError: pass

def restart_fcitx5(fixed_delay_ms=None):
    clean_state_files()
    env = dict(os.environ)
    if fixed_delay_ms is not None:
        env["CLAK_FIXED_DELAY_MS"] = str(fixed_delay_ms)
        env["CLAK_FIXED_GAP_MS"] = "3"
    else:
        env.pop("CLAK_FIXED_DELAY_MS", None)
        env.pop("CLAK_FIXED_GAP_MS", None)
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
            if chars: chars.pop()
            i += 1
        elif b in (0x0a, 0x0d):
            i += 1
        else:
            clen = 1
            if b >= 0xf0: clen = 4
            elif b >= 0xe0: clen = 3
            elif b >= 0xc0: clen = 2
            try:
                chars.append(bytes(raw_bytes[i:i+clen]).decode("utf-8"))
            except Exception: pass
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
        except Exception: pass
        time.sleep(0.05)
    return False

def parse_pacing_events(clak_log, start_offset):
    events = []
    if not os.path.exists(clak_log):
        return events
    with open(clak_log, "r", encoding="utf-8", errors="ignore") as f:
        f.seek(start_offset)
        for line in f:
            if "[PACING]" in line:
                try:
                    parts = line.strip().split()
                    post_delay = int([p for p in parts if p.startswith("post_delay=")][0].split("=")[1].replace("ms", ""))
                    rtt = int([p for p in parts if p.startswith("rtt=")][0].split("=")[1].replace("ms", ""))
                    ssthresh = int([p for p in parts if p.startswith("ssthresh=")][0].split("=")[1].replace("ms", ""))
                    clean = int([p for p in parts if p.startswith("clean=")][0].split("=")[1])
                    events.append({
                        "post_delay": post_delay,
                        "rtt": rtt,
                        "ssthresh": ssthresh,
                        "clean": clean,
                    })
                except Exception: pass
            elif "uinput waiting for sentinel:" in line and "post_delay=" in line:
                try:
                    parts = line.split("post_delay=")[1].split()
                    delay = int(parts[0].replace("ms", ""))
                    events.append({
                        "post_delay": delay,
                        "rtt": 0,
                        "ssthresh": 0,
                        "clean": 0,
                    })
                except Exception: pass
    return events

def run_alternating_experiment(use_harsh=False, fixed_delay_ms=None):
    mode_str = f"Fixed Delay = {fixed_delay_ms}ms" if fixed_delay_ms is not None else "Adaptive Pacing (ssthresh)"
    print("================================================================================")
    print("THÍ NGHIỆM TẢI BẬT/TẮT LUÂN PHIÊN (ALTERNATING LOAD SPIKES: IDLE -> LOAD -> IDLE -> LOAD)")
    print(f"Chế độ động cơ: {mode_str}")
    print(f"Mức tải: {'Khắc nghiệt (CPUQuota=25% + glxgears + cpu_stress)' if use_harsh else 'Thực tế (glxgears + cpu_stress)'}")
    print("Mục tiêu: Kiểm tra xem ssthresh có bảo vệ độ trễ không bị tụt quá sâu khi tải quay lại không")
    print("================================================================================\n")

    tmp_out = "/tmp/matrix_term.bin"
    tmp_ready = "/tmp/matrix_term.ready"
    if os.path.exists(tmp_out): os.remove(tmp_out)
    if os.path.exists(tmp_ready): os.remove(tmp_ready)

    script_dir = os.path.dirname(os.path.abspath(__file__))
    el_dir = os.path.join(script_dir, "electron_terminal")
    cpu_script = os.path.join(script_dir, "cpu_stress.py")
    env = dict(os.environ)
    env.pop("ELECTRON_RUN_AS_NODE", None)

    restart_fcitx5(fixed_delay_ms=fixed_delay_ms)

    # Pre-spawn glxgears in background and immediately pause it
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
        return

    focus_electron_window()
    time.sleep(0.5)

    clak_log = "/tmp/clak.log"
    p_cpu = None
    subprocess.run(["hyprctl", "repl", "hl.config({ input = { repeat_delay = 1000 } })"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    offsets = []
    try:
        # Pha 1: Ranh
        print("Pha 1 (Rảnh): Gõ phần 1 không tải...")
        offsets.append(os.path.getsize(clak_log) if os.path.exists(clak_log) else 0)
        subprocess.run(["wtype", "-d", "10", P1_RAW], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.3)

        # Pha 2: Bat tai 1
        print("Pha 2 (Tải nặng #1): BẬT TẢI NẶNG (glxgears + cpu_stress) và gõ phần 2...")
        offsets.append(os.path.getsize(clak_log) if os.path.exists(clak_log) else 0)
        p_gpu.send_signal(signal.SIGCONT)
        p_cpu = subprocess.Popen(["python3", cpu_script, "3600"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.2)
        subprocess.run(["wtype", "-d", "10", P2_RAW], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.5)

        # Pha 3: Tat tai (Hoi phuc)
        print("Pha 3 (Rảnh trở lại): TẮT TẢI, gõ phần 3 để fast decay co delay xuống...")
        offsets.append(os.path.getsize(clak_log) if os.path.exists(clak_log) else 0)
        p_gpu.send_signal(signal.SIGSTOP)
        if p_cpu:
            try: p_cpu.terminate(); p_cpu.wait(timeout=1.0)
            except Exception:
                try: p_cpu.kill()
                except Exception: pass
            p_cpu = None
        time.sleep(0.2)
        subprocess.run(["wtype", "-d", "10", P3_RAW], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.3)

        # Pha 4: Bat tai tro lai (Kiem tra ssthresh bao ve)
        print("Pha 4 (Tải nặng quay lại #2): BẬT TẢI NẶNG TRỞ LẠI và gõ phần 4...")
        offsets.append(os.path.getsize(clak_log) if os.path.exists(clak_log) else 0)
        p_gpu.send_signal(signal.SIGCONT)
        p_cpu = subprocess.Popen(["python3", cpu_script, "3600"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.2)
        subprocess.run(["wtype", "-d", "10", P4_RAW], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.5)

        # Ket thuc
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
            try: proc.terminate(); proc.wait(timeout=1.0)
            except Exception:
                try: proc.kill()
                except Exception: pass

    actual = ""
    if os.path.exists(tmp_out):
        with open(tmp_out, "rb") as f:
            actual = decode_terminal_bytes(f.read()).strip()

    exp_words = FULL_EXPECTED.split()
    act_words = actual.split()

    p_lens = [len(P1_EXP.split()), len(P2_EXP.split()), len(P3_EXP.split()), len(P4_EXP.split())]
    errors = [[], [], [], []]

    for i in range(min(len(exp_words), len(act_words))):
        ew = exp_words[i]
        aw = act_words[i]
        if ew != aw:
            cum = 0
            for p_idx, l in enumerate(p_lens):
                if i < cum + l:
                    errors[p_idx].append((i - cum, ew, aw))
                    break
                cum += l

    p_delays = []
    for off in offsets:
        p_delays.append(parse_pacing_events(clak_log, off))

    print("\n--------------------------------------------------------------------------------")
    print("KẾT QUẢ THEO 4 PHA LUÂN PHIÊN")
    print("--------------------------------------------------------------------------------")
    phases = ["Pha 1 (Rảnh)", "Pha 2 (Tải nặng #1)", "Pha 3 (Rảnh phục hồi)", "Pha 4 (Tải nặng #2)"]
    for idx, name in enumerate(phases):
        l = p_lens[idx]
        errs = errors[idx]
        delays = p_delays[idx]
        print(f"{name} ({l} từ):")
        print(f"  - Khớp: {l - len(errs)}/{l} (Lỗi: {len(errs)})")
        if errs:
            for w_i, ew, aw in errs[:5]:
                print(f"      + Lỗi từ #{w_i}: '{ew}' != '{aw}'")
            if len(errs) > 5:
                print(f"      + ... và {len(errs) - 5} từ lỗi khác")
        if delays:
            d_vals = [d["post_delay"] for d in delays]
            s_vals = [d["ssthresh"] for d in delays if d["ssthresh"] > 0]
            s_str = f", ssthresh={s_vals[-1]}ms" if s_vals else ""
            print(f"  - Delay: ban đầu {d_vals[0]}ms -> đỉnh {max(d_vals)}ms -> kết thúc {d_vals[-1]}ms{s_str}")

    all_d = []
    for d in p_delays:
        all_d.extend(d)
    distinct = []
    for d in all_d:
        v = d["post_delay"]
        if not distinct or distinct[-1] != v:
            distinct.append(v)
    print(f"\nChuỗi bậc thang toàn bộ 4 pha: {' -> '.join(str(v) + 'ms' for v in distinct)}")

    total_errs = sum(len(e) for e in errors)
    print("\n================================================================================")
    print("TỔNG KẾT THÍ NGHIỆM LUÂN PHIÊN")
    print("================================================================================")
    print(f"Trạng thái: {'SẠCH LỖI 100%' if total_errs == 0 else f'Có {total_errs} từ lỗi'}")
    print("================================================================================\n")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--harsh", action="store_true", help="Apply systemd CPUQuota=25%% to electron")
    parser.add_argument("--fixed-delay", type=int, default=None, help="Lock fixed delay in ms for baseline control")
    args = parser.parse_args()
    run_alternating_experiment(use_harsh=args.harsh, fixed_delay_ms=args.fixed_delay)
