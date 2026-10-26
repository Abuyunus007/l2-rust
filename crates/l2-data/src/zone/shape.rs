//! Zone shapes and their geometry queries.
//!
//! Java: `entity/zone/ZoneForm.java`, `entity/zone/form/{ZoneCuboid,ZoneNPoly,ZoneCylinder}.java`,
//! plus the three JDK classes they lean on: `java.awt.Rectangle`, `java.awt.Polygon` and the
//! `sun.awt.geom.Crossings.EvenOdd` helper behind `Polygon.intersects`.
//!
//! Java's edge behaviour is kept on purpose, because zone borders decide things like "is this
//! player in the town":
//! * a cuboid and a polygon contain their lower/left edge but not their upper/right edge
//!   (`Rectangle.contains`, `Polygon.contains`), a cylinder contains its rim;
//! * the Z band is inclusive on both ends;
//! * coordinates are `int`: differences wrap around like Java's, products (`rad * rad`, the
//!   polygon centre cross product) too. Distances and the containment maths are `double`.

use crate::player_template::Location;

/// `java.awt.Rectangle` as the cuboid zone uses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl Rect {
    /// `Rectangle.contains(int, int)` (`inside`): lower edge in, upper edge out.
    fn contains(&self, px: i32, py: i32) -> bool {
        let (mut w, mut h) = (self.width, self.height);
        if (w | h) < 0 {
            return false;
        }
        let (x, y) = (self.x, self.y);
        if px < x || py < y {
            return false;
        }
        w = w.wrapping_add(x);
        h = h.wrapping_add(y);
        // overflow || intersect
        (w < x || w > px) && (h < y || h > py)
    }

    /// `Rectangle2D.intersects(double, double, double, double)`, which is what `Rectangle` and
    /// `Polygon` call with the (widened) `int` arguments.
    fn intersects(&self, x: f64, y: f64, w: f64, h: f64) -> bool {
        if self.is_empty() || w <= 0.0 || h <= 0.0 {
            return false;
        }
        let x0 = f64::from(self.x);
        let y0 = f64::from(self.y);
        x + w > x0 && y + h > y0 && x < x0 + f64::from(self.width) && y < y0 + f64::from(self.height)
    }

    fn is_empty(&self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    /// `Rectangle2D.contains(double, double)`, used by `Polygon.contains` for its bounding box.
    fn contains_f64(&self, x: f64, y: f64) -> bool {
        let x0 = f64::from(self.x);
        let y0 = f64::from(self.y);
        x >= x0 && y >= y0 && x < x0 + f64::from(self.width) && y < y0 + f64::from(self.height)
    }

    fn max_x(&self) -> i32 {
        self.x.wrapping_add(self.width)
    }

    fn max_y(&self) -> i32 {
        self.y.wrapping_add(self.height)
    }
}

/// `java.awt.Polygon`: the vertex arrays and the bounding box it computes once.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Polygon {
    xs: Vec<i32>,
    ys: Vec<i32>,
    bounds: Rect,
}

