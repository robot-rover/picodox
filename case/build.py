# %%
# Imports
from build123d import *
from ocp_vscode import *
from import_svg import import_svg_as_forced_outline as import_svg
import re
import matplotlib.pyplot as plt
import numpy as np
from pathlib import Path

# %% Import Svg

outline = import_svg("picodox.svg", reorient=False)
show(outline)
print(outline.bounding_box().size)

# %% Import Holes
NPTH_FILE = "picodox-NPTH.drl"
PTH_FILE = "picodox-PTH.drl"
MOUNT_TOOL = 3
PICO_TOOL = 6

def filter_lines(lines, tool):
    lines = iter(lines)
    while next(lines).strip() != f'T{tool}':
        pass
    while True:
        line = next(lines, None)
        if line is None or line.startswith("T"):
            return
        m = re.match(r"X(-?\d+\.\d+)Y(-?\d+\.\d+)", line)
        yield (float(m.group(1)), float(m.group(2)))

with open(NPTH_FILE, 'rt') as f:
    screw_holes = [
        (x, y)
        for x, y in filter_lines(f.readlines(), MOUNT_TOOL)
        if x < 145 or x > 160
    ]
    assert len(screw_holes) == 14

with open(PTH_FILE, 'rt') as f:
    pico_holes = np.array([
        (x, y)
        for x, y in filter_lines(f.readlines(), PICO_TOOL)
        if y > -60
    ])
    assert len(pico_holes) == 24
    
pico_hole_left = np.min(pico_holes[:, 0])
pico_hole_right = np.max(pico_holes[:, 0])
pico_hole_top = np.min(pico_holes[:, 1])
pico_hole_bottom = np.max(pico_holes[:, 1])
print(pico_hole_left, pico_hole_right)

# %% Create Case

hole_dia = 3.2 * MM
pad_dia = 6 * MM

thd_padding = 3 * MM
pico_pin_padding = 5 * MM
pcb_thickness = 1.5 * MM
wall_thickness = 2 * MM
base_thickness = 4 * MM
fudge = 0.5 * MM

nut_dia = 6 * MM
nut_thickness = 3 * MM

height = base_thickness + thd_padding + pcb_thickness

pico_width = 18 * MM
pico_height = 31 * MM
pico_tl = Vector(166.8125 * MM, -36.66875 * MM)
grid_origin = Vector(23.8125 * MM, -16.66875 * MM)

foot_dim = 10.6 * MM
foot_inset_depth = 1 * MM
foot_locs = [
    (15, -12),
    (15, -108),
    (165, -20),
    (140, -120),
]

bed_width = 7.26 * IN

with BuildPart() as case:
    # Create the case outline
    offset(outline, wall_thickness + fudge, kind=Kind.INTERSECTION, mode=Mode.ADD)
    extrude(amount=height)

    chamfer(case.faces().sort_by(Axis.Z)[-1].clean().edges(), 2 * MM)

    offset(outline.moved(Pos(0, 0, height)), fudge, kind=Kind.INTERSECTION, mode=Mode.ADD)
    extrude(amount=-(thd_padding + pcb_thickness), mode=Mode.SUBTRACT)
    
    
    with BuildSketch() as pad:
        with Locations(screw_holes):
            Circle(pad_dia/2)
    extrude(amount=base_thickness + thd_padding)
    with BuildSketch() as hole:
        with Locations(screw_holes):
            Circle(hole_dia/2)
    extrude(amount=base_thickness + thd_padding, mode=Mode.SUBTRACT)
    with BuildSketch() as hex_hole:
        with Locations(screw_holes):
            RegularPolygon(nut_dia/2, 6, major_radius=False)
    extrude(amount=nut_thickness, mode=Mode.SUBTRACT)
    
    circle_bases = case.edges().filter_by(GeomType.CIRCLE).filter_by_position(Axis.Z, base_thickness, base_thickness).filter_by(lambda c: c.radius == pad_dia/2)
    fillet(circle_bases, 1.9 * MM)
    
    with BuildSketch(Plane.XY.offset(thd_padding + base_thickness-pico_pin_padding)) as pico_hole:
        # with Locations(pico_tl - grid_origin):
        #     Rectangle(pico_width, pico_height, align=(Align.MIN, Align.MAX))
        # with Locations(pico_holes):
        #     Circle(1.5)
        for x in (pico_hole_left, pico_hole_right):
            SlotArc(arc=Line((x, pico_hole_top), (x, pico_hole_bottom)), height=1.5 * MM)
    extrude(amount=pico_pin_padding, mode=Mode.SUBTRACT)
    
    with BuildSketch() as feet_insets:
        with Locations(foot_locs):
            Rectangle(foot_dim, foot_dim)
    extrude(amount=foot_inset_depth, mode=Mode.SUBTRACT)
    
    bb = case.part.bounding_box()
    width = bb.size.to_tuple()[0]
    left = bb.min.to_tuple()[0]
    right = bb.max.to_tuple()[0]
    print(left, right)
    
    with BuildSketch(Location((bb.max.to_tuple()[0], 0, 0))) as fit_bed:
        rect = Rectangle(width=wall_thickness, height=bb.size.to_tuple()[1], align=(Align.MAX, Align.MAX))
    extrude(until=Until.LAST, mode=Mode.SUBTRACT)
    
    top_right_edge = case.edges().group_by(Axis.X)[-1].sort_by(Axis.Y)[-1]
    face = Face.extrude(top_right_edge, Vector(-1, 0, 0) * wall_thickness)
    fill_gap = extrude(face, dir=Vector(0, -1, 0), until=Until.LAST)

show(case)

print(case.part.bounding_box().size / IN)

# %%

case_right = mirror(case.part, about=Plane.YZ)

out_dir = Path('output')
out_dir.mkdir(exist_ok=True)
exporterl = Mesher()
exporterl.add_shape(case.part)
exporterl.write(out_dir / "keyboard_left.stl")

exporterr = Mesher()
exporterr.add_shape(case_right)
exporterr.write(out_dir / "keyboard_right.stl")

