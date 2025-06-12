use std::collections::HashMap;

/// Functor for a given base. Will convert strings to natural numbers
#[derive(Debug)]
pub struct Unbaser {
    /// The numeric base for conversion (2-95)
    base: usize,
    /// Character-to-value mapping for bases > 36. None for native bases (2-36)
    dictionary: Option<HashMap<char, usize>>,
}

impl Unbaser {
    /// Creates a new `Unbaser` for the given base.
    ///
    /// # Supported bases
    ///
    /// - **2 to 36**: Uses standard alphanumeric digits with Rust's native [`usize::from_str_radix`].
    ///   - Base 2: `01`
    ///   - Base 8: `01234567`  
    ///   - Base 10: `0123456789`
    ///   - Base 16: `0123456789abcdef`
    ///   - Base 36: `0123456789abcdefghijklmnopqrstuvwxyz`
    ///
    /// - **37 to 62**: Uses digits + lowercase + uppercase letters (0-9, a-z, A-Z).
    ///   - Commonly used for Base62 encoding
    ///
    /// - **95**: Uses full printable ASCII character set (space through tilde).
    ///   - Includes all symbols: `!"#$%&'()*+,-./:;<=>?@[\]^_`{|}~`
    ///
    /// # Errors
    ///
    /// Returns `"Unsupported base encoding."` if the base is not in the supported range.
    pub fn new(base: usize) -> Result<Self, &'static str> {
        const ALPHANUMERIC: &str = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
        const EXTENDED_ASCII: &str = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";

        let dictionary = match base {
            2..=36 => None,
            37..=62 => Some(Self::build_dict(&ALPHANUMERIC[..base])),
            95 => Some(Self::build_dict(EXTENDED_ASCII)),
            _ => return Err("Unsupported base encoding."),
        };

        Ok(Self { base, dictionary })
    }

    /// Builds a character-to-index dictionary from a given alphabet string.
    ///
    /// Creates a HashMap where each character maps to its position in the alphabet.
    /// This is used for character-to-value lookup during conversion.
    ///
    /// # Arguments
    ///
    /// * `alphabet` - String containing all valid characters for the base, in order
    ///
    /// # Returns
    ///
    /// HashMap mapping each character to its numeric value in the base
    fn build_dict(alphabet: &str) -> HashMap<char, usize> {
        alphabet.chars().enumerate().map(|(i, c)| (c, i)).collect()
    }

    /// Converts a string representing a number in the given base into a `usize`.
    ///
    /// # Arguments
    ///
    /// * `input` - String representation of the number in the specified base
    ///
    /// # Returns
    ///
    /// The converted numeric value as a `usize`
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The input contains characters not valid in the base.
    /// - The format is invalid for native bases (2–36).
    pub fn unbase(&self, input: &str) -> Result<usize, &'static str> {
        match self.base {
            2..=36 => {
                usize::from_str_radix(input, self.base as u32).map_err(|_| "Invalid number format")
            }
            _ => self.unbase_with_dict(input),
        }
    }

    /// Performs base conversion using the character dictionary.
    ///
    /// Converts from right-to-left (least significant to most significant digit)
    /// using the positional notation formula: `sum(digit_value × base^position)`
    ///
    /// Used exclusively for non-native bases (above 36) where Rust's built-in
    /// `from_str_radix` is not available.
    ///
    /// # Arguments
    ///
    /// * `input` - String to convert, must contain only characters in the base's alphabet
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The dictionary is missing (internal error)
    /// - The input contains characters not in the base's alphabet
    fn unbase_with_dict(&self, input: &str) -> Result<usize, &'static str> {
        let dict = self
            .dictionary
            .as_ref()
            .ok_or("Dictionary not initialized")?;

        input
            .chars()
            .rev()
            .enumerate()
            .try_fold(0usize, |acc, (i, ch)| {
                dict.get(&ch)
                    .map(|&value| acc + value * self.base.pow(i as u32))
                    .ok_or("Invalid character in input string.")
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_2() {
        let unbaser = Unbaser::new(2).unwrap();
        assert_eq!(unbaser.unbase("1011").unwrap(), 11);
    }

    #[test]
    fn test_base_10() {
        let unbaser = Unbaser::new(10).unwrap();
        assert_eq!(unbaser.unbase("123").unwrap(), 123);
    }

    #[test]
    fn test_base_16() {
        let unbaser = Unbaser::new(16).unwrap();
        assert_eq!(unbaser.unbase("1f").unwrap(), 31);
    }

    #[test]
    fn test_base_36() {
        let unbaser = Unbaser::new(36).unwrap();
        assert_eq!(unbaser.unbase("z").unwrap(), 35);
    }

    #[test]
    fn test_base_62() {
        let unbaser = Unbaser::new(62).unwrap();
        assert_eq!(unbaser.unbase("Az").unwrap(), 2267);
        assert_eq!(unbaser.unbase("10").unwrap(), 62);
        assert_eq!(unbaser.unbase("Z").unwrap(), 61);
    }

    #[test]
    fn test_base_95() {
        let unbaser = Unbaser::new(95).unwrap();
        assert_eq!(unbaser.unbase("A!").unwrap(), {
            let dict = Unbaser::build_dict(
                " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~",
            );
            dict[&'A'] * 95 + dict[&'!']
        });
    }

    #[test]
    fn test_invalid_base() {
        let err = Unbaser::new(70).unwrap_err();
        assert_eq!(err, "Unsupported base encoding.");
    }

    #[test]
    fn test_invalid_character() {
        let unbaser = Unbaser::new(62).unwrap();
        let err = unbaser.unbase("@").unwrap_err();
        assert_eq!(err, "Invalid character in input string.");
    }

    #[test]
    fn test_invalid_format() {
        let unbaser = Unbaser::new(10).unwrap();
        let err = unbaser.unbase("12a").unwrap_err();
        assert_eq!(err, "Invalid number format");
    }
}
