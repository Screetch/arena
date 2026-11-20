//! The words of the data files of arena: each line holds words parted by white space, and the
//! first word of a line says what the line holds.

use std::fmt::Debug;
use std::str::{FromStr, SplitAsciiWhitespace};

/// The words of one line of a data file of the crates, which their tests read whole, so a word
/// that is missing or that is not a number where the format asks one is a defect of the data.
#[derive(Clone, Debug)]
pub struct Words<'a>(SplitAsciiWhitespace<'a>);

impl<'a> Words<'a> {
  /// The words of `line`.
  pub fn new(line: &'a str) -> Self {
    Self(line.split_ascii_whitespace())
  }

  /// The next word.
  #[expect(
    clippy::expect_used,
    reason = "the data files are data of the crates, which their tests read whole"
  )]
  pub fn word(&mut self) -> &'a str {
    self.0.next().expect("the data holds a word here")
  }

  /// The next word, as a number.
  #[expect(
    clippy::expect_used,
    reason = "the data files are data of the crates, which their tests read whole"
  )]
  pub fn number<T: FromStr<Err: Debug>>(&mut self) -> T {
    self.word().parse().expect("the data holds a number here")
  }

  /// The next `N` words, as numbers.
  pub fn numbers<T: FromStr<Err: Debug>, const N: usize>(&mut self) -> [T; N] {
    [(); N].map(|()| self.number())
  }
}

impl<'a> Iterator for Words<'a> {
  type Item = &'a str;

  fn next(&mut self) -> Option<&'a str> {
    self.0.next()
  }
}
