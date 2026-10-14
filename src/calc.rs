//! Thin wrapper around `fend-core`: evaluation with a time limit, persistent variables
//! and `ans`/`_` for the previous result.

use std::time::{Duration, Instant};

use crate::rates::SharedRates;

/// How long a committed calculation may run before it is aborted.
const COMMIT_LIMIT: Duration = Duration::from_secs(2);
/// Live previews run on every keystroke, so they get a much tighter budget to keep typing smooth.
const PREVIEW_LIMIT: Duration = Duration::from_millis(50);

/// Stops a fend calculation once the deadline passes (e.g. `10^10^10`).
struct Deadline(Instant);

impl fend_core::Interrupt for Deadline {
    fn should_interrupt(&self) -> bool {
        Instant::now() >= self.0
    }
}

/// A calculator session. Variables (`x = 3`) and `ans` survive between calculations.
pub struct Calculator {
    context: fend_core::Context,
}

impl Default for Calculator {
    fn default() -> Self {
        Self::new()
    }
}

impl Calculator {
    /// Creates a calculator with an empty variable scope.
    #[must_use]
    pub fn new() -> Self {
        Self {
            context: fend_core::Context::new(),
        }
    }

    /// Creates a calculator that converts currencies (`100 USD to TWD`) with `rates`.
    #[must_use]
    pub fn with_rates(rates: SharedRates) -> Self {
        let mut calc = Self::new();
        calc.context.set_exchange_rate_handler_v2(rates);
        calc
    }

    /// Evaluates `input` without changing any state. Returns `None` while the input is
    /// incomplete, invalid, trivial (`5` → `5`) or too slow, so the UI can simply hide it.
    #[must_use]
    pub fn preview(&self, input: &str) -> Option<String> {
        let deadline = Deadline(Instant::now() + PREVIEW_LIMIT);
        let result =
            fend_core::evaluate_preview_with_interrupt(input.trim(), &self.context, &deadline);
        let text = result.get_main_result();
        (!text.is_empty()).then(|| text.to_owned())
    }

    /// Evaluates `input`, storing assignments and the result as `ans`.
    ///
    /// # Errors
    /// Returns fend's message when the input is invalid or takes too long.
    pub fn evaluate(&mut self, input: &str) -> Result<String, String> {
        let deadline = Deadline(Instant::now() + COMMIT_LIMIT);
        fend_core::evaluate_with_interrupt(input.trim(), &mut self.context, &deadline)
            .map(|result| result.get_main_result().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic() {
        let mut calc = Calculator::new();
        assert_eq!(calc.evaluate("2 + 3 * 4").as_deref(), Ok("14"));
    }

    #[test]
    fn units_convert() {
        let mut calc = Calculator::new();
        assert_eq!(calc.evaluate("1 km to m").as_deref(), Ok("1000 m"));
    }

    #[test]
    fn variables_and_ans_persist() {
        let mut calc = Calculator::new();
        calc.evaluate("x = 6").expect("assignment");
        assert_eq!(calc.evaluate("x * 7").as_deref(), Ok("42"));
        assert_eq!(calc.evaluate("ans + 1").as_deref(), Ok("43"));
    }

    #[test]
    fn preview_does_not_mutate() {
        let calc = Calculator::new();
        assert_eq!(calc.preview("y = 2; y * 5").as_deref(), Some("10"));
        assert_eq!(calc.preview("y"), None);
    }

    #[test]
    fn preview_hides_errors_and_trivial_input() {
        let calc = Calculator::new();
        assert_eq!(calc.preview("2 +"), None);
        assert_eq!(calc.preview("5"), None);
        assert_eq!(calc.preview(""), None);
    }

    fn sample_rates() -> SharedRates {
        let json = r#"{"date":"2026-10-13","usd":{"usd":1,"twd":32,"eur":0.5}}"#;
        SharedRates::with(crate::rates::Rates::parse(json).expect("sample"))
    }

    #[test]
    fn converts_currency() {
        let mut calc = Calculator::with_rates(sample_rates());
        assert_eq!(calc.evaluate("10 USD to TWD").as_deref(), Ok("320 TWD"));
        assert_eq!(calc.evaluate("1 EUR to TWD").as_deref(), Ok("64 TWD"));
        assert_eq!(calc.preview("2 EUR in USD").as_deref(), Some("4 USD"));
    }

    #[test]
    fn currency_errors_without_rates() {
        let mut calc = Calculator::with_rates(SharedRates::default());
        assert!(calc.evaluate("10 USD to TWD").is_err());
        assert!(
            calc.evaluate("1 + 1").is_ok(),
            "non-currency math unaffected"
        );
    }

    #[test]
    fn slow_input_is_interrupted() {
        let mut calc = Calculator::new();
        let start = Instant::now();
        assert!(calc.evaluate("10^10^10^10").is_err(), "interrupted");
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}