impl Polygon {
    fn new(xs: Vec<i32>, ys: Vec<i32>) -> Self {
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for (&x, &y) in xs.iter().zip(&ys) {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        let bounds = Rect {
            x: min_x,
            y: min_y,
            width: max_x.wrapping_sub(min_x),
            height: max_y.wrapping_sub(min_y),
        };
        Self { xs, ys, bounds }
    }

    fn npoints(&self) -> usize {
        self.xs.len()
    }

    /// `Polygon.contains(double, double)`: even-odd ray test.
    fn contains(&self, x: f64, y: f64) -> bool {
        let n = self.npoints();
        if n <= 2 || !self.bounds.contains_f64(x, y) {
            return false;
        }
        let mut hits = 0u32;
        let mut lastx = self.xs[n - 1];
        let mut lasty = self.ys[n - 1];
        for i in 0..n {
            let curx = self.xs[i];
            let cury = self.ys[i];
            let (prevx, prevy) = (lastx, lasty);
            lastx = curx;
            lasty = cury;
            if cury == prevy {
                continue;
            }
            let leftx = if curx < prevx {
                if x >= f64::from(prevx) {
                    continue;
                }
                curx
            } else {
                if x >= f64::from(curx) {
                    continue;
                }
                prevx
            };
            let (test1, test2);
            if cury < prevy {
                if y < f64::from(cury) || y >= f64::from(prevy) {
                    continue;
                }
                if x < f64::from(leftx) {
                    hits += 1;
                    continue;
                }
                test1 = x - f64::from(curx);
                test2 = y - f64::from(cury);
            } else {
                if y < f64::from(prevy) || y >= f64::from(cury) {
                    continue;
                }
                if x < f64::from(leftx) {
                    hits += 1;
                    continue;
                }
                test1 = x - f64::from(prevx);
                test2 = y - f64::from(prevy);
            }
            if test1 < (test2 / f64::from(prevy.wrapping_sub(cury)) * f64::from(prevx.wrapping_sub(curx))) {
                hits += 1;
            }
        }
        (hits & 1) != 0
    }

    /// `Polygon.intersects(double, double, double, double)`.
    fn intersects(&self, x: f64, y: f64, w: f64, h: f64) -> bool {
        if self.npoints() == 0 || !self.bounds.intersects(x, y, w, h) {
            return false;
        }
        // Java's `getCrossings` returns null (= intersects) as soon as one edge is known to cross
        // the rectangle; otherwise the rectangle intersects when its left edge is inside.
        let mut cross = EvenOdd::new(x, y, x + w, y + h);
        let n = self.npoints();
        let mut lastx = self.xs[n - 1];
        let mut lasty = self.ys[n - 1];
        for i in 0..n {
            let (curx, cury) = (self.xs[i], self.ys[i]);
            if cross.accumulate_line(
                f64::from(lastx),
                f64::from(lasty),
                f64::from(curx),
                f64::from(cury),
            ) {
                return true;
            }
            lastx = curx;
            lasty = cury;
        }
        !cross.is_empty()
    }
}

/// `sun.awt.geom.Crossings.EvenOdd`: remembers the Y ranges of the rectangle's left edge that lie
/// inside the polygon, so "rectangle fully inside the polygon" is found without a vertex test.
struct EvenOdd {
    limit: usize,
    yranges: Vec<f64>,
    xlo: f64,
    ylo: f64,
    xhi: f64,
    yhi: f64,
}

impl EvenOdd {
    fn new(xlo: f64, ylo: f64, xhi: f64, yhi: f64) -> Self {
        Self {
            limit: 0,
            yranges: vec![0.0; 10],
            xlo,
            ylo,
            xhi,
            yhi,
        }
    }

    fn is_empty(&self) -> bool {
        self.limit == 0
    }

    fn accumulate_line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> bool {
        if y0 <= y1 {
            self.accumulate_ordered(x0, y0, x1, y1)
        } else {
            self.accumulate_ordered(x1, y1, x0, y0)
        }
    }

