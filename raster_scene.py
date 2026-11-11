"""Rasterize a GoodLookin scene dump into a PNG."""

import sys
from PIL import Image, ImageDraw, ImageFont

def col(parts, i):
    r, g, b, a = (int(parts[i + k]) for k in range(4))
    return (r, g, b, a)

def lerp(a, b, t):
    return tuple(int(a[k] + (b[k] - a[k]) * t) for k in range(4))

def font(size):
    for path in (
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "/Library/Fonts/Arial.ttf",
    ):
        try:
            return ImageFont.truetype(path, max(8, int(size)))
        except OSError:
            continue
    return ImageFont.load_default()

def blend(base, src, alpha):
    if alpha >= 250:
        return src
    t = alpha / 255.0
    return tuple(int(base[k] * (1 - t) + src[k] * t) for k in range(3)) + (255,)

def main():
    lines = sys.stdin.read().splitlines()
    width, height = 980, 460
    image = Image.new("RGBA", (width, height), (20, 24, 28, 255))
    clips = []
    for line in lines:
        parts = line.split()
        if not parts:
            continue
        kind = parts[0]
        if kind == "SIZE":
            width, height = int(float(parts[1])), int(float(parts[2]))
            image = Image.new("RGBA", (width, height), (20, 24, 28, 255))
            continue
        if kind == "CLIP":
            clips.append(tuple(float(v) for v in parts[1:6]))
            continue
        if kind == "POP":
            if clips:
                clips.pop()
            continue
        layer = Image.new("RGBA", image.size, (0, 0, 0, 0))
        px = layer.load()
        draw = ImageDraw.Draw(layer)
        if kind == "FILL_RECT":
            x, y, w, h = (float(parts[i]) for i in range(1, 5))
            top, bot = col(parts, 5), col(parts, 9)
            vertical = parts[13] == "1"
            x0, y0, x1, y1 = int(x), int(y), int(x + w), int(y + h)
            for yy in range(max(0, y0), min(height, y1)):
                t = 0 if y1 == y0 else (yy - y0) / max(1, y1 - y0)
                if not vertical:
                    t = 0.5
                c = lerp(top, bot, t)
                for xx in range(max(0, x0), min(width, x1)):
                    px[xx, yy] = c
        elif kind == "FILL_ROUND":
            x, y, w, h, rad = (float(parts[i]) for i in range(1, 6))
            top, bot = col(parts, 6), col(parts, 10)
            draw.rounded_rectangle([x, y, x + w, y + h], radius=rad, fill=top)
        elif kind == "STROKE_ROUND":
            x, y, w, h, rad = (float(parts[i]) for i in range(1, 6))
            color = col(parts, 6)
            width_px = max(1, int(float(parts[10])))
            draw.rounded_rectangle([x, y, x + w, y + h], radius=rad, outline=color, width=width_px)
        elif kind == "FILL_ELLIPSE":
            x, y, w, h = (float(parts[i]) for i in range(1, 5))
            top, bot = col(parts, 5), col(parts, 9)
            x0, y0, x1, y1 = int(x), int(y), int(x + w), int(y + h)
            cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
            rx, ry = max(1, (x1 - x0) / 2), max(1, (y1 - y0) / 2)
            for yy in range(max(0, y0), min(height, y1)):
                t = 0 if y1 == y0 else (yy - y0) / max(1, y1 - y0)
                c = lerp(top, bot, t)
                for xx in range(max(0, x0), min(width, x1)):
                    if ((xx - cx) / rx) ** 2 + ((yy - cy) / ry) ** 2 <= 1.0:
                        px[xx, yy] = c
        elif kind == "STROKE_ELLIPSE":
            x, y, w, h = (float(parts[i]) for i in range(1, 5))
            color = col(parts, 5)
            width_px = max(1, int(float(parts[9])))
            draw.ellipse([x, y, x + w, y + h], outline=color, width=width_px)
        elif kind == "LINE":
            x1, y1, x2, y2 = (float(parts[i]) for i in range(1, 5))
            color = col(parts, 5)
            width_px = max(1, int(float(parts[9])))
            draw.line([x1, y1, x2, y2], fill=color, width=width_px)
        elif kind == "FILL_POLY":
            n = int(parts[1])
            pts = []
            for i in range(n):
                pts.append((float(parts[2 + i * 2]), float(parts[3 + i * 2])))
            top = col(parts, 2 + n * 2)
            if len(pts) >= 3:
                draw.polygon(pts, fill=top)
        elif kind == "STROKE_POLY":
            n = int(parts[1])
            pts = [(float(parts[2 + i * 2]), float(parts[3 + i * 2])) for i in range(n)]
            color = col(parts, 2 + n * 2)
            width_px = max(1, int(float(parts[6 + n * 2])))
            if len(pts) >= 2:
                draw.line(pts, fill=color, width=width_px)
        elif kind == "ARC":
            cx, cy, radius = float(parts[1]), float(parts[2]), float(parts[3])
            a0, a1 = float(parts[4]), float(parts[5])
            color = col(parts, 6)
            width_px = max(1, int(float(parts[10])))
            # PIL arcs are degrees, 0 at 3 o'clock, clockwise... PIL is counter-clockwise from 3 o'clock.
            import math
            draw.arc(
                [cx - radius, cy - radius, cx + radius, cy + radius],
                start=math.degrees(a0),
                end=math.degrees(a1),
                fill=color,
                width=width_px,
            )
        elif kind == "TEXT":
            x, y, size = float(parts[1]), float(parts[2]), float(parts[3])
            color = col(parts, 4)
            center = parts[8] == "1"
            text = line.split(" ", 9)[-1] if len(parts) > 9 else ""
            # The split above is wrong once colors are 4 ints. Reconstruct from the original line.
            text = " ".join(parts[9:])
            f = font(size)
            if center:
                box = draw.textbbox((0, 0), text, font=f)
                tw, th = box[2] - box[0], box[3] - box[1]
                draw.text((x - tw / 2, y - th / 2), text, fill=color, font=f)
            else:
                draw.text((x, y), text, fill=color, font=f)
        else:
            continue
        if clips:
            x, y, w, h, rad = clips[-1]
            mask = Image.new("L", image.size, 0)
            ImageDraw.Draw(mask).rounded_rectangle([x, y, x + w, y + h], radius=rad, fill=255)
            blank = Image.new("RGBA", image.size, (0, 0, 0, 0))
            blank.paste(layer, (0, 0), mask)
            layer = blank
        image = Image.alpha_composite(image, layer)
    image.convert("RGB").save(sys.argv[1])

if __name__ == "__main__":
    main()
