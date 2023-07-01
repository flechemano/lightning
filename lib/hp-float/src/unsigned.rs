use std::{
    fmt,
    ops::{Add, AddAssign, Div, Mul, Sub, SubAssign},
};

use num_bigint::BigUint;
use num_traits::{zero, CheckedDiv, FromPrimitive, Num, ToPrimitive, Zero};
use random_oracle::RandomOracleInput;
use serde::{Deserialize, Serialize};

use crate::{format_hp_float, HpFloatConversionError};

/// A high-precision unsigned floating-point number backed by a `BigUint`.
///
/// `HpUfloat` is parameterized over the precision `P`, which determines the number of digits
/// maintained after the decimal point. This structure is primarily used for accurate accounting
/// in relation to specific tokens where precision requirements are critical.
///
/// The precision `P` is defined at compile time and is crucial for ensuring accurate
/// interoperability with accounting and balances.
///
/// # Examples
///
/// ```
/// use hp_float::unsigned::HpUfloat;
///
/// let value: HpUfloat<18> = 123_u64.into();
/// ```
///
/// In the above example, `HpUfloat<18>` ensures a precision of 18 decimal places.
///
/// # Notes
///
/// The underlying storage is a `BigUint` from the `num-bigint` crate. When the result of an
/// operation has more than `P` digits after the decimal point, it is truncated at `P` digits.
///
/// # Type Parameters
///
/// * `P`: The number of digits to maintain after the decimal point in this `HpUfloat`. Must be a
///   constant that is known at compile time.
///
/// # Attributes
///
/// * `BigUint`: The underlying large unsigned integer value that the `HpUfloat` wraps around.

#[derive(Clone, Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Default)]
pub struct HpUfloat<const P: usize>(BigUint);

impl<const P: usize> HpUfloat<P> {
    pub fn new(value: BigUint) -> Self {
        HpUfloat::<P>(value * BigUint::from(10u32).pow(P.try_into().unwrap()))
    }

    pub fn zero() -> HpUfloat<P> {
        HpUfloat::new(zero())
    }
    pub fn convert_precision<const Q: usize>(&self) -> HpUfloat<Q> {
        let current_value: &BigUint = &self.0;

        let precision_diff: i32 = P as i32 - Q as i32;

        let scaled_value: BigUint = if precision_diff > 0 {
            current_value / BigUint::from(10u128.pow(precision_diff as u32))
        } else {
            current_value * BigUint::from(10u128.pow((-precision_diff) as u32))
        };

        HpUfloat::<Q>(scaled_value)
    }

    pub fn min<'a>(&'a self, rhs: &'a Self) -> &'a Self {
        if self.0 <= rhs.0 { self } else { rhs }
    }

    pub fn max<'a>(&'a self, rhs: &'a Self) -> &'a Self {
        if self.0 >= rhs.0 { self } else { rhs }
    }

    pub fn get_value(&self) -> &BigUint {
        &self.0
    }
}

impl<const P: usize> fmt::Display for HpUfloat<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_hp_float::<BigUint, P>(&self.0, f)
    }
}

impl<const P: usize> Add<HpUfloat<P>> for HpUfloat<P> {
    type Output = HpUfloat<P>;

    fn add(self, rhs: HpUfloat<P>) -> Self::Output {
        HpUfloat::<P>(&self.0 + &rhs.0)
    }
}

impl<const P: usize> Add<HpUfloat<P>> for &HpUfloat<P> {
    type Output = HpUfloat<P>;

    fn add(self, rhs: HpUfloat<P>) -> Self::Output {
        HpUfloat::<P>(&self.0 + &rhs.0)
    }
}

impl<const P: usize> Add<&HpUfloat<P>> for HpUfloat<P> {
    type Output = HpUfloat<P>;

    fn add(self, rhs: &HpUfloat<P>) -> Self::Output {
        HpUfloat::<P>(&self.0 + &rhs.0)
    }
}

impl<const P: usize> Add<&HpUfloat<P>> for &HpUfloat<P> {
    type Output = HpUfloat<P>;

