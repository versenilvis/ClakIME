import os
import sys
import tty
import termios

output_file = sys.argv[1] if len(sys.argv) > 1 else "/tmp/raw_receiver.bin"
ready_file = sys.argv[2] if len(sys.argv) > 2 else "/tmp/clak_receiver.ready"

fd = sys.stdin.fileno()
old_settings = termios.tcgetattr(fd)

try:
    tty.setraw(fd)
    with open(ready_file, "w") as f_ready:
        f_ready.write("ready\n")
        f_ready.flush()

    with open(output_file, "wb") as f:
        while True:
            b = os.read(fd, 1)
            if not b or b == b'\x04':
                break
            f.write(b)
            f.flush()
finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, old_settings)
    if os.path.exists(ready_file):
        try:
            os.remove(ready_file)
        except OSError:
            pass
