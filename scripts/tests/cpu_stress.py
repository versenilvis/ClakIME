import multiprocessing
import os
import signal
import sys
import time

def worker(stop_flag, nice_val):
    try:
        if nice_val != 0:
            os.nice(nice_val)
    except Exception:
        pass
    while not stop_flag.value:
        _ = 999999999 * 999999999

if __name__ == "__main__":
    cores = os.cpu_count() or 4
    nice_val = int(sys.argv[2]) if len(sys.argv) > 2 else 10
    stop_flag = multiprocessing.Value("b", False)
    procs = [multiprocessing.Process(target=worker, args=(stop_flag, nice_val)) for _ in range(cores)]
    for p in procs:
        p.start()

    def on_term(signum, frame):
        stop_flag.value = True
        for p in procs:
            p.join()
        sys.exit(0)

    signal.signal(signal.SIGTERM, on_term)
    signal.signal(signal.SIGINT, on_term)

    timeout = float(sys.argv[1]) if len(sys.argv) > 1 else 60.0
    time.sleep(timeout)
    stop_flag.value = True
    for p in procs:
        p.join()