    /// Java also passes a direction (+1/-1) that `EvenOdd` ignores.
    fn accumulate_ordered(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> bool {
        if self.yhi <= y0 || self.ylo >= y1 {
            return false;
        }
        if x0 >= self.xhi && x1 >= self.xhi {
            return false;
        }
        if y0 == y1 {
            return x0 >= self.xlo || x1 >= self.xlo;
        }
        let dx = x1 - x0;
        let dy = y1 - y0;
        let (xstart, ystart) = if y0 < self.ylo {
            (x0 + (self.ylo - y0) * dx / dy, self.ylo)
        } else {
            (x0, y0)
        };
        let (xend, yend) = if self.yhi < y1 {
            (x0 + (self.yhi - y0) * dx / dy, self.yhi)
        } else {
            (x1, y1)
        };
        if xstart >= self.xhi && xend >= self.xhi {
            return false;
        }
        if xstart > self.xlo || xend > self.xlo {
            return true;
        }
        self.record(ystart, yend);
        false
    }

    fn record(&mut self, mut ystart: f64, mut yend: f64) {
        if ystart >= yend {
            return;
        }
        let mut from = 0;
        // Jump over all pairs that are completely above.
        while from < self.limit && ystart > self.yranges[from + 1] {
            from += 2;
        }
        let mut to = from;
        while from < self.limit {
            let yrlo = self.yranges[from];
            let yrhi = self.yranges[from + 1];
            from += 2;
            if yend < yrlo {
                // Insert the new range, carry the old one on.
                self.yranges[to] = ystart;
                self.yranges[to + 1] = yend;
                to += 2;
                ystart = yrlo;
                yend = yrhi;
                continue;
            }
            // The ranges overlap: sort, collapse, insert, iterate.
            let (yll, mut ylh) = if ystart < yrlo {
                (ystart, yrlo)
            } else {
                (yrlo, ystart)
            };
            let (mut yhl, yhh) = if yend < yrhi { (yend, yrhi) } else { (yrhi, yend) };
            if ylh == yhl {
                ystart = yll;
                yend = yhh;
            } else {
                if ylh > yhl {
                    ystart = yhl;
                    yhl = ylh;
                    ylh = ystart;
                }
                if yll != ylh {
                    self.yranges[to] = yll;
                    self.yranges[to + 1] = ylh;
                    to += 2;
                }
                ystart = yhl;
                yend = yhh;
            }
            if ystart >= yend {
                break;
            }
        }
        if to < from && from < self.limit {
            self.yranges.copy_within(from..self.limit, to);
        }
        to += self.limit - from;
        if ystart < yend {
            if to + 2 > self.yranges.len() {
                self.yranges.resize(to + 10, 0.0);
            }
            self.yranges[to] = ystart;
            self.yranges[to + 1] = yend;
            to += 2;
        }
        self.limit = to;
    }
}

/// A zone shape. Java `ZoneForm` with its three subclasses.
#[derive(Debug, Clone, PartialEq)]
pub enum ZoneForm {
    /// Java `ZoneCuboid`: a rectangle plus a Z band.
    Cuboid(Cuboid),
    /// Java `ZoneNPoly`: a polygon plus a Z band.
    NPoly(NPoly),
    /// Java `ZoneCylinder`: a circle plus a Z band.
    Cylinder(Cylinder),
}

/// Java `ZoneCuboid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cuboid {
    z1: i32,
    z2: i32,
    r: Rect,
    center: Location,
}

/// Java `ZoneNPoly`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NPoly {
    p: Polygon,
    z1: i32,
    z2: i32,
    center: Location,
}

/// Java `ZoneCylinder`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cylinder {
    x: i32,
    y: i32,
    z1: i32,
    z2: i32,
    rad: i32,
    rad_s: i32,
    center: Location,
}

impl ZoneForm {
    /// Java `new ZoneCuboid(x1, x2, y1, y2, z1, z2)`: the corners may come in any order.
    pub fn cuboid(x1: i32, x2: i32, y1: i32, y2: i32, z1: i32, z2: i32) -> Self {
        let (min_x, max_x) = (x1.min(x2), x1.max(x2));
        let (min_y, max_y) = (y1.min(y2), y1.max(y2));
        let r = Rect {
            x: min_x,
            y: min_y,
            width: max_x.wrapping_sub(min_x),
            height: max_y.wrapping_sub(min_y),
        };
        let (z1, z2) = (z1.min(z2), z1.max(z2));
        let center = Location {
            x: min_x.wrapping_add(max_x) / 2,
            y: min_y.wrapping_add(max_y) / 2,
            z: z1.wrapping_add(z2) / 2,
        };
        Self::Cuboid(Cuboid { z1, z2, r, center })
    }

    /// Java `new ZoneNPoly(x[], y[], z1, z2)`. `xs` and `ys` must have the same length and at
    /// least one vertex (the zone loader requires three).
    pub fn npoly(xs: Vec<i32>, ys: Vec<i32>, z1: i32, z2: i32) -> Self {
        debug_assert!(xs.len() == ys.len() && !xs.is_empty());
        let p = Polygon::new(xs, ys);
        let (z1, z2) = (z1.min(z2), z1.max(z2));
        let n = p.npoints();
        let mut area = 0.0_f64;
        let mut cx = 0.0_f64;
        let mut cy = 0.0_f64;
        for i in 0..n {
            let next = (i + 1) % n;
            // NOTE(java-quirk): the products are `int * int` in Java and overflow for large map
            // coordinates (about 46000 and up), so the centre is only approximate there. Kept.
            let cross = f64::from(
                p.xs[i]
                    .wrapping_mul(p.ys[next])
                    .wrapping_sub(p.xs[next].wrapping_mul(p.ys[i])),
            );
            area += cross;
            cx += f64::from(p.xs[i].wrapping_add(p.xs[next])) * cross;
            cy += f64::from(p.ys[i].wrapping_add(p.ys[next])) * cross;
        }
        area /= 2.0;
        if area == 0.0 {
            cx = 0.0;
            cy = 0.0;
            for i in 0..n {
                cx += f64::from(p.xs[i]);
                cy += f64::from(p.ys[i]);
            }
            cx /= n as f64;
            cy /= n as f64;
        } else {
            cx /= 6.0 * area;
            cy /= 6.0 * area;
        }
        // Rust's float to int `as` saturates and maps NaN to 0, exactly like Java's `(int)`.
        let center = Location {
            x: cx as i32,
            y: cy as i32,
            z: z1.wrapping_add(z2) / 2,
        };
        Self::NPoly(NPoly { p, z1, z2, center })
    }

