"""Virtual kernel mouse (uinput): its events go through libinput and GNOME Shell,
exactly like a physical mouse. Usage: vmouse.py move X Y | click | moveclick X Y"""
import fcntl, os, struct, subprocess, sys, time

UI_SET_EVBIT, UI_SET_KEYBIT, UI_SET_RELBIT = 0x40045564, 0x40045565, 0x40045566
UI_DEV_SETUP, UI_DEV_CREATE, UI_DEV_DESTROY = 0x405C5503, 0x5501, 0x5502
EV_SYN, EV_KEY, EV_REL = 0, 1, 2
REL_X, REL_Y, BTN_LEFT = 0, 1, 0x110


def where():
    out = subprocess.run(["xdotool", "getmouselocation"], capture_output=True, text=True,
                         env={**os.environ, "DISPLAY": ":0"}).stdout
    parts = dict(p.split(":") for p in out.split()[:2])
    return int(parts["x"]), int(parts["y"])


class Mouse:
    def __init__(self):
        self.fd = os.open("/dev/uinput", os.O_WRONLY | os.O_NONBLOCK)
        for ev in (EV_KEY, EV_REL):
            fcntl.ioctl(self.fd, UI_SET_EVBIT, ev)
        fcntl.ioctl(self.fd, UI_SET_KEYBIT, BTN_LEFT)
        for rel in (REL_X, REL_Y):
            fcntl.ioctl(self.fd, UI_SET_RELBIT, rel)
        setup = struct.pack("HHHH80sI", 3, 0x1234, 0x5678, 1, b"claude-vmouse", 0)
        fcntl.ioctl(self.fd, UI_DEV_SETUP, setup)
        fcntl.ioctl(self.fd, UI_DEV_CREATE)
        time.sleep(1.0)  # let libinput/mutter pick the device up

    def emit(self, etype, code, value):
        os.write(self.fd, struct.pack("llHHi", 0, 0, etype, code, value))

    def rel(self, dx, dy):
        self.emit(EV_REL, REL_X, dx)
        self.emit(EV_REL, REL_Y, dy)
        self.emit(EV_SYN, 0, 0)

    def move_to(self, tx, ty):
        # Needs the flat accel profile at speed 0 (1 unit = 1 px): pin the
        # pointer in the top-left corner, then walk there in exact steps.
        for _ in range(60):
            self.rel(-100, -100)
            time.sleep(0.004)
        x = y = 0
        while (x, y) != (tx, ty):
            dx, dy = max(-20, min(20, tx - x)), max(-20, min(20, ty - y))
            self.rel(dx, dy)
            x, y = x + dx, y + dy
            time.sleep(0.01)
        time.sleep(0.15)
        return where()

    def click(self):
        self.emit(EV_KEY, BTN_LEFT, 1); self.emit(EV_SYN, 0, 0)
        time.sleep(0.06)
        self.emit(EV_KEY, BTN_LEFT, 0); self.emit(EV_SYN, 0, 0)

    def close(self):
        time.sleep(0.2)
        fcntl.ioctl(self.fd, UI_DEV_DESTROY)
        os.close(self.fd)


if __name__ == "__main__":
    m = Mouse()
    try:
        cmd = sys.argv[1]
        if cmd in ("move", "moveclick"):
            print("at", m.move_to(int(sys.argv[2]), int(sys.argv[3])))
        if cmd in ("click", "moveclick"):
            time.sleep(0.5)
            m.click()
            print("clicked")
    finally:
        m.close()
