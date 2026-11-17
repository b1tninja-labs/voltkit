//! Shape of a US second-generation Volt, from GM's VIN standards filed with NHTSA.
//!
//! The filings are `gm-vin-2016` through `gm-vin-2019` in `D:\reference\vin`. Each one
//! has the same worked check-digit example. The Chevrolet Volt page of each filing is
//! the character check. This module does not contain a vehicle identification number;
//! the worked example lives in the unit test.

/// Why a string is not a 2016–2019 US Volt identification number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeError {
    /// Not 17 characters.
    Length,
    /// A character the check-digit table does not assign. `I`, `O`, and `Q` are in this set.
    Character,
    /// Position 9 is not the remainder the Check Digit section computes.
    CheckDigit,
    /// The check digit matches, and a Chevrolet Volt position does not.
    NotUsVolt,
}

/// Position 5 on the Chevrolet Volt page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Series {
    /// `A`: LT, standard emissions, on every card from 2016 through 2019.
    Lt,
    /// `C`: LT with the low-emissions package, on every card from 2016 through 2019.
    LtLowEmissions,
    /// `B`: LTZ on the 2016 card, and Premier on the 2017, 2018, and 2019 cards.
    LtzOrPremier,
    /// `D`: LTZ with the low-emissions package on the 2016 card, and Premier with that package on the 2017, 2018, and 2019 cards.
    LtzOrPremierLowEmissions,
}

/// A string that matches the US Chevrolet Volt card for one of model years 2016–2019.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsVolt {
    /// Position 10: `G` 2016, `H` 2017, `J` 2018, `K` 2019.
    pub model_year: u16,
    /// Position 5.
    pub series: Series,
    /// Positions 12–17, as a number.
    pub sequence: u32,
}

/// Result of the Volt-page check.
pub type Shape = Result<UsVolt, ShapeError>;

const WEIGHT: [u32; 17] = [8, 7, 6, 5, 4, 3, 2, 10, 0, 9, 8, 7, 6, 5, 4, 3, 2];

/// Value assigned by the Check Digit conversion table. Digits keep their value.
fn assigned(c: u8) -> Option<u32> {
    Some(match c {
        b'0'..=b'9' => u32::from(c - b'0'),
        b'A' | b'J' => 1,
        b'B' | b'K' | b'S' => 2,
        b'C' | b'L' | b'T' => 3,
        b'D' | b'M' | b'U' => 4,
        b'E' | b'N' | b'V' => 5,
        b'F' | b'W' => 6,
        b'G' | b'P' | b'X' => 7,
        b'H' | b'Y' => 8,
        b'R' | b'Z' => 9,
        _ => return None,
    })
}

fn chars(vin: &str) -> Result<[u8; 17], ShapeError> {
    let b = vin.as_bytes();
    if b.len() != 17 {
        return Err(ShapeError::Length);
    }
    let mut out = [0u8; 17];
    out.copy_from_slice(b);
    if out.iter().any(|c| assigned(*c).is_none()) {
        return Err(ShapeError::Character);
    }
    Ok(out)
}

/// Sum of (assigned value × position weight). Position 9 has weight 0.
///
/// The worked example in each filing totals 362.
///
/// # Errors
///
/// Returns [`ShapeError::Length`] or [`ShapeError::Character`] when `vin` is not 17
/// assigned characters.
pub fn weighted_sum(vin: &str) -> Result<u32, ShapeError> {
    let c = chars(vin)?;
    let mut sum = 0u32;
    for (i, byte) in c.iter().enumerate() {
        sum += assigned(*byte).ok_or(ShapeError::Character)? * WEIGHT[i];
    }
    Ok(sum)
}

/// Check character for `vin`. Position 9 of the input is ignored because its weight is 0.
///
/// Remainder 0 through 9 is that digit. Remainder 10 is `X`.
///
/// # Errors
///
/// Returns [`ShapeError::Length`] or [`ShapeError::Character`] when `vin` is not 17
/// assigned characters.
pub fn expected_check_digit(vin: &str) -> Result<char, ShapeError> {
    let rem = weighted_sum(vin)? % 11;
    Ok(if rem == 10 {
        'X'
    } else {
        char::from(b'0' + u8::try_from(rem).expect("remainder is 0..=10"))
    })
}

/// `true` when position 9 equals [`expected_check_digit`].
///
/// # Errors
///
/// Returns [`ShapeError::Length`] or [`ShapeError::Character`] when `vin` is not 17
/// assigned characters.
pub fn check_digit_matches(vin: &str) -> Result<bool, ShapeError> {
    let c = chars(vin)?;
    Ok(char::from(c[8]) == expected_check_digit(vin)?)
}