    /// Java `new ZoneCylinder(x, y, z1, z2, rad)`. Unlike the other two shapes the Z values are
    /// not swapped into order.
    pub fn cylinder(x: i32, y: i32, z1: i32, z2: i32, rad: i32) -> Self {
        let center = Location {
            x,
            y,
            z: z1.wrapping_add(z2) / 2,
        };
        Self::Cylinder(Cylinder {
            x,
            y,
            z1,
            z2,
            rad,
            rad_s: rad.wrapping_mul(rad),
            center,
        })
    }

    /// Java `isInsideZone(x, y, z)`.
    pub fn is_inside_zone(&self, x: i32, y: i32, z: i32) -> bool {
        match self {
            Self::Cuboid(c) => c.r.contains(x, y) && z >= c.z1 && z <= c.z2,
            Self::NPoly(n) => n.p.contains(f64::from(x), f64::from(y)) && z >= n.z1 && z <= n.z2,
            Self::Cylinder(c) => {
                let dx = f64::from(c.x.wrapping_sub(x));
                let dy = f64::from(c.y.wrapping_sub(y));
                ((dx * dx) + (dy * dy)) <= f64::from(c.rad_s) && z >= c.z1 && z <= c.z2
            }
        }
    }

    /// Java `intersectsRectangle(x1, x2, y1, y2)`. Note the argument order: the two X values
    /// first, then the two Y values (the zone table calls it for its 32768-unit grid cells).
    pub fn intersects_rectangle(&self, ax1: i32, ax2: i32, ay1: i32, ay2: i32) -> bool {
        match self {
            Self::Cuboid(c) => c.r.intersects(
                f64::from(ax1.min(ax2)),
                f64::from(ay1.min(ay2)),
                f64::from(ax2.wrapping_sub(ax1).wrapping_abs()),
                f64::from(ay2.wrapping_sub(ay1).wrapping_abs()),
            ),
            Self::NPoly(n) => n.p.intersects(
                f64::from(ax1.min(ax2)),
                f64::from(ay1.min(ay2)),
                f64::from(ax2.wrapping_sub(ax1).wrapping_abs()),
                f64::from(ay2.wrapping_sub(ay1).wrapping_abs()),
            ),
            Self::Cylinder(c) => c.intersects_rectangle(ax1, ax2, ay1, ay2),
        }
    }

    /// Java `getDistanceToZone(x, y)`: to the nearest corner/vertex for cuboid and polygon (so
    /// it can be large for a point next to a long edge), to the rim for a cylinder (negative
    /// inside). Ignores Z.
    pub fn distance_to_zone(&self, x: i32, y: i32) -> f64 {
        match self {
            Self::Cuboid(c) => {
                let dx1 = f64::from(c.r.x.wrapping_sub(x));
                let dx2 = f64::from(c.r.max_x().wrapping_sub(x));
                let dy1 = f64::from(c.r.y.wrapping_sub(y));
                let dy2 = f64::from(c.r.max_y().wrapping_sub(y));
                let (dx1_sq, dx2_sq, dy1_sq, dy2_sq) = (dx1 * dx1, dx2 * dx2, dy1 * dy1, dy2 * dy2);
                let mut shortest = dx1_sq + dy1_sq;
                for test in [dx1_sq + dy2_sq, dx2_sq + dy1_sq, dx2_sq + dy2_sq] {
                    if test < shortest {
                        shortest = test;
                    }
                }
                shortest.sqrt()
            }
            Self::NPoly(n) => {
                let mut dx = f64::from(n.p.xs[0].wrapping_sub(x));
                let mut dy = f64::from(n.p.ys[0].wrapping_sub(y));
                let mut shortest = (dx * dx) + (dy * dy);
                for i in 1..n.p.npoints() {
                    dx = f64::from(n.p.xs[i].wrapping_sub(x));
                    dy = f64::from(n.p.ys[i].wrapping_sub(y));
                    let test = (dx * dx) + (dy * dy);
                    if test < shortest {
                        shortest = test;
                    }
                }
                shortest.sqrt()
            }
            Self::Cylinder(c) => {
                let dx = f64::from(c.x.wrapping_sub(x));
                let dy = f64::from(c.y.wrapping_sub(y));
                ((dx * dx) + (dy * dy)).sqrt() - f64::from(c.rad)
            }
        }
    }

