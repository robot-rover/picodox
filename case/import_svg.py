import copy
import os
from math import degrees, sqrt
from pathlib import Path
from typing import TextIO, Union
import svgpathtools as svg

from build123d.build_enums import CenterOf, Mode, AngularDirection
from build123d.build_line import BuildLine
from build123d.geometry import Location, Vector
from build123d.objects_curve import Line, EllipticalCenterArc, Bezier, Polyline
from build123d.operations_generic import add, offset, mirror
from build123d.operations_sketch import make_face
from build123d.topology import (
    Vertex,
    Wire,
)

def import_svg_as_forced_outline(
    svg_file: Union[str, Path, TextIO],
    reorient: bool = True,
    duplicate_tolerance: float = 0.01,
    extra_cleaning=False,
    cleaning_tolerance: float = 0.01,
    simplify_beziers=False,
) -> Wire:
    """Import an SVG and apply cleaning operations to return a closed wire outline, if possible. Useful for SVG outlines that are actually made of thin shapes or slightly disconnected paths. May fail on more complex shapes.

    * Removes duplicate lines, including ones that are reverses of the other, within a tolerance level (useful for 'outlines' that are actually very thin shapes)
    * Sorts paths such that they are end to start in order of distance, flipping them if needed to line up start to end
    * Goes through each path and creates the next one such that it starts at the end of the last one
    * Ensures the last and first paths are connected


    Args:
        svg_file (Union[str, Path, TextIO]): svg file
        reorient (bool, optional): Center result on origin by bounding box, and
        flip objects to compensate for svg orientation (so the resulting wire
        is the same way up as it looks when opened in an SVG viewer). Defaults
        to True.
        duplicate_tolerance (float, optional): Amount of tolerance to use considering paths to be duplicates. Defaults to 0.01.
        extra_clean (bool, optional): Do some extra cleaning, mainly skipping tiny paths. Defaults to False.
        cleaning_tolerance (float, optional): Amount of tolerance to use discarding small paths if extra cleaning is used. Defaults to 0.01.

    Raises:
        ValueError: If an unknown path type is encountered.
        FileNotFoundError: the input file cannot be found.

    Returns:
        Wire: Forcefully connected SVG paths as a wire.
    """

    def point(path_point):
        return (path_point.real, path_point.imag)

    paths = svg.svg2paths(svg_file)[0]
    curves = []
    for p in paths:
        curves.extend(p)
    curves = _remove_duplicate_paths(curves, tolerance=duplicate_tolerance)
    curves = _sort_curves(curves)
    first_line = curves[0]
    previous_edge = None
    with BuildLine() as bd_l:
        line_start = Vector(point(first_line.start))
        for i, p in enumerate(curves):
            if extra_cleaning and p.length() < cleaning_tolerance:
                # Filter out tiny edges that may cause issues with OCCT ops
                continue
            line_end = point(p.end)
            if i == len(curves) - 1:
                # Forcefully reconnect the end to the start.
                # Note: This won't quite work if the last path is an arc,
                # but make_face should still sort it out. Once
                # EllipticalStartArc is released in build123d, this can be
                # fixed.
                line_end = point(first_line.start)
            else:
                if (
                    extra_cleaning
                    and Vertex(line_end).distance(Vertex(line_start)) < cleaning_tolerance
                ):
                    # Skip this path if it's really short, just go straight
                    # to the next one.
                    continue
            if isinstance(p, svg.Line):
                edge = Line(line_start, line_end)
                # if (
                #     extra_cleaning
                #     and previous_edge is not None
                #     and isinstance(previous_edge, Line)
                #     and abs(edge % 0 - previous_edge % 0) < duplicate_tolerance
                # ):
                #     # Merge straight lines that are split into multiple paths.
                #     add(previous_edge, mode=Mode.SUBTRACT)
                #     previous_edge = Line(previous_edge @ 0, line_end)
                #     edge = previous_edge
                #     add(edge)
            elif isinstance(p, svg.CubicBezier):
                pts = [line_start, point(p.control1), point(p.control2), line_end]
                if simplify_beziers:
                    # Splines seem to cause issues with offsetting or tapered extrusion, so we may have to approximate them with polylines.
                    edge = Polyline(*pts)
                else:
                    edge = Bezier(*pts)
            elif isinstance(p, svg.QuadraticBezier):
                print("Warning: this shape contais quadratic beziers. These are untested, and may fail to generate a valid case.")
                edge = Bezier(line_start, point(p.control), line_end)
            elif isinstance(p, svg.Arc):
                start, end = sorted(
                    [
                        p.theta,
                        p.theta + p.delta,
                    ]
                )
                if p.delta < 0.0:
                    dir_ = AngularDirection.CLOCKWISE
                else:
                    dir_ = AngularDirection.COUNTER_CLOCKWISE
                edge = EllipticalCenterArc(
                    center=point(p.center),
                    x_radius=p.radius.real,
                    y_radius=p.radius.imag,
                    start_angle=start,
                    end_angle=end,
                    rotation=degrees(p.phi),
                    angular_direction=dir_,
                    mode=Mode.PRIVATE,
                )
                to_move = line_start - edge @ 0
                edge = edge.moved(Location(to_move))
                add(edge)

            else:
                print("Unknown path type for ", p)
                raise ValueError
            line_start = edge @ 1
            previous_edge = edge
    face = make_face(bd_l.wire()).face()
    face = mirror(face)
    # Mirroring faces sometimes causes invalid geometry, but apparently this process of offsetting in then out 'cures' it.
    # https://github.com/gumyr/build123d/issues/719
    off = 0.01
    face = offset(-offset(face, off).face(), -off).face()
    if reorient:
        face = face.move(Location(-face.center(center_of=CenterOf.BOUNDING_BOX)))
    # Ensure face normal is up
    if face.normal_at().Z < 0:
        face = -face
    return face

    # edges = bd_l.edges()
    # for i, edge in enumerate(edges):
        # print(i, edges[i-1] @ 1 == edges[i] @ 0)

    wire = bd_l.wire()
    if reorient:
        wire = wire.move(Location(-wire.center(center_of=CenterOf.BOUNDING_BOX)))
        # Mirroring wires can introduce bad geometry!?
        wire = mirror(wire)

    # new = []
    # for edge in wire.edges():
    #     new.append(mirror(edge))
    # wire = Wire.combine(new).wire()

    return wire

