#!/usr/bin/env bash
# End-to-end check of the clock pill with REAL input (uinput → libinput → GNOME
# Shell → XWayland), as a person's mouse would go. Needs the flat accel profile.
# Usage: e2e.sh <cycles>   (set the flat accel profile first: see README.md)
set -u
export DISPLAY=:0
S=$(dirname "$0")
CYCLES=${1:-3}
W=$(for w in $(xdotool search --name '^Coucou$'); do xwininfo -id "$w" | grep -q IsViewable && echo "$w"; done | head -1)
geo() { xwininfo -id "$W" | grep -E "Width|Height" | awk '{print $NF}' | paste -sd'x'; }
top_y() { xwininfo -id "$W" | awk '/Absolute upper-left Y/{print $NF}'; }
input_shape() { python3 "$S/shape.py" "$W" | grep '^input'; }
near() { # near <WxH> <WxH>: equal within 1px each way
  python3 -c "import sys; a=[int(v) for v in sys.argv[1].split('x')]; b=[int(v) for v in sys.argv[2].split('x')]; sys.exit(0 if all(abs(p-q)<=1 for p,q in zip(a,b)) else 1)" "$1" "$2"
}
wait_for() { # wait_for <geometry> <seconds>
  for _ in $(seq 1 "$2"); do near "$(geo)" "$1" && return 0; sleep 1; done; return 1
}
pass=0; fail=0
ok() { echo "  PASS $1"; pass=$((pass+1)); }
ko() { echo "  FAIL $1"; fail=$((fail+1)); }

for c in $(seq 1 "$CYCLES"); do
  echo "cycle $c"
  python3 "$S/vmouse.py" move 1280 700 >/dev/null
  # Live Claude sessions keep peeking the compact island (by design), so a cycle
  # may have to start there; the pill checks run whenever it does settle.
  if wait_for 184x32 90; then
    ok "collapsed to the 184x32 pill"
    x=$(xwininfo -id "$W" | awk '/Absolute upper-left X/{print $NF}')
    if [ "$x" -ge 1187 ] && [ "$x" -le 1189 ]; then ok "pill centred (x=$x)"; else ko "pill at x=$x"; fi
    python3 "$S/vmouse.py" move 1280 16 >/dev/null
    if wait_for 288x32 5; then ok "hover opens the compact island (window 288x32)"; else ko "hover did nothing (geo $(geo))"; continue; fi
  elif near "$(geo)" 288x32 && [ "$(top_y)" = 0 ]; then
    echo "  (sessions active: starting from the compact island)"
    python3 "$S/vmouse.py" move 1280 16 >/dev/null
  else
    ko "neither pill nor compact (geo $(geo) y=$(top_y))"; continue
  fi
  sleep 0.6
  python3 "$S/vmouse.py" click >/dev/null
  sleep 1.5
  # Never click a tab position unless the popup is really there: anything else
  # under that point is one of the user's windows.
  if [ "$(top_y)" -lt 30 ] || ! near "$(geo | cut -dx -f1)x0" 640x0; then
    ko "click did not open the popup ($(geo) y=$(top_y))"; continue
  fi
  ok "click opens the popup ($(geo))"
  # A live Claude session makes the click open the overview; go to the calendar tab.
  python3 "$S/vmouse.py" moveclick 1134 56 >/dev/null
  sleep 1.5
  xwd -silent -id "$W" -out "$S/cyc$c.xwd" && convert "$S/cyc$c.xwd" "$S/cyc$c.png"
  # Today's cell is the only indigo (#6366F1) area: present only on the calendar.
  indigo=$(convert "$S/cyc$c.png" -fuzz 6% -fill white -opaque '#6366F1' -fill black +opaque white -format '%[fx:mean*w*h]' info:)
  if [ "${indigo%.*}" -gt 100 ]; then ok "click shows the calendar ($indigo indigo px)"; else ko "click did not show the calendar ($indigo indigo px)"; fi
  if near "$(geo)" 640x262; then ok "window is exactly the calendar island (640x262): nothing below it is blocked"; else ko "calendar window $(geo)"; fi
  A=$(xprop -root _NET_ACTIVE_WINDOW | grep -o '0x[0-9a-f]*$')
  if [ -n "$A" ] && [ "$((A))" = "$((W))" ]; then ok "popup has keyboard focus"; else ko "popup not focused (active=$A island=$W)"; fi
  y=$(xwininfo -id "$W" | awk '/Absolute upper-left Y/{print $NF}')
  if [ "$y" -ge 30 ]; then ok "popup sits under the top bar (y=$y)"; else ko "popup y=$y"; fi
  # Type only into a window we have just proven holds the keyboard.
  if [ -n "$A" ] && [ "$((A))" = "$((W))" ]; then
    python3 "$S/vkbd.py" '\e'; sleep 1.5
    # Back in the bar: the pill, or compact if a session's work event peeks it.
    if [ "$(top_y)" = 0 ] && { near "$(geo)" 184x32 || near "$(geo)" 288x32; }; then ok "Esc closes the popup ($(geo) in the bar)"; else ko "Esc did nothing ($(geo) y=$(top_y))"; fi
  fi
done
echo "RESULT pass=$pass fail=$fail"
