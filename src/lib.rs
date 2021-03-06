// Copyright (C) 2020 Nathaniel Edgar
// nathaniel.edgar.fl@gmail.com
// GPL-3.0-or-later
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Provides the Infinint struct, a semi-infinite-precision integer type. Infinint is written to be
//! space-efficient - one nybble is required per decimal digit - but sacrifices some performance
//! compared to primitive integer types. More details on implementation are contained in the
//! Infinint struct documentation.

// TODO: arithmetic
// TODO: assignment
// TODO: to/from string
// TODO: to/from bitstream?
// TODO: add credit to
// - https://crates.io/crates/num-bigint
// - https://crates.io/crates/ramp
// - http://darbinding.sourceforge.net/libdarc/group__infinint.html
// TODO: check for feature completeness from bigint/ramp
// TODO: update docs to match crates like bigint
// TODO: write comparison to other bigint crates:
// - no unsafe code
// - compact representation
// - readable ints

use std::{cmp, fmt, ops};

/// A semi-infinite-precision integer type.
///
/// # Examples
/// ```rust
/// # use infinint::Infinint;
/// let a = Infinint::new();
/// assert_eq!(a.negative(), false);
/// assert_eq!(a.digits(), [0]);
///
/// let b = Infinint::from(123_456);
/// assert_eq!(b.digits(), [6, 5, 4, 3, 2, 1]);
///
/// // add operators when supported
/// ```
///
/// # Implementation
/// The `Infinint` struct contains two elements: a boolean to identify the integer as positive or
/// negative, and a vector of bytes containing the decimal digits. Each byte in the vector holds
/// data for two decimal digits: one in the upper nybble, and one in the lower. The decimal digits
/// are stored in little-endian order.
///
/// For example, the decimal number `1998` would be stored as the following two bytes:
/// ```lang-none
/// [1000_1001, 1001_0001]
/// ```
/// which represent these decimal digit pairs:
/// ```lang-none
/// [(8, 9), (9, 1)]
/// ```
///
/// If the number of decimal digits is uneven, the lower nybble of the final byte will be 0. For
/// example:
/// ```lang-none
/// 137 = [0111_0011, 0001_0000] = [(7, 3), (1, 0)]
/// ```
pub struct Infinint {
    negative: bool,
    digits_vec: Vec<u8>,
}

#[allow(dead_code)]
impl Infinint {
    /// Initializes a new Infinint with the value +0.
    ///
    /// # Examples
    /// ```rust
    /// # use infinint::Infinint;
    /// let x = Infinint::new();
    /// ```
    pub fn new() -> Infinint {
        Infinint {
            negative: false,
            digits_vec: vec![0],
        }
    }

    /// Returns a boolean indicating if the Infinint is negative.
    ///
    /// # Examples
    /// ```rust
    /// # use infinint::Infinint;
    /// let x = Infinint::from(-3);
    /// let n = x.negative();
    /// assert_eq!(n, true);
    /// ```
    pub fn negative(&self) -> bool {
        self.negative
    }

    /// Returns a vector where each element is a single digit of the Infinint.
    ///
    /// As with the underlying data, the digits are returned in little-endian order.
    ///
    /// # Examples
    /// ```rust
    /// # use infinint::Infinint;
    /// let x = Infinint::from(1998);
    /// let d = x.digits();
    /// assert_eq!(d, [8, 9, 9, 1]);
    /// ```
    pub fn digits(&self) -> Vec<u8> {
        // initialize return value
        // length is capped at 2 * internal vector length since there are max two decimal digits
        //   per byte/digits_vec element
        let mut digits = Vec::with_capacity(self.digits_vec.len() * 2);

        for byte in &self.digits_vec {
            let digit_pair = decimal_digits(*byte).unwrap();
            digits.push(digit_pair.0);
            digits.push(digit_pair.1);
        }
        match digits.last() {
            Some(d) if *d == 0 => digits.pop(),
            _ => None,
        };

        digits
    }

    fn digits_vec_from_int(n: u128) -> Vec<u8> {
        let mut n = n;

        let bytes_needed = match n {
            0 => 1,
            _ => (((n as f64).abs().log10()) as usize / 2) + 1,
        };
        let next_exp = (bytes_needed as f64).log2().ceil();
        let next_pow_of_two = 2_i128.pow(next_exp as u32);
        let mut digits_vec: Vec<u8> = Vec::with_capacity(next_pow_of_two as usize);

        if n > 0 {
            while n > 0 {
                let n_mod = (n % 10) as u8;
                let d = n_mod << 4;
                n /= 10;

                let n_mod = (n % 10) as u8;
                let d = n_mod | d;
                n /= 10;

                digits_vec.push(d);
            }
        } else {
            digits_vec.push(0);
        }

        digits_vec
    }