/// Accept a 2016–2019 US Chevrolet Volt string from the four filings.
///
/// Positions, for a car whose first character is `1` (United States): `1G1` and line
/// `R`, series `A`/`B`/`C`/`D` (not `9`, which those pages mark non-US and non-Canada),
/// body `6`, restraint `S`, engine `5` (the card's L3A hybrid; the card calls that
/// engine "GEN 1"), a matching check digit, model year `G`/`H`/`J`/`K`, plant `U`
/// (Detroit-Hamtramck), and six sequence digits.
///
/// # Errors
///
/// Returns a [`ShapeError`] naming the first check that failed.
pub fn us_volt(vin: &str) -> Shape {
    let c = chars(vin)?;
    if !check_digit_matches(vin)? {
        return Err(ShapeError::CheckDigit);
    }
    if &c[0..3] != b"1G1"
        || c[3] != b'R'
        || c[5] != b'6'
        || c[6] != b'S'
        || c[7] != b'5'
        || c[10] != b'U'
    {
        return Err(ShapeError::NotUsVolt);
    }
    let series = match c[4] {
        b'A' => Series::Lt,
        b'B' => Series::LtzOrPremier,
        b'C' => Series::LtLowEmissions,
        b'D' => Series::LtzOrPremierLowEmissions,
        _ => return Err(ShapeError::NotUsVolt),
    };
    let model_year = match c[9] {
        b'G' => 2016,
        b'H' => 2017,
        b'J' => 2018,
        b'K' => 2019,
        _ => return Err(ShapeError::NotUsVolt),
    };
    let sequence = std::str::from_utf8(&c[11..17])
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|_| c[11..17].iter().all(|b| b.is_ascii_digit()))
        .ok_or(ShapeError::NotUsVolt)?;
    Ok(UsVolt {
        model_year,
        series,
        sequence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Worked example from the Check Digit section of gm-vin-2016 through gm-vin-2019.
    /// The filings leave position 9 blank in the multiplication and state that the
    /// remainder 10 makes the check digit `X`. This is the filing's example, not a car.
    const WORKED: &str = "1G2NG12EX2M923456";

    #[test]
    fn the_filing_example_sums_to_362_and_the_check_digit_is_x() {
        assert_eq!(weighted_sum(WORKED), Ok(362));
        assert_eq!(362 % 11, 10);
        assert_eq!(expected_check_digit(WORKED), Ok('X'));
        assert_eq!(check_digit_matches(WORKED), Ok(true));
        let mut other = WORKED.to_string();
        other.replace_range(8..9, "0");
        assert_eq!(expected_check_digit(&other), Ok('X'));
        assert_eq!(check_digit_matches(&other), Ok(false));
    }

    #[test]
    fn the_worked_example_is_not_a_us_volt() {
        assert_eq!(us_volt(WORKED), Err(ShapeError::NotUsVolt));
    }

    #[test]
    fn a_string_built_from_the_volt_card_matches_when_the_check_digit_is_computed() {
        // 2016 card: 1G1, line R, series A, body 6, restraint S, engine 5, year G, plant U.
        // Sequence digits are the last six of the worked example. Position 9 is filled
        // by the same function, so this is not a second copied number.
        let mut s = "1G1RA6S50GU923456".to_string();
        let digit = expected_check_digit(&s).unwrap();
        s.replace_range(8..9, &digit.to_string());
        assert_eq!(
            us_volt(&s),
            Ok(UsVolt {
                model_year: 2016,
                series: Series::Lt,
                sequence: 923_456,
            })
        );
        s.replace_range(9..10, "H");
        let digit = expected_check_digit(&s).unwrap();
        s.replace_range(8..9, &digit.to_string());
        assert_eq!(us_volt(&s).unwrap().model_year, 2017);
        s.replace_range(9..10, "J");
        let digit = expected_check_digit(&s).unwrap();
        s.replace_range(8..9, &digit.to_string());
        assert_eq!(us_volt(&s).unwrap().model_year, 2018);
        s.replace_range(9..10, "K");
        let digit = expected_check_digit(&s).unwrap();
        s.replace_range(8..9, &digit.to_string());
        assert_eq!(us_volt(&s).unwrap().model_year, 2019);

        s.replace_range(4..5, "9");
        let digit = expected_check_digit(&s).unwrap();
        s.replace_range(8..9, &digit.to_string());
        assert_eq!(us_volt(&s), Err(ShapeError::NotUsVolt));
    }

    #[test]
    fn letters_the_table_omits_are_rejected() {
        assert_eq!(us_volt("1G2NG12EX2M92345"), Err(ShapeError::Length));
        assert_eq!(
            expected_check_digit("1G2NG12IX2M923456"),
            Err(ShapeError::Character)
        );
        assert_eq!(
            expected_check_digit("1G2NG12OX2M923456"),
            Err(ShapeError::Character)
        );
        assert_eq!(
            expected_check_digit("1G2NG12QX2M923456"),
            Err(ShapeError::Character)
        );
    }
}