    fn add(self, rhs: &HpUfloat<P>) -> Self::Output {
        HpUfloat::<P>(&self.0 + &rhs.0)
    }
}

impl<const P: usize> Sub<HpUfloat<P>> for HpUfloat<P> {
    type Output = HpUfloat<P>;

    fn sub(self, rhs: HpUfloat<P>) -> Self::Output {
        HpUfloat::<P>(&self.0 - &rhs.0)
    }
}

impl<const P: usize> Sub<HpUfloat<P>> for &HpUfloat<P> {
    type Output = HpUfloat<P>;

    fn sub(self, rhs: HpUfloat<P>) -> Self::Output {
        HpUfloat::<P>(&self.0 - &rhs.0)
    }
}

impl<const P: usize> Sub<&HpUfloat<P>> for HpUfloat<P> {
    type Output = HpUfloat<P>;

    fn sub(self, rhs: &HpUfloat<P>) -> Self::Output {
        HpUfloat::<P>(&self.0 - &rhs.0)
    }
}

impl<const P: usize> Sub<&HpUfloat<P>> for &HpUfloat<P> {
    type Output = HpUfloat<P>;

    fn sub(self, rhs: &HpUfloat<P>) -> Self::Output {
        HpUfloat::<P>(&self.0 - &rhs.0)
    }
}

impl<const P: usize> Mul<HpUfloat<P>> for HpUfloat<P> {
    type Output = HpUfloat<P>;

