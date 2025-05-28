/// Strategies for displaying missing files
use crate::verification::verifier::VerificationResult;

/// Trait for different missing file display strategies
pub trait MissingFileDisplayStrategy {
    /// Display missing files according to the strategy
    fn display_missing_files(&self, result: &VerificationResult);

    /// Get the name of the strategy for identification
    fn name(&self) -> &'static str;
}

/// Strategy 1: Display all missing files individually
pub struct DetailedDisplayStrategy;

impl MissingFileDisplayStrategy for DetailedDisplayStrategy {
    fn display_missing_files(&self, result: &VerificationResult) {
        for missing in &result.missing_files {
            println!("    - {}", missing);
        }
    }

    fn name(&self) -> &'static str {
        "detailed"
    }
}

/// Strategy 2: Display consolidated view (directories when appropriate)
pub struct ConsolidatedDisplayStrategy;

impl MissingFileDisplayStrategy for ConsolidatedDisplayStrategy {
    fn display_missing_files(&self, result: &VerificationResult) {
        let consolidated_missing = result.get_consolidated_missing_files();
        for missing in &consolidated_missing {
            println!("    - {}", missing);
        }
    }

    fn name(&self) -> &'static str {
        "consolidated"
    }
}

/// Context that uses a display strategy
pub struct MissingFileDisplayContext {
    strategy: Box<dyn MissingFileDisplayStrategy>,
}

impl MissingFileDisplayContext {
    /// Create a new display context with the specified strategy
    pub fn new(strategy: Box<dyn MissingFileDisplayStrategy>) -> Self {
        Self { strategy }
    }

    /// Create context with detailed display strategy
    pub fn with_detailed_strategy() -> Self {
        Self::new(Box::new(DetailedDisplayStrategy))
    }

    /// Create context with consolidated display strategy
    pub fn with_consolidated_strategy() -> Self {
        Self::new(Box::new(ConsolidatedDisplayStrategy))
    }

    /// Display missing files using the configured strategy
    pub fn display_missing_files(&self, result: &VerificationResult) {
        self.strategy.display_missing_files(result);
    }

    /// Get the name of the current strategy
    pub fn strategy_name(&self) -> &'static str {
        self.strategy.name()
    }

    /// Switch to a different strategy
    pub fn set_strategy(&mut self, strategy: Box<dyn MissingFileDisplayStrategy>) {
        self.strategy = strategy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detailed_strategy_name() {
        let strategy = DetailedDisplayStrategy;
        assert_eq!(strategy.name(), "detailed");
    }

    #[test]
    fn test_consolidated_strategy_name() {
        let strategy = ConsolidatedDisplayStrategy;
        assert_eq!(strategy.name(), "consolidated");
    }

    #[test]
    fn test_context_creation() {
        let context = MissingFileDisplayContext::with_detailed_strategy();
        assert_eq!(context.strategy_name(), "detailed");

        let context = MissingFileDisplayContext::with_consolidated_strategy();
        assert_eq!(context.strategy_name(), "consolidated");
    }
}
