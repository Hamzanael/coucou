"""Virtual kernel keyboard (uinput). Usage: vkbd.py <text-of-lowercase-letters>"""
import fcntl, os, struct, sys, time
UI_SET_EVBIT, UI_SET_KEYBIT, UI_DEV_SETUP, UI_DEV_CREATE, UI_DEV_DESTROY = 0x40045564, 0x40045565, 0x405C5503, 0x5501, 0x5502
KEYS = {c: k for c, k in zip("qwertyuiop", range(16, 26))} | {c: k for c, k in zip("asdfghjkl", range(30, 39))} | {c: k for c, k in zip("zxcvbnm", range(44, 51))}
KEYS["\n"] = 28; KEYS[" "] = 57; KEYS["\x1b"] = 1
fd = os.open("/dev/uinput", os.O_WRONLY | os.O_NONBLOCK)
fcntl.ioctl(fd, UI_SET_EVBIT, 1)
for k in set(KEYS.values()): fcntl.ioctl(fd, UI_SET_KEYBIT, k)
fcntl.ioctl(fd, UI_DEV_SETUP, struct.pack("HHHH80sI", 3, 0x1234, 0x5679, 1, b"claude-vkbd", 0))
fcntl.ioctl(fd, UI_DEV_CREATE); time.sleep(1.0)
def emit(t, c, v): os.write(fd, struct.pack("llHHi", 0, 0, t, c, v))
for ch in sys.argv[1].replace("\\n", "\n").replace("\\e", "\x1b"):
    emit(1, KEYS[ch], 1); emit(0, 0, 0); time.sleep(0.02); emit(1, KEYS[ch], 0); emit(0, 0, 0); time.sleep(0.03)
time.sleep(0.3); fcntl.ioctl(fd, UI_DEV_DESTROY); os.close(fd)