    fn cmp_digits(n_digits_vec: &Vec<u8>, m_digits_vec: &Vec<u8>) -> cmp::Ordering {
        let mut self_iter = n_digits_vec.iter().rev();
        let mut other_iter = m_digits_vec.iter().rev();

        loop {
            let n_next_digits = *self_iter.next().unwrap_or(&0);
            let m_next_digits = *other_iter.next().unwrap_or(&0);

            if n_next_digits == 0 && m_next_digits == 0 {
                return cmp::Ordering::Equal;
            }

            let n_next_digits = decimal_digits(n_next_digits).unwrap();
            let m_next_digits = decimal_digits(m_next_digits).unwrap();

            if n_next_digits.1 < m_next_digits.1 {
                return cmp::Ordering::Less;
            } else if n_next_digits.1 > m_next_digits.1 {
                return cmp::Ordering::Greater;
            }

            if n_next_digits.0 < m_next_digits.0 {
                return cmp::Ordering::Less;
            } else if n_next_digits.0 > m_next_digits.0 {
                return cmp::Ordering::Greater;
            }
        }
    }

    fn infinint_cmp(n: &Infinint, m: &Infinint, negate_n: bool, negate_m: bool) -> cmp::Ordering {
        let n_negative = if negate_n == false {
            n.negative
        } else {
            !n.negative
        };
        let m_negative = if negate_m == false {
            m.negative
        } else {
            !m.negative
        };

        if n_negative == true && m_negative == false {
            cmp::Ordering::Less
        } else if n_negative == false && m_negative == true {
            return cmp::Ordering::Greater;
        } else {
            if n.digits_vec.len() < m.digits_vec.len() {
                cmp::Ordering::Less
            } else if n.digits_vec.len() > m.digits_vec.len() {
                cmp::Ordering::Greater
            } else {
                let digits_ordering = Infinint::cmp_digits(&n.digits_vec, &m.digits_vec);

                if n_negative == true {
                    digits_ordering.reverse()
                } else {
                    digits_ordering
                }
            }
        }
    }

    fn op_digits(
        n_digits_vec: &Vec<u8>,
        m_digits_vec: &Vec<u8>,
        op: fn(u8, u8, u8) -> (u8, u8),
    ) -> Vec<u8> {
        let mut n_iter = n_digits_vec.iter();
        let mut m_iter = m_digits_vec.iter();
        let mut carry = 0;
        let mut result_digits_vec: Vec<u8> =
            Vec::with_capacity(cmp::max(n_digits_vec.capacity(), m_digits_vec.capacity()));

        let mut n_next_digits = *n_iter.next().unwrap_or(&0);
        let mut m_next_digits = *m_iter.next().unwrap_or(&0);

        while n_next_digits != 0 || m_next_digits != 0 {
            let n_digits = decimal_digits(n_next_digits).unwrap();
            let m_digits = decimal_digits(m_next_digits).unwrap();

            let (upper_result_digit, new_carry) = op(n_digits.0, m_digits.0, carry);
            carry = new_carry;

            let (lower_result_digit, new_carry) = op(n_digits.1, m_digits.1, carry);
            carry = new_carry;

            let result_digit = (upper_result_digit << 4) | lower_result_digit;
            result_digits_vec.push(result_digit);

            n_next_digits = *n_iter.next().unwrap_or(&0);
            m_next_digits = *m_iter.next().unwrap_or(&0);
        }

        // possible because:
        // - this does not apply to subtraction; we can guarantee carry == 0 at the end
        // - if the carry is not 0 after the last run of addition, this means there is overflow
        //     to a new digit; if there wasn't overflow, the second update of result_digit
        //     would capture the carry
        if carry > 0 {
            result_digits_vec.push(carry << 4);
        }

        if result_digits_vec.len() == 0 {
            result_digits_vec.push(0);
        }

        result_digits_vec
    }

