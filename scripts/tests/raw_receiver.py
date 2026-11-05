import os
import sys
import tty
import termios
import time
import json

output_file = sys.argv[1] if len(sys.argv) > 1 else "/tmp/raw_receiver_events.jsonl"

fd = sys.stdin.fileno()
old_settings = termios.tcgetattr(fd)

# unbuffered stdout
sys.stdout.reconfigure(line_buffering=True)
print(f"READY:{output_file}")

try:
    tty.setraw(fd)
    with open(output_file, "w", encoding="utf-8") as f:
        while True:
            b = os.read(fd, 1)
            if not b or b == b'\x04': # eof or ctrl+d
                break
            record = {
                "t": time.time(),
                "b": b[0],
                "char": chr(b[0]) if 0x20 <= b[0] <= 0x7e else ""
            }
            f.write(json.dumps(record) + "\n")
            f.flush()
finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, old_settings)