    /// Java `getLowZ()`.
    pub fn low_z(&self) -> i32 {
        match self {
            Self::Cuboid(c) => c.z1,
            Self::NPoly(n) => n.z1,
            Self::Cylinder(c) => c.z1,
        }
    }

    /// Java `getHighZ()`.
    pub fn high_z(&self) -> i32 {
        match self {
            Self::Cuboid(c) => c.z2,
            Self::NPoly(n) => n.z2,
            Self::Cylinder(c) => c.z2,
        }
    }

    /// Java `getCenterPoint()`.
    pub fn center_point(&self) -> Location {
        match self {
            Self::Cuboid(c) => c.center,
            Self::NPoly(n) => n.center,
            Self::Cylinder(c) => c.center,
        }
    }

    /// `(z1 + z2) / 2`: the Z guess `getRandomPoint()` gives the geoengine.
    pub fn mid_z(&self) -> i32 {
        self.center_point().z
    }

    /// Smallest box `(min_x, max_x, min_y, max_y)` that holds the whole shape. The zone table
    /// uses it to skip grid cells that cannot intersect; not a Java method.
    pub fn bounds(&self) -> (i32, i32, i32, i32) {
        match self {
            Self::Cuboid(c) => (c.r.x, c.r.max_x(), c.r.y, c.r.max_y()),
            Self::NPoly(n) => (n.p.bounds.x, n.p.bounds.max_x(), n.p.bounds.y, n.p.bounds.max_y()),
            Self::Cylinder(c) => (
                c.x.saturating_sub(c.rad),
                c.x.saturating_add(c.rad),
                c.y.saturating_sub(c.rad),
                c.y.saturating_add(c.rad),
            ),
        }
    }

    /// Java `visualizeZone(z)` without the item drops: the X/Y of every debug item along the
    /// border, in the order Java drops them (`STEP` = 10 units apart). The caller drops an item
    /// at each point.
    pub fn visualize_points(&self) -> Vec<(i32, i32)> {
        const STEP: i32 = 10;
        let mut points = Vec::new();
        match self {
            Self::Cuboid(c) => {
                let (x1, x2) = (c.r.x, c.r.x.wrapping_add(c.r.width));
                let (y1, y2) = (c.r.y, c.r.y.wrapping_add(c.r.height));
                // x1->x2
                let mut x = x1;
                while x < x2 {
                    points.push((x, y1));
                    points.push((x, y2));
                    x = x.wrapping_add(STEP);
                }
                // y1->y2
                let mut y = y1;
                while y < y2 {
                    points.push((x1, y));
                    points.push((x2, y));
                    y = y.wrapping_add(STEP);
                }
            }
            Self::Cylinder(c) => {
                let count = ((2.0 * std::f64::consts::PI * f64::from(c.rad)) / f64::from(STEP)) as i32;
                let angle = (2.0 * std::f64::consts::PI) / f64::from(count);
                for i in 0..count {
                    let a = angle * f64::from(i);
                    points.push((
                        c.x.wrapping_add((a.cos() * f64::from(c.rad)) as i32),
                        c.y.wrapping_add((a.sin() * f64::from(c.rad)) as i32),
                    ));
                }
            }
            Self::NPoly(n) => {
                let count = n.p.xs.len();
                for i in 0..count {
                    let next = if i + 1 == count { 0 } else { i + 1 };
                    let vx = n.p.xs[next].wrapping_sub(n.p.xs[i]);
                    let vy = n.p.ys[next].wrapping_sub(n.p.ys[i]);
                    let squared = vx.wrapping_mul(vx).wrapping_add(vy.wrapping_mul(vy));
                    // `(float) Math.sqrt(..) / STEP`: single precision, like Java.
                    let length = (f64::from(squared).sqrt() as f32) / STEP as f32;
                    let mut o = 1;
                    while o as f32 <= length {
                        let x = n.p.xs[i] as f32 + ((o as f32 / length) * vx as f32);
                        let y = n.p.ys[i] as f32 + ((o as f32 / length) * vy as f32);
                        points.push((x as i32, y as i32));
                        o += 1;
                    }
                }
            }
        }
        points
    }

