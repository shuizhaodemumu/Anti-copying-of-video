#!/usr/bin/env python3
"""Generate the original text sheet and 30-fps MJPEG clip. Requires Pillow and fontconfig."""
from functools import lru_cache
from math import cos, pi, sin
from pathlib import Path
from random import Random
from subprocess import check_output

from PIL import Image, ImageDraw, ImageFont

WIDTH, HEIGHT = 800, 450
FPS, SECONDS = 30, 10
SCALE = 2
ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets"


def coordinates(values):
    return tuple(round(v * SCALE) for v in values)


@lru_cache()
def font_file(family):
    output = check_output(["fc-match", "-f", "%{file}\n%{index}", family], text=True)
    path, index = output.splitlines()
    return path, int(index) & 0xFFFF


@lru_cache()
def font(size, family="Noto Sans CJK SC", weight=450):
    path, index = font_file(family)
    face = ImageFont.truetype(path, size * SCALE, index=index)
    try:
        axes = face.get_variation_axes()
    except OSError:
        return face
    face.set_variation_by_axes([
        min(axis["maximum"], max(axis["minimum"], weight))
        if axis["name"] == b"Weight" else axis["default"] for axis in axes
    ])
    return face


def label(draw, x, y, text, size, color, *, family="Noto Sans CJK SC", weight=450, anchor="lt"):
    draw.text(coordinates((x, y)), text, font=font(size, family, weight),
              fill=color, anchor=anchor, language="zh-Hans")


def rect(draw, bounds, fill):
    draw.rectangle(coordinates(bounds), fill=fill)


def rounded(draw, bounds, radius, fill):
    draw.rounded_rectangle(coordinates(bounds), radius=radius * SCALE, fill=fill)


def line(draw, points, fill, width=1):
    draw.line([coordinates(point) for point in points], fill=fill,
              width=round(width * SCALE), joint="curve")


def ellipse(draw, bounds, fill):
    draw.ellipse(coordinates(bounds), fill=fill)


def polygon(draw, points, fill):
    draw.polygon([coordinates(point) for point in points], fill=fill)


def new_image(color):
    return Image.new("RGB", (WIDTH * SCALE, HEIGHT * SCALE), color)


def downsample(im):
    return im.resize((WIDTH, HEIGHT), Image.Resampling.LANCZOS)


def make_sheet():
    image = new_image("#efefef")
    d = ImageDraw.Draw(image)
    ink = "#232323"
    secondary = "#535353"
    label(d, 48, 34, "渡口日志", 44, ink, family="Noto Serif CJK SC", weight=700)
    label(d, 752, 50, "河谷站 · 早班记录", 17, secondary, anchor="rt")
    line(d, [(48, 99), (752, 99)], "#696969", 1)
    label(d, 48, 124, "六点四十，第一班渡船靠岸。", 31, ink, weight=550)
    label(d, 48, 188, "水面有薄雾。值班员收起缆绳，沿着西岸走向灯塔。", 22, ink)
    label(d, 48, 235, "桥边有三盏灯，最远的一盏已熄灭。", 17, ink)
    label(d, 48, 274, "小字记录：风向东北，气温十二度，水尺读数 1.86 米。", 13, secondary)
    rect(d, (48, 327, 752, 411), "#dedede")
    label(d, 68, 344, "核对码", 15, secondary, weight=550)
    label(d, 68, 371, "请逐字抄录", 13, secondary)
    label(d, 248, 347, "K7M4 · R8Q2", 42, ink, family="Noto Sans Mono", weight=600)
    downsample(image).save(ASSETS / "text.jpg", quality=93, subsampling=0, optimize=True)


def blend(a, b, t):
    return tuple(round(x * (1 - t) + y * t) for x, y in zip(a, b))


def ridge(draw, baseline, seeds, color):
    rng = Random(seeds)
    ridge_points = []
    for x in range(-16, WIDTH + 32, 4):
        y = (baseline + 12 * sin(x / 74 + seeds) + 19 * sin(x / 136)
             + 8 * sin(x / 28) + rng.uniform(-1, 1))
        ridge_points.append((x, y))
    polygon(draw, ridge_points + [(WIDTH + 32, HEIGHT), (-16, HEIGHT)], color)