    fn infinint_add(
        n: &Infinint,
        m: &Infinint,
        negate_n: bool,
        negate_m: bool,
        negate_result: bool,
    ) -> Infinint {
        let n_negative = if negate_n == false {
            n.negative
        } else {
            !n.negative
        };
        let m_negative = if negate_m == false {
            m.negative
        } else {
            !m.negative
        };

        if n_negative == false && m_negative == true {
            return Infinint::infinint_subtract(n, m, negate_n, !negate_m, negate_result);
        } else if n_negative == true && m_negative == false {
            return Infinint::infinint_subtract(m, n, negate_m, !negate_n, negate_result);
        } // otherwise, negative can be determined later

        let result_digits_vec =
            Infinint::op_digits(&n.digits_vec, &m.digits_vec, decimal_add_with_carry);

        let result_negative = if negate_result == false {
            n_negative
        } else {
            !n_negative
        };

        Infinint {
            negative: result_negative,
            digits_vec: result_digits_vec,
        }
    }

    fn infinint_subtract(
        n: &Infinint,
        m: &Infinint,
        negate_n: bool,
        negate_m: bool,
        negate_result: bool,
    ) -> Infinint {
        let n_negative = if negate_n == false {
            n.negative
        } else {
            !n.negative
        };
        let m_negative = if negate_m == false {
            m.negative
        } else {
            !m.negative
        };

        if n_negative == false && m_negative == true {
            return Infinint::infinint_add(n, m, negate_n, !negate_m, negate_result);
        } else if n_negative == true && m_negative == false {
            return Infinint::infinint_add(n, m, !negate_n, negate_m, !negate_result);
        } else if n_negative == true && m_negative == true {
            return Infinint::infinint_subtract(m, n, !negate_m, !negate_n, negate_result);
        }

        match Infinint::infinint_cmp(n, m, negate_n, negate_m) {
            cmp::Ordering::Less => {
                return Infinint::infinint_subtract(m, n, negate_m, negate_n, !negate_result);
            }
            cmp::Ordering::Equal => return Infinint::from(0),
            cmp::Ordering::Greater => (),
        }

        let result_digits_vec =
            Infinint::op_digits(&n.digits_vec, &m.digits_vec, decimal_subtract_with_carry);

        let result_negative = if negate_result == false { false } else { true };

        Infinint {
            negative: result_negative,
            digits_vec: result_digits_vec,
        }
    }
}

impl fmt::Debug for Infinint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\nnegative: {}\n", self.negative)?;
        write!(f, "{}", format!("digits: [\n"))?;
        self.digits_vec.iter()
            .cloned()
            .map(|d| (d, decimal_digits(d).unwrap()))
            .map(|(d, (lo, hi))| write!(f, "{}", format!(
                    "\t{:04b}_{:04b} -> ({}, {})\n",
                    (0xF0 & d) >> 4,
                    0xF & d,
                    lo,
                    hi))).collect::<std::fmt::Result>()?;
        write!(f, "]")
    }
}

impl fmt::Display for Infinint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let raw_digits = self.digits();
        let num_digits = raw_digits.len();
        let num_chars = num_digits
            + if !f.alternate() {
                (num_digits - 1) / 3
            } else {
                0
            };

        let number = raw_digits.iter()
                            .cloned()
                            .map(u8::into)
                            .map(|x: u32| std::char::from_digit(x, 10))
                            .flatten()
                            .rev();
        if !f.alternate() {
            let add_commas = |(i, x)| { 
                if (num_chars - i) % 3 == 0 { 
                    Some(',') 
                } else { 
                    None 
                }.into_iter().chain(std::iter::once(x))
            };
            let number = number.enumerate() // Default display, we insert commas where necessary by chaining an option with the current digit.
                     .flat_map(add_commas);
            f.pad_integral(!self.negative, "", &number.collect::<String>())
        } else {
            f.pad_integral(!self.negative, "", &number.collect::<String>())
        }
    }
}

impl From<u128> for Infinint {
    fn from(n: u128) -> Infinint {
        let digits_vec = Infinint::digits_vec_from_int(n);

        Infinint {
            negative: false,
            digits_vec,
        }
    }
}

impl From<i128> for Infinint {
    fn from(n: i128) -> Infinint {
        let negative = n < 0;
        let digits_vec = Infinint::digits_vec_from_int(n.abs() as u128);

        Infinint {
            negative,
            digits_vec,
        }
    }
}

impl From<usize> for Infinint {
    fn from(n: usize) -> Infinint {
        // since usize < u128, conversion is safe
        Infinint::from(n as u128)
    }
}

