#!/usr/bin/env python3
import os
import sys
import time
import json
import signal
import subprocess

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
        "/tmp/matrix_term.ready",
    ]:
        if os.path.exists(p):
            try:
                os.remove(p)
            except OSError:
                pass

def restart_fcitx5():
    clean_state_files()
    env = dict(os.environ)
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

def parse_pacing_events(clak_log, start_offset):
    events = []
    if not os.path.exists(clak_log):
        return events
    with open(clak_log, "r", encoding="utf-8", errors="ignore") as f:
        f.seek(start_offset)
        for line in f:
            if "uinput waiting for sentinel:" in line and "post_delay=" in line:
                try:
                    parts = line.split("post_delay=")[1].split()
                    delay = int(parts[0].replace("ms", ""))
                    events.append((line[:14], delay))
                except Exception:
                    pass
    return events

def run_experiment(use_harsh=False):
    print("================================================================================")
    print("THÍ NGHIỆM ĐO TẢI BẬT GIỮA CHỪNG (DYNAMIC LOAD SPIKE) VÀ PHỤC HỒI (FAST DECAY)")
    print(f"Chế độ: {'Khắc nghiệt (CPUQuota=25% + glxgears + cpu_stress)' if use_harsh else 'Thực tế (Background build: glxgears + cpu_stress)'}")
    print("Kịch bản: Đang gõ lúc rảnh -> Đột ngột bật tải nặng -> Tắt tải & đo phục hồi")
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

    restart_fcitx5()

    # Pre-spawn glxgears in background and immediately pause it so it never steals focus
    p_gpu = subprocess.Popen(["glxgears"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.4)
    p_gpu.send_signal(signal.SIGSTOP)

    # Launch Electron
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
    offset_p1 = os.path.getsize(clak_log) if os.path.exists(clak_log) else 0

    p_cpu = None
    subprocess.run(["hyprctl", "repl", "hl.config({ input = { repeat_delay = 1000 } })"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    try:
        # Giai doan 1: Go 35 tu dau luc ranh (khong tai)
        print("1. Giai đoạn 1: Gõ 35 từ đầu lúc máy rảnh (không tải)...")
        subprocess.run(["wtype", "-d", "10", PART1_RAW], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.2)
        offset_p2 = os.path.getsize(clak_log) if os.path.exists(clak_log) else 0

        # Giai doan 2: Dot ngot bat tai nang (cpu_stress + resume glxgears)
        print("2. Giai đoạn 2: BẬT TẢI NẶNG ĐỘT NGỘT (glxgears + cpu_stress) và gõ tiếp 65 từ...")
        p_gpu.send_signal(signal.SIGCONT)
        p_cpu = subprocess.Popen(["python3", cpu_script, "3600"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.2)

        subprocess.run(["wtype", "-d", "10", PART2_RAW], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.5)
        offset_p3 = os.path.getsize(clak_log) if os.path.exists(clak_log) else 0

        # Giai doan 3: Tat tai nang (pause glxgears + kill cpu_stress), do phuc hoi
        print("3. Giai đoạn 3: TẮT TẢI NẶNG và gõ 40 từ còn lại để đo tốc độ co về sàn (fast decay)...")
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
            try:
                proc.terminate()
                proc.wait(timeout=1.0)
            except Exception:
                try: proc.kill()
                except Exception: pass

    # Doc du lieu go thuc te
    actual = ""
    if os.path.exists(tmp_out):
        with open(tmp_out, "rb") as f:
            actual = decode_terminal_bytes(f.read()).strip()

    exp_words = FULL_EXPECTED.split()
    act_words = actual.split()

    p1_words = PART1_EXP.split()
    p2_words = PART2_EXP.split()
    p3_words = PART3_EXP.split()

    p1_len = len(p1_words)
    p2_len = len(p2_words)
    p3_len = len(p3_words)

    errors_p1 = []
    errors_p2 = []
    errors_p3 = []

    for i in range(min(len(exp_words), len(act_words))):
        ew = exp_words[i]
        aw = act_words[i]
        if ew != aw:
            if i < p1_len:
                errors_p1.append((i, ew, aw))
            elif i < p1_len + p2_len:
                errors_p2.append((i - p1_len, ew, aw))
            else:
                errors_p3.append((i - p1_len - p2_len, ew, aw))

    delays_p1 = parse_pacing_events(clak_log, offset_p1)
    delays_p2 = parse_pacing_events(clak_log, offset_p2)
    delays_p3 = parse_pacing_events(clak_log, offset_p3)

    print("\n--------------------------------------------------------------------------------")
    print("KẾT QUẢ PHÂN TÍCH THEO TỪNG GIAI ĐOẠN")
    print("--------------------------------------------------------------------------------")

    print(f"Giai đoạn 1 (35 từ lúc rảnh - không tải):")
    print(f"  - Số từ khớp: {p1_len - len(errors_p1)}/{p1_len} (Lỗi: {len(errors_p1)})")
    if errors_p1:
        for idx, ew, aw in errors_p1:
            print(f"      + Lỗi từ #{idx}: '{ew}' != '{aw}'")
    if delays_p1:
        d_vals = [d for _, d in delays_p1]
        print(f"  - Delay: ban đầu {d_vals[0]}ms, cuối {d_vals[-1]}ms (min={min(d_vals)}ms, max={max(d_vals)}ms)")

    print(f"\nGiai đoạn 2 (65 từ - BẬT TẢI NẶNG ĐỘT NGỘT):")
    print(f"  - Số từ khớp: {p2_len - len(errors_p2)}/{p2_len} (Lỗi: {len(errors_p2)})")
    if errors_p2:
        print("  - Chi tiết các từ lỗi trong quá trình chuyển tiếp sang tải nặng:")
        for idx, ew, aw in errors_p2:
            print(f"      + Từ thứ {idx} sau khi bật tải: mong đợi '{ew}', thực tế '{aw}'")
        first_err = errors_p2[0][0]
        last_err = errors_p2[-1][0]
        print(f"  - Vùng dao động chuyển tiếp: Từ thứ {first_err} đến từ thứ {last_err}")
        print(f"  - Số từ để adaptive pacing hội tụ: {last_err + 1} từ (từ thứ {last_err + 2} trở đi sạch 100%)")
    else:
        print("  - Xuất sắc: Không có từ nào bị lỗi! Adaptive pacing hấp thụ cú sốc tải tức thì!")

    if delays_p2:
        d_vals = [d for _, d in delays_p2]
        print(f"  - Phản ứng của Adaptive Engine: ban đầu {d_vals[0]}ms -> đỉnh cực đại {max(d_vals)}ms -> cuối pha tải {d_vals[-1]}ms")

    print(f"\nGiai đoạn 3 (40 từ - TẮT TẢI, PHỤC HỒI / FAST DECAY):")
    print(f"  - Số từ khớp: {p3_len - len(errors_p3)}/{p3_len} (Lỗi: {len(errors_p3)})")
    if errors_p3:
        for idx, ew, aw in errors_p3:
            print(f"      + Lỗi từ #{idx}: '{ew}' != '{aw}'")
    else:
        print("  - Sạch lỗi 100% trong toàn bộ giai đoạn co về sàn!")

    if delays_p3:
        d_vals = [d for _, d in delays_p3]
        print(f"  - Tiến trình co về sàn: bắt đầu ở {d_vals[0]}ms -> kết thúc ở {d_vals[-1]}ms (sàn mục tiêu: 12ms)")
        # Show progression
        distinct_progression = []
        for v in d_vals:
            if not distinct_progression or distinct_progression[-1] != v:
                distinct_progression.append(v)
        print(f"  - Chuỗi bậc thang delay hạ xuống: {' -> '.join(str(v) + 'ms' for v in distinct_progression)}")

    print("\n================================================================================")
    print("TỔNG KẾT")
    print("================================================================================")
    print(f"Văn bản tổng thể: {'KHỚP TUYỆT ĐỐI 100%' if actual == FULL_EXPECTED else f'Lệch {len(errors_p1) + len(errors_p2) + len(errors_p3)} từ'}")
    if delays_p2:
        print(f"Độ trễ thích ứng cực đại dưới tải: {max(d for _, d in delays_p2)}ms")
    if delays_p3:
        print(f"Độ trễ sau khi phục hồi 40 từ rảnh: {delays_p3[-1][1]}ms")
    print("================================================================================\n")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--harsh", action="store_true", help="Apply systemd CPUQuota=25%% to electron")
    args = parser.parse_args()
    run_experiment(use_harsh=args.harsh)
