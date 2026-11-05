#!/usr/bin/env python3
import os
import sys
import time
import json
import subprocess

RAW_TEXT = "ddaay buoori sangs treen ddirnh nuis cao mang laij mootj carm giacs thaatj suwj thanh tharn vaf tuyeetj vowfi. nhuwxng tia nawngs vowis khoong gian roojng lowsn chieeus roji xuoongs khawps cais thung lungx, noowi cos nhuwxng tharm cor xanh mown mowrn vaf dofng suoosi nhor luowjn uoosn quanh qua tuwfng khe ddas. con nguwowfi noowi ddaay luoon chawm chir, kieen trif gawsn bos vowis ruoojng ddoofng, giuwx gifn truyeefn thoongs vawn hoas ddaamj ddaf barn sawsc cuar toor tieen. booj gox tieengs vieetj clak hoatj ddoojng voo cufng muowjt maf, pharn hoofi nhanh chongs vaf chinhs xacs treen cais mooi truwowfng terminal cuar heej ddieefu hanhf linux. duf laf vieest mootj ddoanj vawn hay laapj trinhf phaafn meefm phuwsc tapj, trari nghieemj gox phims luoon muowjt maf vaf ddem laij suwj thoari mais tuyeetj ddoosi cho nguwowif suwr dujng."

EXPECTED_TEXT = "đây buổi sáng trên đỉnh núi cao mang lại một cảm giác thật sự thanh thản và tuyệt vời. những tia nắng với không gian rộng lớn chiếu rọi xuống khắp cái thung lũng, nơi có những thảm cỏ xanh mơn mởn và dòng suối nhỏ lượn uốn quanh qua từng khe đá. con người nơi đây luôn chăm chỉ, kiên trì gắn bó với ruộng đồng, giữ gìn truyền thống văn hóa đậm đà bản sắc của tổ tiên. bộ gõ tiếng việt clak hoạt động vô cùng mượt mà, phản hồi nhanh chóng và chính xác trên cái môi trường terminal của hệ điều hành linux. dù là viết một đoạn văn hay lập trình phần mềm phức tạp, trải nghiệm gõ phím luôn mượt mà và đem lại sự thoải mái tuyệt đối cho người sử dụng."

def clean_state_files():
    for p in [
        os.path.expanduser("~/.local/state/clak/pacing.json"),
        "/tmp/clak_pacing_state.json",
        "/tmp/clak.log",
        "/tmp/matrix_term.bin",
        "/tmp/matrix_term.ready",
    ]:
        if os.path.exists(p):
            try:
                os.remove(p)
            except OSError:
                pass

def restart_fcitx5(preload_pacing=None):
    clean_state_files()
    if preload_pacing:
        state_dir = os.path.expanduser("~/.local/state/clak")
        os.makedirs(state_dir, exist_ok=True)
        with open(os.path.join(state_dir, "pacing.json"), "w", encoding="utf-8") as f:
            f.write(preload_pacing)

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

