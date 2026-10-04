# Linux end-to-end harness (real input)

`xdotool` clicks are injected inside XWayland and skip GNOME Shell's input routing,
so they pass where a real mouse fails. These scripts create kernel input devices
through `/dev/uinput` (needs rw access — `getfacl /dev/uinput`), so events travel
libinput → GNOME Shell → XWayland exactly like a physical mouse and keyboard.

- `vmouse.py move X Y | click | moveclick X Y` — needs exact 1 unit = 1 px:
  `gsettings set org.gnome.desktop.peripherals.mouse accel-profile flat` and `speed 0`
  (note the old values first and restore them after).
- `vkbd.py '<text>'` (`\e` = Esc, `\n` = Enter) types into **whatever has keyboard focus** —
  only call it after checking `xprop -root _NET_ACTIVE_WINDOW` is the window under test.
- `shape.py <window-id>` prints the bounding and input shape rectangles.
- `e2e.sh <cycles>` — pill → hover → click → calendar popup (focus, position) → Esc.
- Screenshots: `xwd -silent -id <window> | convert xwd:- out.png` (root grabs are blocked on Wayland).