    /// Java `getRandomPoint()` without the Z lookup: the X/Y of a random point in the shape.
    /// `rnd(min, max)` must behave like `Rnd.get(min, max)` (both ends inclusive). The caller
    /// asks the geoengine for the Z, with [`mid_z`](Self::mid_z) as the guess.
    pub fn random_xy(&self, rnd: &mut dyn FnMut(i32, i32) -> i32) -> (i32, i32) {
        match self {
            Self::Cuboid(c) => {
                let x = rnd(c.r.x, c.r.max_x());
                let y = rnd(c.r.y, c.r.max_y());
                (x, y)
            }
            Self::NPoly(n) => {
                let b = &n.p.bounds;
                let mut x = rnd(b.x, b.max_x());
                let mut y = rnd(b.y, b.max_y());
                let mut anti_blocker = 0;
                // NOTE(java-quirk): after 1000 misses the last point is returned even though it
                // is outside the polygon.
                while !n.p.contains(f64::from(x), f64::from(y)) {
                    anti_blocker += 1;
                    if anti_blocker > 1000 {
                        break;
                    }
                    x = rnd(b.x, b.max_x());
                    y = rnd(b.y, b.max_y());
                }
                (x, y)
            }
            Self::Cylinder(c) => {
                let (mut x, mut y) = (0, 0);
                let (x2, y2) = (c.x.wrapping_sub(c.rad), c.y.wrapping_sub(c.rad));
                let (x3, y3) = (c.x.wrapping_add(c.rad), c.y.wrapping_add(c.rad));
                // Java starts from (0, 0), so the loop body runs only when that is outside.
                let mut dx = f64::from(c.x.wrapping_sub(x));
                let mut dy = f64::from(c.y.wrapping_sub(y));
                while ((dx * dx) + (dy * dy)) > f64::from(c.rad_s) {
                    x = rnd(x2, x3);
                    y = rnd(y2, y3);
                    dx = f64::from(c.x.wrapping_sub(x));
                    dy = f64::from(c.y.wrapping_sub(y));
                }
                (x, y)
            }
        }
    }
}