def start_stress():
    script_dir = os.path.dirname(os.path.abspath(__file__))
    cpu_script = os.path.join(script_dir, "cpu_stress.py")
    p_cpu = subprocess.Popen(["python3", cpu_script, "3600"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    p_gpu = subprocess.Popen(["glxgears"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(1.0)
    return [p_cpu, p_gpu]

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

def run_trial(use_harsh=True, cpu_quota=25, gap_ms=10):
    tmp_out = "/tmp/matrix_term.bin"
    tmp_ready = "/tmp/matrix_term.ready"
    if os.path.exists(tmp_out):
        os.remove(tmp_out)
    if os.path.exists(tmp_ready):
        os.remove(tmp_ready)

    script_dir = os.path.dirname(os.path.abspath(__file__))
    el_dir = os.path.join(script_dir, "electron_terminal")
    env = dict(os.environ)
    env.pop("ELECTRON_RUN_AS_NODE", None)

    if use_harsh:
        cmd = ["systemd-run", "--user", "--scope", f"-pCPUQuota={cpu_quota}%", "env", "-u", "ELECTRON_RUN_AS_NODE", "/usr/lib/electron40/electron", el_dir, tmp_out, tmp_ready]
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
        if proc: proc.kill()
        return False, [], "failed ready marker", []

    focus_electron_window()
    time.sleep(0.4)

    subprocess.run(["fcitx5-remote", "-s", "clak"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    # snapshot clak.log size before typing
    clak_log = "/tmp/clak.log"
    log_offset = 0
    if os.path.exists(clak_log):
        log_offset = os.path.getsize(clak_log)

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
            try: proc.kill()
            except Exception: pass

    actual = ""
    if os.path.exists(tmp_out):
        with open(tmp_out, "rb") as f:
            actual = decode_terminal_bytes(f.read()).strip()

    exp_words = EXPECTED_TEXT.split()
    act_words = actual.split()
    mismatched_words = []
    for i, (ew, aw) in enumerate(zip(exp_words, act_words)):
        if ew != aw:
            mismatched_words.append((i, ew, aw))

    # parse pacing events from log slice
    pacing_events = []
    if os.path.exists(clak_log):
        with open(clak_log, "r", encoding="utf-8", errors="ignore") as f:
            f.seek(log_offset)
            for line in f:
                if "uinput waiting for sentinel:" in line:
                    pacing_events.append(line.strip())

    is_match = (actual == EXPECTED_TEXT)
    return is_match, mismatched_words, actual, pacing_events

def main():
    print("================================================================================")
    print("THÍ NGHIỆM ĐO PHÍ HỌC (LEARNING COST) VÀ HIỆU QUẢ COLD VS WARM START")
    print("Môi trường: Electron dưới tải khắc nghiệt (glxgears + cpu_stress + CPUQuota=25%)")
    print("================================================================================\n")

    # 1. COLD START DƯỚI TẢI KHẮC NGHIỆT (Reset sạch hoàn toàn)
    print("--- 1. KIỂM THỬ COLD START (Khởi động lạnh, không có cache đĩa) ---")
    stress_procs = start_stress()
    restart_fcitx5(preload_pacing=None)

    ok_cold, mismatches_cold, actual_cold, events_cold = run_trial(use_harsh=True)
    stop_stress(stress_procs)

    print(f"Cold Start Kết quả: {'THÀNH CÔNG (0 lỗi)' if ok_cold else 'CÓ LỖI'}")
    if mismatches_cold:
        print(f"  Tổng số từ lỗi: {len(mismatches_cold)}/{len(EXPECTED_TEXT.split())}")
        for idx, ew, aw in mismatches_cold[:10]:
            print(f"    - Từ #{idx}: mong đợi '{ew}', thực tế '{aw}'")
        first_err = mismatches_cold[0][0]
        last_err = mismatches_cold[-1][0]
        print(f"  Phạm vi lỗi: Từ #{first_err} đến từ #{last_err}")
    else:
        print("  Không có từ nào bị lỗi ngay cả ở cold start!")

    print(f"  Số sự kiện uinput trong lượt cold: {len(events_cold)}")
    if events_cold:
        first_post = [e for e in events_cold if "post_delay=" in e]
        if first_post:
            print(f"  Post delay đầu tiên: {first_post[0].split('post_delay=')[1].split()[0]}")
            print(f"  Post delay cuối cùng: {first_post[-1].split('post_delay=')[1].split()[0]}")

    # 2. WARM START VỚI CACHE SÀN HIỆN TẠI (floor + 4 = 20ms)
    print("\n--- 2. KIỂM THỬ WARM START VỚI CACHE SÀN (floor + 4 = 20ms) ---")
    cached_floor_json = json.dumps({
        "env_key": "hyprland:acc1aa0bfd4b:0.7.8:fcitx5",
        "timestamp": int(time.time()),
        "web": {"delay": 20, "gap": 8},
        "terminal": {"delay": 16, "gap": 4},
        "heavy": {"delay": 19, "gap": 5},
        "default": {"delay": 12, "gap": 3}
    }, indent=2)

    stress_procs = start_stress()
    restart_fcitx5(preload_pacing=cached_floor_json)
    ok_warm_floor, mismatches_warm_floor, _, events_warm_floor = run_trial(use_harsh=True)
    stop_stress(stress_procs)

    print(f"Warm Start (Sàn 20ms) Kết quả: {'THÀNH CÔNG (0 lỗi)' if ok_warm_floor else 'CÓ LỖI'}")
    if mismatches_warm_floor:
        print(f"  Tổng số từ lỗi: {len(mismatches_warm_floor)}")
        for idx, ew, aw in mismatches_warm_floor[:10]:
            print(f"    - Từ #{idx}: mong đợi '{ew}', thực tế '{aw}'")
    else:
        print("  Sạch lỗi 100%!")

    # 3. WARM START VỚI CACHE ĐÃ HỘI TỤ DƯỚI TẢI (32ms, gap 8ms)
    print("\n--- 3. KIỂM THỬ WARM START VỚI CACHE HỘI TỤ (32ms, gap 8ms) ---")
    cached_converged_json = json.dumps({
        "env_key": "hyprland:acc1aa0bfd4b:0.7.8:fcitx5",
        "timestamp": int(time.time()),
        "web": {"delay": 32, "gap": 8},
        "terminal": {"delay": 24, "gap": 6},
        "heavy": {"delay": 30, "gap": 8},
        "default": {"delay": 20, "gap": 5}
    }, indent=2)

    stress_procs = start_stress()
    restart_fcitx5(preload_pacing=cached_converged_json)
    ok_warm_conv, mismatches_warm_conv, _, events_warm_conv = run_trial(use_harsh=True)
    stop_stress(stress_procs)

    print(f"Warm Start (Hội tụ 32ms) Kết quả: {'THÀNH CÔNG (0 lỗi)' if ok_warm_conv else 'CÓ LỖI'}")
    if mismatches_warm_conv:
        print(f"  Tổng số từ lỗi: {len(mismatches_warm_conv)}")
        for idx, ew, aw in mismatches_warm_conv[:10]:
            print(f"    - Từ #{idx}: mong đợi '{ew}', thực tế '{aw}'")
    else:
        print("  Sạch lỗi 100%!")

    # 4. ĐO THỜI GIAN CO VỀ SÀN SAU KHI HẾT TẢI (Decay back to floor)
    print("\n--- 4. ĐO TỐC ĐỘ CO VỀ SÀN SAU KHI HẾT TẢI ---")
    # Fcitx5 is currently converged at high delay from step 3. Now run idle typing trials.
    print("Chạy gõ lúc rảnh để quan sát tốc độ co về sàn...")
    decay_steps = []
    for step in range(1, 4):
        _, _, _, events_idle = run_trial(use_harsh=False)
        if events_idle:
            first_p = [e for e in events_idle if "post_delay=" in e]
            if first_p:
                delay_val = first_p[-1].split("post_delay=")[1].split()[0]
                decay_steps.append((step, delay_val))
                print(f"  Lượt {step} (140 từ lúc rảnh): delay co về {delay_val}")

    print("\n================================================================================")
    print("TỔNG KẾT THỰC NGHIỆM")
    print("================================================================================")
    print(f"Cold Start dưới tải:       {'100% OK' if ok_cold else f'{len(mismatches_cold)} từ lỗi'}")
    print(f"Warm Start (Sàn +4ms):      {'100% OK' if ok_warm_floor else f'{len(mismatches_warm_floor)} từ lỗi'}")
    print(f"Warm Start (Hội tụ 32ms):   {'100% OK' if ok_warm_conv else f'{len(mismatches_warm_conv)} từ lỗi'}")
    for step, d in decay_steps:
        print(f"Tốc độ co về sàn:          Lượt {step} (140 từ): delay = {d}")
    print("================================================================================\n")

if __name__ == "__main__":
    main()