impl From<isize> for Infinint {
    fn from(n: isize) -> Infinint {
        // since isize < i128, conversion is safe
        Infinint::from(n as i128)
    }
}

impl From<u64> for Infinint {
    fn from(n: u64) -> Infinint {
        Infinint::from(u128::from(n))
    }
}

impl From<i64> for Infinint {
    fn from(n: i64) -> Infinint {
        Infinint::from(i128::from(n))
    }
}

impl From<u32> for Infinint {
    fn from(n: u32) -> Infinint {
        Infinint::from(u128::from(n))
    }
}

impl From<i32> for Infinint {
    fn from(n: i32) -> Infinint {
        Infinint::from(i128::from(n))
    }
}

impl From<u16> for Infinint {
    fn from(n: u16) -> Infinint {
        Infinint::from(u128::from(n))
    }
}

impl From<i16> for Infinint {
    fn from(n: i16) -> Infinint {
        Infinint::from(i128::from(n))
    }
}

impl From<u8> for Infinint {
    fn from(n: u8) -> Infinint {
        Infinint::from(u128::from(n))
    }
}

impl From<i8> for Infinint {
    fn from(n: i8) -> Infinint {
        Infinint::from(i128::from(n))
    }
}

impl cmp::Ord for Infinint {
    fn cmp(&self, other: &Infinint) -> cmp::Ordering {
        Infinint::infinint_cmp(self, other, false, false)
    }
}

impl cmp::PartialOrd for Infinint {
    fn partial_cmp(&self, other: &Infinint) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::Eq for Infinint {}

impl cmp::PartialEq for Infinint {
    fn eq(&self, other: &Infinint) -> bool {
        self.cmp(other) == cmp::Ordering::Equal
    }
}

impl ops::Neg for &Infinint {
    type Output = Infinint;

    fn neg(self) -> Infinint {
        let new_negative = !self.negative;
        Infinint {
            negative: new_negative,
            digits_vec: self.digits_vec.to_vec(),
        }
    }
}

impl ops::Add<&Infinint> for &Infinint {
    type Output = Infinint;
    fn add(self, other: &Infinint) -> Infinint {
        Infinint::infinint_add(self, other, false, false, false)
    }
}

impl ops::Sub<&Infinint> for &Infinint {
    type Output = Infinint;

    fn sub(self, other: &Infinint) -> Infinint {
        Infinint::infinint_subtract(self, other, false, false, false)
    }
}

fn decimal_digits(n: u8) -> Result<(u8, u8), &'static str> {
    let high = decimal_digit_high(n)?;
    let low = decimal_digit_low(n)?;
    Ok((high, low))
}

fn decimal_digit_high(n: u8) -> Result<u8, &'static str> {
    decimal_digit_nybble((0xF0 & n) >> 4)
}

fn decimal_digit_low(n: u8) -> Result<u8, &'static str> {
    decimal_digit_nybble(0x0F & n)
}

fn decimal_digit_nybble(n: u8) -> Result<u8, &'static str> {
    if n < 10 {
        Ok(n)
    } else {
        Err("digit too large")
    }
}

fn decimal_add_with_carry(n: u8, m: u8, carry: u8) -> (u8, u8) {
    let result = n + m + carry;
    let carry = result / 10;
    let result = result % 10;
    (result, carry)
}

fn decimal_subtract_with_carry(n: u8, m: u8, carry: u8) -> (u8, u8) {
    let (result, carry) = if n >= (m + carry) {
        (n - m - carry, 0)
    } else {
        ((n + 10) - m - carry, 1)
    };
    (result, carry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infinint_declaration() {
        let test = Infinint::from(1998);
        assert_eq!(test.negative, false);
        assert_eq!(test.digits_vec, [0b1000_1001, 0b1001_0001]);
    }

    #[test]
    fn simple_addition_subtraction() {
        for x in 0..100 {
            for y in 0..100 {
                let a = Infinint::from(x);
                let b = Infinint::from(y);
                assert_eq!(&a + &b, Infinint::from(x + y));
                assert_eq!(&a - &b, Infinint::from(x - y));
            }
        }
    }

    #[test]
    fn complex_addition_subtraction() {
        for x in -25..25 {
            for y in -25..25 {
                let a = Infinint::from(x);
                let b = Infinint::from(y);
                assert_eq!(&a + &b, Infinint::from(x + y));
                assert_eq!(&a - &b, Infinint::from(x - y));
            }
        }
    }
}
