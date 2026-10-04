import ctypes, sys
x11 = ctypes.CDLL("libX11.so.6"); xe = ctypes.CDLL("libXext.so.6")
x11.XOpenDisplay.restype = ctypes.c_void_p
class R(ctypes.Structure): _fields_ = [("x", ctypes.c_short), ("y", ctypes.c_short), ("w", ctypes.c_ushort), ("h", ctypes.c_ushort)]
xe.XShapeGetRectangles.restype = ctypes.POINTER(R)
xe.XShapeGetRectangles.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_int)]
d = x11.XOpenDisplay(None)
w = int(sys.argv[1], 0)
for kind, name in ((0, "bounding"), (2, "input")):
    n = ctypes.c_int(); o = ctypes.c_int()
    p = xe.XShapeGetRectangles(d, w, kind, ctypes.byref(n), ctypes.byref(o))
    print(name, n.value, "rects", [(p[i].x, p[i].y, p[i].w, p[i].h) for i in range(min(n.value, 5))])