impl Cylinder {
    fn intersects_rectangle(&self, ax1: i32, ax2: i32, ay1: i32, ay2: i32) -> bool {
        let (x, y, rad) = (self.x, self.y, self.rad);
        let rad_s = f64::from(self.rad_s);
        // The circle's centre inside the rectangle?
        if x > ax1 && x < ax2 && y > ay1 && y < ay2 {
            return true;
        }
        // Any corner of the rectangle inside the circle?
        let dx1 = f64::from(ax1.wrapping_sub(x));
        let dy1 = f64::from(ay1.wrapping_sub(y));
        let dx1_sq = dx1 * dx1;
        let dy1_sq = dy1 * dy1;
        if (dx1_sq + dy1_sq) < rad_s {
            return true;
        }
        let dy2 = f64::from(ay2.wrapping_sub(y));
        let dy2_sq = dy2 * dy2;
        if (dx1_sq + dy2_sq) < rad_s {
            return true;
        }
        let dx2 = f64::from(ax2.wrapping_sub(x));
        let dx2_sq = dx2 * dx2;
        if (dx2_sq + dy1_sq) < rad_s {
            return true;
        }
        if (dx2_sq + dy2_sq) < rad_s {
            return true;
        }
        // The circle crosses a side of the rectangle?
        if x > ax1 && x < ax2 {
            if y.wrapping_sub(ay2).wrapping_abs() < rad {
                return true;
            }
            if y.wrapping_sub(ay1).wrapping_abs() < rad {
                return true;
            }
        }
        if y > ay1 && y < ay2 {
            if x.wrapping_sub(ax2).wrapping_abs() < rad {
                return true;
            }
            if x.wrapping_sub(ax1).wrapping_abs() < rad {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> ZoneForm {
        ZoneForm::npoly(vec![0, 100, 100, 0], vec![0, 0, 100, 100], 10, 20)
    }

    #[test]
    fn cuboid_contains_lower_edge_not_upper_edge() {
        let c = ZoneForm::cuboid(100, 0, 200, 0, 30, -5);
        assert!(c.is_inside_zone(0, 0, -5));
        assert!(c.is_inside_zone(99, 199, 30));
        assert!(!c.is_inside_zone(100, 100, 0), "x = x2 is outside");
        assert!(!c.is_inside_zone(50, 200, 0), "y = y2 is outside");
        assert!(!c.is_inside_zone(-1, 0, 0));
        assert!(!c.is_inside_zone(50, 50, 31));
        assert!(!c.is_inside_zone(50, 50, -6));
        assert_eq!((c.low_z(), c.high_z()), (-5, 30));
        assert_eq!(c.center_point(), Location { x: 50, y: 100, z: 12 });
    }

    #[test]
    fn polygon_edges_follow_awt() {
        let p = square();
        assert!(p.is_inside_zone(0, 0, 10));
        assert!(p.is_inside_zone(50, 50, 20));
        assert!(p.is_inside_zone(99, 99, 15));
        assert!(!p.is_inside_zone(100, 50, 15), "right edge is outside");
        assert!(!p.is_inside_zone(50, 100, 15), "top edge is outside");
        assert!(!p.is_inside_zone(50, 50, 21));
        assert!(!p.is_inside_zone(-1, 50, 15));
    }

    #[test]
    fn concave_polygon() {
        // An L shape: the notch at the top right is outside.
        let l = ZoneForm::npoly(vec![0, 100, 100, 50, 50, 0], vec![0, 0, 50, 50, 100, 100], 0, 10);
        assert!(l.is_inside_zone(25, 75, 5));
        assert!(l.is_inside_zone(75, 25, 5));
        assert!(!l.is_inside_zone(75, 75, 5));
        assert!(!l.is_inside_zone(60, 60, 5));
    }

    #[test]
    fn cylinder_contains_its_rim() {
        let c = ZoneForm::cylinder(0, 0, -10, 10, 5);
        assert!(c.is_inside_zone(5, 0, 0));
        assert!(c.is_inside_zone(3, 4, 0), "3-4-5: exactly on the rim");
        assert!(!c.is_inside_zone(4, 4, 0));
        assert!(c.is_inside_zone(0, 0, 10));
        assert!(!c.is_inside_zone(0, 0, 11));
        assert_eq!(c.distance_to_zone(0, 0), -5.0);
        assert_eq!(c.distance_to_zone(8, 6), 5.0);
    }

    #[test]
    fn cylinder_keeps_unordered_z() {
        // Java does not swap z1/z2 for cylinders: a reversed band contains nothing.
        let c = ZoneForm::cylinder(0, 0, 10, -10, 5);
        assert!(!c.is_inside_zone(0, 0, 0));
        assert_eq!((c.low_z(), c.high_z()), (10, -10));
    }

    #[test]
    fn cuboid_and_polygon_distance_is_to_corners() {
        let c = ZoneForm::cuboid(0, 100, 0, 100, 0, 1);
        assert_eq!(c.distance_to_zone(50, 50), (50.0_f64 * 50.0 * 2.0).sqrt());
        assert_eq!(c.distance_to_zone(-3, -4), 5.0);
        let p = square();
        assert_eq!(p.distance_to_zone(103, 104), 5.0);
    }

    #[test]
    fn rectangle_intersection_uses_open_intervals() {
        let c = ZoneForm::cuboid(0, 100, 0, 100, 0, 1);
        assert!(c.intersects_rectangle(50, 150, 50, 150));
        assert!(c.intersects_rectangle(150, 50, 150, 50), "corners may be swapped");
        assert!(
            !c.intersects_rectangle(100, 200, 0, 100),
            "touching edges do not intersect"
        );
        assert!(!c.intersects_rectangle(-100, 0, 0, 100));
        assert!(
            !c.intersects_rectangle(50, 50, 0, 100),
            "a zero-width rectangle never intersects"
        );
    }

    #[test]
    fn polygon_rectangle_intersection() {
        let p = square();
        assert!(p.intersects_rectangle(50, 150, 50, 150), "overlap");
        assert!(
            p.intersects_rectangle(10, 20, 10, 20),
            "rectangle inside the polygon"
        );
        assert!(
            p.intersects_rectangle(-50, 200, -50, 200),
            "polygon inside the rectangle"
        );
        assert!(!p.intersects_rectangle(200, 300, 0, 100));
        assert!(!p.intersects_rectangle(100, 200, 0, 100), "touching");
        // Triangle: the bounding boxes overlap but the shapes do not.
        let t = ZoneForm::npoly(vec![0, 100, 0], vec![0, 0, 100], 0, 1);
        assert!(!t.intersects_rectangle(80, 100, 80, 100));
        assert!(t.intersects_rectangle(20, 40, 20, 40));
    }

    #[test]
    fn cylinder_rectangle_intersection() {
        let c = ZoneForm::cylinder(0, 0, 0, 1, 10);
        assert!(c.intersects_rectangle(-100, 100, -100, 100), "centre inside");
        assert!(c.intersects_rectangle(5, 100, 5, 100), "corner inside the circle");
        assert!(
            c.intersects_rectangle(-1, 1, 5, 100),
            "circle crosses the lower side"
        );
        assert!(
            !c.intersects_rectangle(10, 100, -1, 1),
            "rim touches the side only"
        );
        assert!(!c.intersects_rectangle(8, 100, 8, 100), "corner at 11.3 > 10");
    }

    #[test]
    fn polygon_center_is_the_centroid() {
        let p = square();
        assert_eq!(p.center_point(), Location { x: 50, y: 50, z: 15 });
        // Zero area (a line): the average of the vertices.
        let line = ZoneForm::npoly(vec![0, 10, 20], vec![0, 10, 20], 0, 0);
        assert_eq!(line.center_point(), Location { x: 10, y: 10, z: 0 });
    }

    #[test]
    fn random_points_stay_inside() {
        let mut seed = 12345u64;
        let mut rnd = |lo: i32, hi: i32| {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            lo + ((seed >> 33) % u64::try_from(hi - lo + 1).unwrap()) as i32
        };
        let poly = square();
        let cyl = ZoneForm::cylinder(10, 10, 0, 1, 30);
        for _ in 0..200 {
            let (x, y) = poly.random_xy(&mut rnd);
            assert!(poly.is_inside_zone(x, y, poly.mid_z()), "{x},{y}");
            let (x, y) = cyl.random_xy(&mut rnd);
            assert!(cyl.is_inside_zone(x, y, cyl.mid_z()), "{x},{y}");
            let (x, y) = ZoneForm::cuboid(0, 50, 0, 70, 0, 1).random_xy(&mut rnd);
            assert!((0..=50).contains(&x) && (0..=70).contains(&y));
        }
    }

    #[test]
    fn visualize_points_follow_the_java_loops() {
        // 25 x 15: x1->x2 gives 3 pairs (0, 10, 20), y1->y2 gives 2 pairs (0, 10).
        let c = ZoneForm::cuboid(0, 25, 0, 15, 0, 1);
        let points = c.visualize_points();
        assert_eq!(points.len(), 10);
        assert_eq!(&points[..4], &[(0, 0), (0, 15), (10, 0), (10, 15)]);
        assert_eq!(&points[6..], &[(0, 0), (25, 0), (0, 10), (25, 10)]);

        // Circle of radius 100: (int)(2 pi 100 / 10) = 62 points, the first on the +x axis.
        let circle = ZoneForm::cylinder(1000, 2000, 0, 1, 100);
        let points = circle.visualize_points();
        assert_eq!(points.len(), 62);
        assert_eq!(points[0], (1100, 2000));

        // The square polygon: each side is 100 long, 10 points each, the last on the corner.
        let points = square().visualize_points();
        assert_eq!(points.len(), 40);
        assert_eq!(points[0], (10, 0));
        assert_eq!(points[9], (100, 0));
        assert_eq!(points[39], (0, 0));
    }
}