def _mirror_around_center(shape, plane):
    shape = mirror(shape, around=plane.move(Location(shape.center(center_of=CenterOf.BOUNDING_BOX))))

def _center_obj(shape):
    return shape.move(Location(-shape.center(center_of=CenterOf.BOUNDING_BOX)))

def _sort_curves(curves):
    """Return list of paths sorted and flipped so that they are connected end to end as the list iterates."""
    if not curves:
        return []

    def euclidean_distance(p1, p2):
        return sqrt((p1.real - p2.real) ** 2 + (p1.imag - p2.imag) ** 2)

    # Start with the first curve
    sorted_curves = [curves.pop(0)]

    while curves:
        last_curve = sorted_curves[-1]
        last_end = last_curve.end

        # Find the closest curve to the previous end point.
        closest_curve, closest_distance, flip = None, float("inf"), False
        for curve in curves:
            dist_start = euclidean_distance(last_end, curve.start)
            dist_end = euclidean_distance(last_end, curve.end)
            # If end is closer than start, flip the curve right way around.
            if dist_start < closest_distance:
                closest_curve, closest_distance, flip = curve, dist_start, False
            if dist_end < closest_distance:
                closest_curve, closest_distance, flip = curve, dist_end, True

        # Flip the curve if necessary
        if flip:
            flipped = _reverse_svg_curve(closest_curve)
            sorted_curves.append(flipped)
        else:
            sorted_curves.append(closest_curve)
        curves.remove(closest_curve)

    return sorted_curves