def background():
    image = new_image("#cad9da")
    d = ImageDraw.Draw(image)
    for y in range(HEIGHT * SCALE):
        tint = blend((160, 195, 208), (252, 221, 175), min(1, y / (SCALE * 285)))
        d.line([(0, y), (WIDTH * SCALE, y)], fill=tint)
    ellipse(d, (556, 50, 617, 111), "#f6deac")
    ridge(d, 178, 8, "#abbfc0")
    ridge(d, 204, 3, "#92adad")
    ridge(d, 223, 1, "#759695")
    ridge(d, 255, 6, "#567e7b")
    polygon(d, [(-8, 403), (54, 328), (163, 287), (226, 295), (311, 313),
               (386, 336), (455, 346), (526, 349), (649, 312), (800, 281),
               (812, 453), (-8, 453)], "#7fa7a0")
    polygon(d, [(386, 344), (437, 343), (486, 350), (452, 356), (386, 360),
               (341, 370), (359, 380), (490, 401), (524, 411), (528, 438),
               (459, 451), (343, 451), (401, 437), (444, 417), (419, 412),
               (302, 387), (281, 372), (311, 360), (397, 351), (434, 348)], "#c0d2c5")
    def slope(x):
        return 369 + 39 * sin(x / 127) + 38 * sin(x / 240)

    ridge_points = [(x, slope(x)) for x in range(-8, 822, 4)]
    polygon(d, ridge_points + [(822, HEIGHT), (-8, HEIGHT)], "#3c6660")
    rng = Random(17)
    for x, floor in ridge_points:
        h = rng.uniform(10, 30)
        for dy, w in ((0, .24), (.25, .18), (.48, .12)):
            top = floor - h
            mid = floor - h * dy
            polygon(d, [(x, top), (x - h * w, mid), (x + h * w, mid)], "#3c6660")
        line(d, [(x, floor - h), (x, floor + 1)], "#3c6660", 2)
    # The far and near cables converge slightly toward the mountain station.
    line(d, [(31, 255), (755, 80)], "#738485", 1)
    line(d, [(31, 261), (755, 84)], "#344c50", 2)
    for x, y, name in [(64, 253, "西站"), (735, 89, "东站")]:
        line(d, [(x + 5, y), (x - 7, y + 93)], "#435d5a", 6)
        line(d, [(x - 6, y + 10), (x + 11, y + 87)], "#56706b", 2)
        line(d, [(x - 13, y + 2), (x + 21, y - 5)], "#3a514e", 5)
        rounded(d, (x - 19, y + 44, x + 23, y + 65), 2, "#dfe6d8")
        label(d, x + 2, y + 47, name, 13, "#314a46", weight=600, anchor="mt")
    return image


def cabin(draw, progress):
    # Cosine round trip joins seamlessly with zero speed at either station.
    x = 91 + progress * 606
    y = 261 - (x - 31) * 177 / 724
    for dx in (-5, 5):
        ellipse(draw, (x + dx - 2, y - 2, x + dx + 2, y + 2), "#2d4143")
    line(draw, [(x, y + 1), (x - 3, y + 11), (x + 1, y + 24)], "#334547", 3)
    rounded(draw, (x - 23, y + 21, x + 24, y + 55), 6, "#873d36")
    rounded(draw, (x - 20, y + 23, x + 21, y + 35), 3, "#d6e3db")
    line(draw, [(x, y + 22), (x, y + 36)], "#8a4940", 2)
    rounded(draw, (x - 28, y + 19, x + 27, y + 23), 2, "#563e37")
    rect(draw, (x - 21, y + 39, x + 22, y + 41), "#ce8267")
    label(draw, x, y + 42, "07", 10, "#faf0de", family="Noto Sans Mono", weight=600, anchor="mt")


CAPTIONS = [
    "七号缆车从西站出发。",
    "前方的东站就在山坡上。",
    "缆车转向，开始返回。",
    "河水绕过山脚，西站越来越近。",
]


def make_video():
    scene = background()
    path = ASSETS / "video.mjpg"
    temporary = path.with_suffix(".tmp")
    with temporary.open("wb") as target:
        for i in range(FPS * SECONDS):
            frame = scene.copy()
            d = ImageDraw.Draw(frame)
            phase = i / (FPS * SECONDS)
            cabin(d, (1 - cos(2 * pi * phase)) / 2)
            caption = CAPTIONS[min(3, int(4 * phase))]
            text_width = round(font(23, weight=550).getlength(caption) / SCALE)
            left, right = (WIDTH - text_width) / 2, (WIDTH + text_width) / 2
            rounded(d, (left - 16, 380, right + 16, 423), 5, "#273433")
            label(d, WIDTH / 2, 390, caption, 23, "#f0efea", weight=550, anchor="mt")
            downsample(frame).save(target, format="JPEG", quality=82, subsampling=2)
    temporary.replace(path)


if __name__ == "__main__":
    ASSETS.mkdir(exist_ok=True)
    make_sheet()
    make_video()
