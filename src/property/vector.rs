use std::ops::{Add, Sub, Mul, Div};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector2D(pub i32, pub i32);

impl Vector2D {
    pub fn new(x: i32, y: i32) -> Vector2D {
        Vector2D(x, y)
    }
    pub fn distance(&self, other: &Vector2D) -> f64 {
        let x = (other.0 - self.0) as f64;
        let y = (other.1 - self.1) as f64;
        (x * x + y * y).sqrt()
    }
}

impl fmt::Display for Vector2D {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
    }
}

impl Add for Vector2D {
    type Output = Vector2D;

    fn add(self, other: Vector2D) -> Vector2D {
        Vector2D(self.0 + other.0, self.1 + other.1)
    }
}

impl Sub for Vector2D {
    type Output = Vector2D;

    fn sub(self, other: Vector2D) -> Vector2D {
        Vector2D(self.0 - other.0, self.1 - other.1)
    }
}

impl Mul for Vector2D {
    type Output = Vector2D;

    fn mul(self, other: Vector2D) -> Vector2D {
        Vector2D(self.0 * other.0, self.1 * other.1)
    }
}

impl Div for Vector2D {
    type Output = Vector2D;

    fn div(self, other: Vector2D) -> Vector2D {
        Vector2D(self.0 / other.0, self.1 / other.1)
    }
}