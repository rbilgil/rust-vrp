//! Provides customized implementation of Growing Self Organizing Map.

mod network;
pub use self::network::Network;

/// Represents an input for network.
pub trait Input {
    /// Returns weights.
    fn weights(&self) -> &[f64];
}

/// Represents input data storage.
pub trait Storage: Default {
    /// An input type.
    type Item: Input;

    /// Adds an input to the storage.
    fn add(&mut self, input: Self::Item);

    /// Returns stored input.
    fn all<'a>(&'a self) -> Box<dyn Iterator<Item = &Self::Item> + 'a>;

    /// Returns a distance between two input weights.
    fn distance(&self, a: &[f64], b: &[f64]) -> f64;
}
