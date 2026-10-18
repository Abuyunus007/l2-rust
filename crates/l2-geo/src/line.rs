//! Bresenham line iterators over the geo grid.
//!
//! Java: `gameserver/geoengine/util/GridLineIterator2D.java` and `GridLineIterator3D.java`.
//!
//! Both visit the start point first and the end point last. Java's `next()` is [`advance`](GridLineIterator2D::advance)
//! here (it returns `true` while there is a point to read through `x()`, `y()`, `z()`), so a
//! call reads like the Java loop: `it.advance(); ... while it.advance() { ... }`. All arithmetic
//! wraps like Java `int`.

use std::cmp::Ordering;

/// `GridLineIterator2D`: the cells of a line from `(start_x, start_y)` to `(end_x, end_y)`.
#[derive(Clone, Debug)]
pub struct GridLineIterator2D {
    current_x: i32,
    current_y: i32,
    target_x: i32,
    target_y: i32,
    step_x: i32,
    step_y: i32,
    delta_x: i32,
    delta_y: i32,
    is_steep: bool,
    accumulated_error: i32,
    has_started: bool,
}

/// `Integer.compare(a, b)` as -1 / 0 / 1.
fn compare(a: i32, b: i32) -> i32 {
    match a.cmp(&b) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

impl GridLineIterator2D {
    pub fn new(start_x: i32, start_y: i32, end_x: i32, end_y: i32) -> Self {
        let delta_x = end_x.wrapping_sub(start_x).wrapping_abs();
        let delta_y = end_y.wrapping_sub(start_y).wrapping_abs();
        let is_steep = delta_y > delta_x;
        GridLineIterator2D {
            current_x: start_x,
            current_y: start_y,
            target_x: end_x,
            target_y: end_y,
            step_x: compare(end_x, start_x),
            step_y: compare(end_y, start_y),
            delta_x,
            delta_y,
            is_steep,
            // Half the dominant axis, so the first minor step happens in the middle.
            accumulated_error: (if is_steep { delta_y } else { delta_x }) / 2,
            has_started: false,
        }
    }

    /// `next()`: moves to the next point. The first call keeps the start point. Returns false once
    /// the end point has been passed.
    pub fn advance(&mut self) -> bool {
        if !self.has_started {
            self.has_started = true;
            return true;
        }
        if self.current_x == self.target_x && self.current_y == self.target_y {
            return false;
        }
        if self.is_steep {
            self.current_y = self.current_y.wrapping_add(self.step_y);
            self.accumulated_error = self.accumulated_error.wrapping_sub(self.delta_x);
            if self.accumulated_error < 0 {
                self.current_x = self.current_x.wrapping_add(self.step_x);
                self.accumulated_error = self.accumulated_error.wrapping_add(self.delta_y);
            }
        } else {
            self.current_x = self.current_x.wrapping_add(self.step_x);
            self.accumulated_error = self.accumulated_error.wrapping_sub(self.delta_y);
            if self.accumulated_error < 0 {
                self.current_y = self.current_y.wrapping_add(self.step_y);
                self.accumulated_error = self.accumulated_error.wrapping_add(self.delta_x);
            }
        }
        true
    }

    pub fn x(&self) -> i32 {
        self.current_x
    }

    pub fn y(&self) -> i32 {
        self.current_y
    }
}

/// `GridLineIterator3D`: a line in X, Y and Z; the axis with the largest delta advances every step.
#[derive(Clone, Debug)]
pub struct GridLineIterator3D {
    current_x: i32,
    current_y: i32,
    current_z: i32,
    target_x: i32,
    target_y: i32,
    target_z: i32,
    delta_x: i32,
    delta_y: i32,
    delta_z: i32,
    step_x: i32,
    step_y: i32,
    step_z: i32,
    accumulated_error_xy: i32,
    accumulated_error_xz: i32,
    has_started: bool,
}

impl GridLineIterator3D {
    pub fn new(start_x: i32, start_y: i32, start_z: i32, end_x: i32, end_y: i32, end_z: i32) -> Self {
        let delta_x = end_x.wrapping_sub(start_x).wrapping_abs();
        let delta_y = end_y.wrapping_sub(start_y).wrapping_abs();
        let delta_z = end_z.wrapping_sub(start_z).wrapping_abs();
        // NOTE(java-quirk): the step is -1 (never 0) on an axis that does not change; the error
        // terms keep that axis from moving.
        let initial_error = if delta_x >= delta_y && delta_x >= delta_z {
            delta_x / 2
        } else if delta_y >= delta_x && delta_y >= delta_z {
            delta_y / 2
        } else {
            delta_z / 2
        };
        GridLineIterator3D {
            current_x: start_x,
            current_y: start_y,
            current_z: start_z,
            target_x: end_x,
            target_y: end_y,
            target_z: end_z,
            delta_x,
            delta_y,
            delta_z,
            step_x: if start_x < end_x { 1 } else { -1 },
            step_y: if start_y < end_y { 1 } else { -1 },
            step_z: if start_z < end_z { 1 } else { -1 },
            accumulated_error_xy: initial_error,
            accumulated_error_xz: initial_error,
            has_started: false,
        }
    }

    /// `next()`: moves to the next point (see [`GridLineIterator2D::advance`]).
    pub fn advance(&mut self) -> bool {
        if !self.has_started {
            self.has_started = true;
            return true;
        }
        if self.current_x == self.target_x
            && self.current_y == self.target_y
            && self.current_z == self.target_z
        {
            return false;
        }

        if self.delta_x >= self.delta_y && self.delta_x >= self.delta_z {
            // X is the dominant axis.
            self.current_x = self.current_x.wrapping_add(self.step_x);
            self.accumulated_error_xy = self.accumulated_error_xy.wrapping_add(self.delta_y);
            if self.accumulated_error_xy >= self.delta_x {
                self.current_y = self.current_y.wrapping_add(self.step_y);
                self.accumulated_error_xy = self.accumulated_error_xy.wrapping_sub(self.delta_x);
            }
            self.accumulated_error_xz = self.accumulated_error_xz.wrapping_add(self.delta_z);
            if self.accumulated_error_xz >= self.delta_x {
                self.current_z = self.current_z.wrapping_add(self.step_z);
                self.accumulated_error_xz = self.accumulated_error_xz.wrapping_sub(self.delta_x);
            }
        } else if self.delta_y >= self.delta_x && self.delta_y >= self.delta_z {
            // Y is the dominant axis.
            self.current_y = self.current_y.wrapping_add(self.step_y);
            self.accumulated_error_xy = self.accumulated_error_xy.wrapping_add(self.delta_x);
            if self.accumulated_error_xy >= self.delta_y {
                self.current_x = self.current_x.wrapping_add(self.step_x);
                self.accumulated_error_xy = self.accumulated_error_xy.wrapping_sub(self.delta_y);
            }
            self.accumulated_error_xz = self.accumulated_error_xz.wrapping_add(self.delta_z);
            if self.accumulated_error_xz >= self.delta_y {
                self.current_z = self.current_z.wrapping_add(self.step_z);
                self.accumulated_error_xz = self.accumulated_error_xz.wrapping_sub(self.delta_y);
            }
        } else {
            // Z is the dominant axis. The two error terms track X and Y here.
            self.current_z = self.current_z.wrapping_add(self.step_z);
            self.accumulated_error_xy = self.accumulated_error_xy.wrapping_add(self.delta_x);
            if self.accumulated_error_xy >= self.delta_z {
                self.current_x = self.current_x.wrapping_add(self.step_x);
                self.accumulated_error_xy = self.accumulated_error_xy.wrapping_sub(self.delta_z);
            }
            self.accumulated_error_xz = self.accumulated_error_xz.wrapping_add(self.delta_y);
            if self.accumulated_error_xz >= self.delta_z {
                self.current_y = self.current_y.wrapping_add(self.step_y);
                self.accumulated_error_xz = self.accumulated_error_xz.wrapping_sub(self.delta_z);
            }
        }
        true
    }

    pub fn x(&self) -> i32 {
        self.current_x
    }

    pub fn y(&self) -> i32 {
        self.current_y
    }

    pub fn z(&self) -> i32 {
        self.current_z
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points_2d(sx: i32, sy: i32, ex: i32, ey: i32) -> Vec<(i32, i32)> {
        let mut it = GridLineIterator2D::new(sx, sy, ex, ey);
        let mut out = Vec::new();
        while it.advance() {
            out.push((it.x(), it.y()));
        }
        out
    }

    fn points_3d(s: (i32, i32, i32), e: (i32, i32, i32)) -> Vec<(i32, i32, i32)> {
        let mut it = GridLineIterator3D::new(s.0, s.1, s.2, e.0, e.1, e.2);
        let mut out = Vec::new();
        while it.advance() {
            out.push((it.x(), it.y(), it.z()));
        }
        out
    }

    #[test]
    fn same_point_visits_it_once() {
        assert_eq!(points_2d(3, -4, 3, -4), vec![(3, -4)]);
        assert_eq!(points_3d((1, 2, 3), (1, 2, 3)), vec![(1, 2, 3)]);
    }

    #[test]
    fn axis_aligned_and_diagonal() {
        assert_eq!(points_2d(0, 0, 3, 0), vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
        assert_eq!(points_2d(0, 0, 0, -2), vec![(0, 0), (0, -1), (0, -2)]);
        assert_eq!(points_2d(2, 2, -1, -1), vec![(2, 2), (1, 1), (0, 0), (-1, -1)]);
    }

    #[test]
    fn shallow_line_steps_the_minor_axis_in_the_middle() {
        // dx = 4, dy = 1: error starts at 2; after x+1: 1, x+2: 0, x+3: -1 -> y steps at the 3rd step.
        assert_eq!(
            points_2d(0, 0, 4, 1),
            vec![(0, 0), (1, 0), (2, 0), (3, 1), (4, 1)]
        );
        // Steep: dy = 4, dx = 1.
        assert_eq!(
            points_2d(0, 0, 1, 4),
            vec![(0, 0), (0, 1), (0, 2), (1, 3), (1, 4)]
        );
    }

    #[test]
    fn negative_coordinates_are_plain_integers() {
        let p = points_2d(-3, -3, -9, -4);
        assert_eq!(p.first(), Some(&(-3, -3)));
        assert_eq!(p.last(), Some(&(-9, -4)));
        assert_eq!(p.len(), 7);
    }

    #[test]
    fn three_d_dominant_axis_and_endpoints() {
        let p = points_3d((0, 0, 0), (4, 2, 1));
        assert_eq!(p.len(), 5);
        assert_eq!(p.first(), Some(&(0, 0, 0)));
        assert_eq!(p.last(), Some(&(4, 2, 1)));
        // Z dominant.
        let p = points_3d((0, 0, 10), (1, 1, 0));
        assert_eq!(p.len(), 11);
        assert_eq!(p.last(), Some(&(1, 1, 0)));
    }
}