    fn mul(self, rhs: HpUfloat<P>) -> Self::Output {
        HpUfloat::<P>((&self.0 * &rhs.0) / BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}
impl<const P: usize> Mul<HpUfloat<P>> for &HpUfloat<P> {
    type Output = HpUfloat<P>;

    fn mul(self, rhs: HpUfloat<P>) -> Self::Output {
        HpUfloat::<P>((&self.0 * &rhs.0) / BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}
impl<const P: usize> Mul<&HpUfloat<P>> for HpUfloat<P> {
    type Output = HpUfloat<P>;

    fn mul(self, rhs: &HpUfloat<P>) -> Self::Output {
        HpUfloat::<P>((&self.0 * &rhs.0) / BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}
impl<const P: usize> Mul<&HpUfloat<P>> for &HpUfloat<P> {
    type Output = HpUfloat<P>;

    fn mul(self, rhs: &HpUfloat<P>) -> Self::Output {
        HpUfloat::<P>((&self.0 * &rhs.0) / BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> Div<HpUfloat<P>> for HpUfloat<P> {
    type Output = HpUfloat<P>;

    fn div(self, rhs: HpUfloat<P>) -> Self::Output {
        HpUfloat::<P>((&self.0 * BigUint::from(10u32).pow(P.try_into().unwrap())) / &rhs.0)
    }
}
impl<const P: usize> Div<HpUfloat<P>> for &HpUfloat<P> {
    type Output = HpUfloat<P>;

    fn div(self, rhs: HpUfloat<P>) -> Self::Output {
        HpUfloat::<P>((&self.0 * BigUint::from(10u32).pow(P.try_into().unwrap())) / &rhs.0)
    }
}
impl<const P: usize> Div<&HpUfloat<P>> for HpUfloat<P> {
    type Output = HpUfloat<P>;

    fn div(self, rhs: &HpUfloat<P>) -> Self::Output {
        HpUfloat::<P>((&self.0 * BigUint::from(10u32).pow(P.try_into().unwrap())) / &rhs.0)
    }
}
impl<const P: usize> Div<&HpUfloat<P>> for &HpUfloat<P> {
    type Output = HpUfloat<P>;

    fn div(self, rhs: &HpUfloat<P>) -> Self::Output {
        HpUfloat::<P>((&self.0 * BigUint::from(10u32).pow(P.try_into().unwrap())) / &rhs.0)
    }
}

impl<const P: usize> AddAssign for HpUfloat<P> {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl<const P: usize> SubAssign for HpUfloat<P> {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl<const P: usize> From<f64> for HpUfloat<P> {
    fn from(value: f64) -> Self {
        let s = format!("{value}");
        let parts: Vec<&str> = s.split('.').collect();

        // It is safe to unwrap here since we are converting a valid f64 to a string. If the input
        // value was not a valid f64, this function wouldn't have been called in the first place.
        let integer_part: BigUint = BigUint::from_str_radix(parts[0], 10).unwrap();

        let fraction_part: BigUint = if parts.len() > 1 {
            let mut frac_str = parts[1].to_string();
            while frac_str.len() < P {
                frac_str.push('0');
            }
            frac_str.truncate(P);
            BigUint::from_str_radix(&frac_str, 10).unwrap()
        } else {
            BigUint::zero()
        };

        HpUfloat(integer_part * BigUint::from(10u32).pow(P.try_into().unwrap()) + fraction_part)
    }
}

impl<const P: usize> From<BigUint> for HpUfloat<P> {
    fn from(value: BigUint) -> Self {
        HpUfloat(value * BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> From<u16> for HpUfloat<P> {
    fn from(value: u16) -> Self {
        let value_to_big: BigUint = BigUint::from_u16(value).unwrap();
        HpUfloat(value_to_big * BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}
impl<const P: usize> From<u32> for HpUfloat<P> {
    fn from(value: u32) -> Self {
        let value_to_big: BigUint = BigUint::from_u32(value).unwrap();
        HpUfloat(value_to_big * BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> From<u64> for HpUfloat<P> {
    fn from(value: u64) -> Self {
        let value_to_big: BigUint = BigUint::from_u64(value).unwrap();
        HpUfloat(value_to_big * BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> From<u128> for HpUfloat<P> {
    fn from(value: u128) -> Self {
        let value_to_big: BigUint = BigUint::from_u128(value).unwrap();
        HpUfloat(value_to_big * BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> From<usize> for HpUfloat<P> {
    fn from(value: usize) -> Self {
        let value_to_big: BigUint = BigUint::from_usize(value).unwrap();
        HpUfloat(value_to_big * BigUint::from(10u32).pow(P.try_into().unwrap()))
    }
}

impl<const P: usize> TryFrom<HpUfloat<P>> for f64 {
    type Error = HpFloatConversionError;

    fn try_from(value: HpUfloat<P>) -> Result<Self, Self::Error> {
        let divisor = BigUint::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFloatConversionError::PrecisionLevelNotSupported)?,
        );
        let fraction_part = value.0.clone() % divisor.clone();
        let integer_part = value
            .0
            .checked_div(&divisor)
            .ok_or(HpFloatConversionError::DivisionError)?;
        let s = format!("{integer_part}.{fraction_part}");

        // WARNING: Truncation occurs when converting from HpUfloat to f64 if the string
        // representation is longer than 18 digits. This is expected behavior due to the
        // limited precision of f64. Exercise caution and consider the potential loss of
        // precision for longer decimal values.
        s.parse::<f64>()
            .map_err(|_| HpFloatConversionError::FloatParseError)
    }
}

impl<const P: usize> TryFrom<HpUfloat<P>> for u32 {
    type Error = HpFloatConversionError;

    fn try_from(value: HpUfloat<P>) -> Result<Self, Self::Error> {
        let divisor = BigUint::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFloatConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(&divisor)
            .ok_or(HpFloatConversionError::DivisionError)?;
        interim.to_u32().ok_or(HpFloatConversionError::Overflow)
    }
}

impl<const P: usize> TryFrom<HpUfloat<P>> for u64 {
    type Error = HpFloatConversionError;

    fn try_from(value: HpUfloat<P>) -> Result<Self, Self::Error> {
        let divisor = BigUint::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFloatConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(&divisor)
            .ok_or(HpFloatConversionError::DivisionError)?;
        interim.to_u64().ok_or(HpFloatConversionError::Overflow)
    }
}

impl<const P: usize> TryFrom<HpUfloat<P>> for u128 {
    type Error = HpFloatConversionError;

    fn try_from(value: HpUfloat<P>) -> Result<Self, Self::Error> {
        let divisor = BigUint::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFloatConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(&divisor)
            .ok_or(HpFloatConversionError::DivisionError)?;
        interim.to_u128().ok_or(HpFloatConversionError::Overflow)
    }
}

impl<const P: usize> TryFrom<HpUfloat<P>> for usize {
    type Error = HpFloatConversionError;

    fn try_from(value: HpUfloat<P>) -> Result<Self, Self::Error> {
        let divisor = BigUint::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFloatConversionError::PrecisionLevelNotSupported)?,
        );
        let interim = value
            .0
            .checked_div(&divisor)
            .ok_or(HpFloatConversionError::DivisionError)?;
        interim.to_usize().ok_or(HpFloatConversionError::Overflow)
    }
}

impl<const P: usize> TryFrom<HpUfloat<P>> for BigUint {
    type Error = HpFloatConversionError;

    fn try_from(value: HpUfloat<P>) -> Result<Self, Self::Error> {
        let divisor = BigUint::from(10u32).pow(
            P.try_into()
                .map_err(|_| HpFloatConversionError::PrecisionLevelNotSupported)?,
        );
        value
            .0
            .checked_div(&divisor)
            .ok_or(HpFloatConversionError::DivisionError)
    }
}

impl<const P: usize> RandomOracleInput for HpUfloat<P> {
    const TYPE: &'static str = "HpUfloat";

    fn to_random_oracle_input(&self) -> Vec<u8> {
        let mut input = Vec::new();

        // Append the precision value as a byte
        input.push(P as u8);

        // Append the BigUint data as bytes
        let data_bytes = self.get_value().to_bytes_le();
        input.extend_from_slice(&data_bytes);

        input
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_try_into() {
        let large = HpUfloat::<20>::new(BigUint::from(std::u64::MAX as u128 + 1_u128));
        let medium = HpUfloat::<19>::new(BigUint::from(std::u32::MAX as u64 + 1_u64));
        let small = HpUfloat::<18>::new(BigUint::from(std::u16::MAX as u32 + 1_u32));

        assert_eq!(
            std::u64::MAX as u128 + 1_u128,
            large.clone().try_into().unwrap()
        );
        assert!(matches!(
            TryInto::<usize>::try_into(large.clone()),
            Err(HpFloatConversionError::Overflow)
        ));
        assert!(matches!(
            TryInto::<u64>::try_into(large.clone()),
            Err(HpFloatConversionError::Overflow)
        ));
        assert!(matches!(
            TryInto::<u32>::try_into(large),
            Err(HpFloatConversionError::Overflow)
        ));

        assert_eq!(
            TryInto::<u128>::try_into(medium.clone()).unwrap(),
            std::u32::MAX as u128 + 1
        );
        assert_eq!(
            TryInto::<u64>::try_into(medium.clone()).unwrap(),
            std::u32::MAX as u64 + 1
        );
        assert_eq!(
            TryInto::<usize>::try_into(medium.clone()).unwrap(),
            std::u32::MAX as usize + 1
        );
        assert!(matches!(
            TryInto::<u32>::try_into(medium),
            Err(HpFloatConversionError::Overflow)
        ));

        assert_eq!(
            TryInto::<u128>::try_into(small.clone()).unwrap(),
            std::u16::MAX as u128 + 1
        );
        assert_eq!(
            TryInto::<usize>::try_into(small.clone()).unwrap(),
            std::u16::MAX as usize + 1
        );
        assert_eq!(
            TryInto::<u64>::try_into(small.clone()).unwrap(),
            std::u16::MAX as u64 + 1
        );
        assert_eq!(
            TryInto::<u32>::try_into(small.clone()).unwrap(),
            std::u16::MAX as u32 + 1
        );

        let small_by_2 = &small / &200_u64.try_into().unwrap();
        let small_float: f64 = small_by_2.try_into().unwrap();
        // small_float = 65536(small) / 200   = 327.68
        assert_eq!(327.68, small_float);
        // Todo: more tests to test overflow and bigger gloats
    }

    #[test]
    fn test_hp_float_add() {
        let decimal1: HpUfloat<18> = 1_000_000_000_000_000_000u64.into();
        let decimal2: HpUfloat<18> = 2_000_000_000_000_000_000u64.into();
        let res = BigUint::from(3_000_000_000_000_000_000_000_000_000_000_000_000u128);

        let both_ref = &decimal1 + &decimal2;
        assert_eq!(both_ref.0, res);
        let second_ref = decimal1.clone() + &decimal2;
        assert_eq!(second_ref.0, res);
        let first_ref = &decimal1 + decimal2.clone();
        assert_eq!(first_ref.0, res);
        let both_owned = decimal1 + decimal2;
        assert_eq!(both_owned.0, res);
    }

    #[test]
    fn test_hp_float_sub() {
        let decimal1: HpUfloat<18> = 5_000_000_000_000_000_000u64.into();
        let decimal2: HpUfloat<18> = 2_000_000_000_000_000_000u64.into();
        let res = BigUint::from(3_000_000_000_000_000_000_000_000_000_000_000_000u128);

        let both_ref = &decimal1 - &decimal2;
        assert_eq!(both_ref.0, res);
        let second_ref = decimal1.clone() - &decimal2;
        assert_eq!(second_ref.0, res);
        let first_ref = &decimal1 - decimal2.clone();
        assert_eq!(first_ref.0, res);
        let both_owned = decimal1 - decimal2;
        assert_eq!(both_owned.0, res);
    }

    #[test]
    fn test_hp_float_mul() {
        let decimal1: HpUfloat<18> = 5_000_000u64.into();
        let decimal2: HpUfloat<18> = 2_000_000u64.into();
        let res = BigUint::from(10_000_000_000_000_000_000_000_000_000_000u128);

        let both_ref = &decimal1 * &decimal2;
        assert_eq!(both_ref.0, res);
        let second_ref = decimal1.clone() * &decimal2;
        assert_eq!(second_ref.0, res);
        let first_ref = &decimal1 * decimal2.clone();
        assert_eq!(first_ref.0, res);
        let both_owned = decimal1 * decimal2;
        assert_eq!(both_owned.0, res);
    }

    #[test]
    fn test_hp_float_div() {
        let decimal1: HpUfloat<18> = 1u64.into();
        let decimal2: HpUfloat<18> = 50u64.into();
        let res = BigUint::from(20_000_000_000_000_000u128);

        let both_ref = &decimal1 / &decimal2;
        assert_eq!(both_ref.0, res);
        let second_ref = decimal1.clone() / &decimal2;
        assert_eq!(second_ref.0, res);
        let first_ref = &decimal1 / decimal2.clone();
        assert_eq!(first_ref.0, res);
        let both_owned = decimal1 / decimal2;
        assert_eq!(both_owned.0, res);
    }

    #[test]
    fn test_hp_float_from_f64() {
        let decimal: f64 = 1234.567891234567;
        let result = HpUfloat::<18>::from(decimal);
        assert_eq!(result.0, BigUint::from(1_234_567_891_234_567_000_000u128));
    }
    #[test]
    fn test_hp_float_from_f64_truncation() {
        #[allow(clippy::excessive_precision)]
        let decimal: f64 = 1234.5678912345678909;
        let result = HpUfloat::<18>::from(decimal);
        assert_eq!(result.0, BigUint::from(1_234_567_891_234_568_000_000u128));
    }

    #[test]
    fn test_convert_precsion_up() {
        let decimal: f64 = 1_234.123456;
        let decimal1 = HpUfloat::<6>::from(decimal);
        let result = decimal1.convert_precision::<18>();
        assert_eq!(result.0, BigUint::from(1_234_123_456_000_000_000_000_u128));
    }

    #[test]
    fn test_convert_precsion_down() {
        let decimal: f64 = 1234.123456;
        let decimal1 = HpUfloat::<6>::from(decimal);
        let result = decimal1.convert_precision::<2>();
        assert_eq!(result.0, BigUint::from(123_412_u128));
    }
}