def _remove_duplicate_paths(paths, tolerance=0.01):
    """Remove paths that are identical to within the given positional and
    parameter tolerance limit, including similar but reversed paths."""
    cleaned_paths = []

    for _, path in enumerate(paths):
        if path.length() == 0:
            # Skip zero-length paths
            continue
        # Check if a similar path already exists in the cleaned list (either
        # forward or reversed)
        flipped = _reverse_svg_curve(path)
        for _, cleaned_path in enumerate(cleaned_paths):
            if (
                _are_paths_similar(path, cleaned_path, tolerance) or
                _are_paths_similar(flipped, cleaned_path, tolerance)
            ):
                # Skip this path if a similar one is already in the list
                break
        else:
            cleaned_paths.append(path)

    return cleaned_paths


def _are_paths_similar(path1, path2, tolerance=0.01):
    """Compares two SVG paths, based on type, start/end points, length, and Arc attributes."""

    if type(path1) != type(path2):
        return False

    def lengths_are_close(p1, p2):
        return (
            abs(p1.length() - p2.length()) / max(p1.length(), p2.length()) < tolerance
        )

    if not lengths_are_close(path1, path2):
        return False

    def points_are_close(p1, p2):
        return abs(p1.real - p2.real) < tolerance and abs(p1.imag - p2.imag) < tolerance

    if not (points_are_close(path1.start, path2.start) and points_are_close(
            path1.end, path2.end
        )):
        return False

    # Additional checks for arcs (to handle radius, rotation, etc.)
    if isinstance(path1, svg.Arc) and isinstance(path2, svg.Arc):
        arc_attributes = [
            "radius",
            "phi",
            "theta",
            "delta",
            "rotation",
            "center",
            "large_arc",
            "sweep",
        ]

        for attr in arc_attributes:
            try:
                if abs(vars(path1)[attr] - vars(path2)[attr]) > tolerance:
                    return False
            except KeyError:
                continue

    return True


def _reverse_svg_curve(c):
    c = copy.deepcopy(c)
    t = c.start
    c.start = c.end
    c.end = t
    if isinstance(c, svg.Arc):
        # Flipping ElipticalArcs is a bit more complicated.
        # Calculate the new theta as the original end angle.
        new_theta = c.theta + c.delta
        # Reverse the delta.
        c.delta = -c.delta
        # Set theta to the new start angle.
        c.theta = new_theta
    return c


if "__file__" in globals():
    script_dir = Path(__file__).parent
else:
    script_dir = Path(os.getcwd())


# For debugging/viewing in cq-editor or vscode's ocp_vscode plugin.
if __name__ not in ["__cq_main__", "temp"]:
    show_object = lambda *_, **__: None
    log = lambda x: print(x)
    # show_object = lambda *_, **__: None

    if __name__ == "__main__":
        import ocp_vscode as ocp
        from ocp_vscode import show

        ocp.set_port(3939)
        ocp.set_defaults(reset_camera=ocp.Camera.KEEP)
        show_object = lambda *args, **__: ocp.show(args)

        p = Path(
            "~/src/keyboard_design/maizeless/pcb/build/maizeless-Edge_Cuts gerber.svg"
        ).expanduser()
        p = script_dir / "../manual_outlines/ferris-base-0.1.svg"
        # p = script_dir / "build/outline.svg"
        # p = Path("~/src/keeb_snakeskin/manual_outlines/ferris-base-0.1.svg").expanduser()

        import build123d as bd
        base_face = bd.make_face(
            import_svg_as_forced_outline(
                p,
                cleaning_tolerance=0.05,
                extra_cleaning=True,
                duplicate_tolerance=0.1,
                simplify_bezier=True,
            )
        )
        show_object(base_face, name="base_face")
